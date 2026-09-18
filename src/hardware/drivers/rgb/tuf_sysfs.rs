// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Clean-room implementation of ASUS TUF series keyboard backlighting driver.
//!
//! Controls 1-zone keyboard RGB backlighting via ACPI sysfs attributes:
//! - `/sys/class/leds/asus::kbd_backlight/kbd_rgb_mode` (6-byte binary command)
//! - `/sys/class/leds/asus::kbd_backlight/brightness` (Discrete 0..=3 level)

use crate::domain::{BrightnessPercent, ColorRgb, RgbTimeoutPolicy, TimeoutDuration};
use crate::hardware::capabilities::UnavailableReason;
use crate::hardware::error::DriverError;
use crate::hardware::traits::RgbDriver;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Default sysfs path for the TUF keyboard RGB mode attribute.
pub const SYS_KBD_RGB_MODE: &str = "/sys/class/leds/asus::kbd_backlight/kbd_rgb_mode";
/// Default sysfs path for the keyboard backlight brightness attribute.
pub const SYS_KBD_BRIGHTNESS: &str = "/sys/class/leds/asus::kbd_backlight/brightness";

/// TUF RGB Lighting Mode: Static single color.
pub const TUF_MODE_STATIC: u8 = 0;
/// TUF RGB Lighting Mode: Breathing cycle.
pub const TUF_MODE_BREATHING: u8 = 1;
/// TUF RGB Lighting Mode: Strobing cycle.
pub const TUF_MODE_STROBING: u8 = 2;
/// TUF RGB Lighting Mode: Color rainbow spectrum cycle.
pub const TUF_MODE_COLOR_CYCLE: u8 = 3;

/// TUF Effect Speed: Slow.
pub const TUF_SPEED_SLOW: u8 = 0;
/// TUF Effect Speed: Normal.
pub const TUF_SPEED_NORMAL: u8 = 1;
/// TUF Effect Speed: Fast.
pub const TUF_SPEED_FAST: u8 = 2;

/// Builds a 6-byte TUF ACPI sysfs command payload `[1, mode, r, g, b, speed]`.
pub fn build_tuf_rgb_packet(mode: u8, r: u8, g: u8, b: u8, speed: u8) -> [u8; 6] {
    [1, mode, r, g, b, speed]
}

/// Scales percentage (0..=100) to discrete sysfs backlight level (0..=3).
pub fn percent_to_sysfs_level(percent: u8) -> u8 {
    match percent {
        0 => 0,
        1..=33 => 1,
        34..=66 => 2,
        _ => 3,
    }
}

/// Converts discrete sysfs backlight level (0..=3) to approximate percentage.
pub fn sysfs_level_to_percent(level: u8) -> u8 {
    match level {
        0 => 0,
        1 => 33,
        2 => 66,
        _ => 100,
    }
}

/// Communication backend for `TufSysfsRgbDriver`.
#[derive(Debug)]
pub enum TufBackend {
    /// Live kernel sysfs nodes.
    Sysfs {
        rgb_mode_path: PathBuf,
        brightness_path: PathBuf,
    },
    /// In-memory mock for headless testing.
    Mock {
        written_packets: Arc<Mutex<Vec<[u8; 6]>>>,
        brightness_level: Arc<Mutex<u8>>,
        simulate_failure: bool,
    },
}

impl TufBackend {
    /// Writes 6-byte RGB payload to `kbd_rgb_mode`.
    pub fn write_rgb_mode(&mut self, packet: &[u8; 6]) -> Result<(), DriverError> {
        match self {
            Self::Sysfs { rgb_mode_path, .. } => std::fs::write(rgb_mode_path.as_path(), packet)
                .map_err(|e| {
                    DriverError::Io(std::io::Error::new(
                        e.kind(),
                        format!("Failed to write to {}: {e}", rgb_mode_path.display()),
                    ))
                }),
            Self::Mock {
                written_packets,
                simulate_failure,
                ..
            } => {
                if *simulate_failure {
                    return Err(DriverError::Communication(
                        "Simulated TUF sysfs failure".into(),
                    ));
                }
                let mut guard = written_packets.lock().map_err(|_| {
                    DriverError::Unavailable(UnavailableReason::HardwareError(
                        "Mock lock poisoned".into(),
                    ))
                })?;
                guard.push(*packet);
                Ok(())
            }
        }
    }

    /// Writes discrete 0..=3 level to `brightness`.
    pub fn write_brightness(&mut self, level: u8) -> Result<(), DriverError> {
        match self {
            Self::Sysfs {
                brightness_path, ..
            } => std::fs::write(brightness_path.as_path(), format!("{level}\n")).map_err(|e| {
                DriverError::Io(std::io::Error::new(
                    e.kind(),
                    format!("Failed to write to {}: {e}", brightness_path.display()),
                ))
            }),
            Self::Mock {
                brightness_level,
                simulate_failure,
                ..
            } => {
                if *simulate_failure {
                    return Err(DriverError::Communication(
                        "Simulated TUF sysfs failure".into(),
                    ));
                }
                let mut guard = brightness_level.lock().map_err(|_| {
                    DriverError::Unavailable(UnavailableReason::HardwareError(
                        "Mock lock poisoned".into(),
                    ))
                })?;
                *guard = level;
                Ok(())
            }
        }
    }

    /// Reads discrete 0..=3 level from `brightness`.
    pub fn read_brightness(&self) -> Result<u8, DriverError> {
        match self {
            Self::Sysfs {
                brightness_path, ..
            } => {
                let s = std::fs::read_to_string(brightness_path).map_err(|e| {
                    DriverError::Io(std::io::Error::new(
                        e.kind(),
                        format!("Failed to read {}: {e}", brightness_path.display()),
                    ))
                })?;
                s.trim().parse::<u8>().map_err(|e| {
                    DriverError::InvalidParameter(format!("Invalid brightness integer: {e}"))
                })
            }
            Self::Mock {
                brightness_level,
                simulate_failure,
                ..
            } => {
                if *simulate_failure {
                    return Err(DriverError::Communication(
                        "Simulated TUF sysfs failure".into(),
                    ));
                }
                let guard = brightness_level.lock().map_err(|_| {
                    DriverError::Unavailable(UnavailableReason::HardwareError(
                        "Mock lock poisoned".into(),
                    ))
                })?;
                Ok(*guard)
            }
        }
    }
}

/// Hardware driver for ASUS TUF series ACPI sysfs keyboard backlighting.
#[derive(Debug)]
pub struct TufSysfsRgbDriver {
    backend: TufBackend,
    cached_color: ColorRgb,
    cached_brightness: BrightnessPercent,
    policy: RgbTimeoutPolicy,
    timeout: TimeoutDuration,
    sleeping: bool,
}

/// Shared packet log buffer for TUF RGB mock driver.
pub type TufPacketLog = Arc<Mutex<Vec<[u8; 6]>>>;

/// Shared brightness level handle for TUF RGB mock driver.
pub type TufBrightnessHandle = Arc<Mutex<u8>>;

impl TufSysfsRgbDriver {
    /// Probes for TUF keyboard sysfs node at `/sys/class/leds/asus::kbd_backlight/kbd_rgb_mode`.
    ///
    /// Returns `Ok(Some(driver))` if present and accessible, or `Ok(None)` if absent.
    pub fn probe() -> Result<Option<Self>, DriverError> {
        Self::probe_with_paths(Path::new(SYS_KBD_RGB_MODE), Path::new(SYS_KBD_BRIGHTNESS))
    }

    /// Probes with custom paths (useful for tests or mock sysfs environments).
    pub fn probe_with_paths(
        rgb_mode_path: &Path,
        brightness_path: &Path,
    ) -> Result<Option<Self>, DriverError> {
        if !rgb_mode_path.exists() {
            return Ok(None);
        }

        match std::fs::OpenOptions::new().write(true).open(rgb_mode_path) {
            Ok(_) => {
                tracing::info!(
                    "Discovered TUF ACPI RGB interface at {}",
                    rgb_mode_path.display()
                );
                Ok(Some(Self {
                    backend: TufBackend::Sysfs {
                        rgb_mode_path: rgb_mode_path.to_path_buf(),
                        brightness_path: brightness_path.to_path_buf(),
                    },
                    cached_color: ColorRgb::new(255, 255, 255),
                    cached_brightness: BrightnessPercent::new(100).unwrap(),
                    policy: RgbTimeoutPolicy::Always,
                    timeout: TimeoutDuration::default(),
                    sleeping: false,
                }))
            }
            Err(e) => {
                tracing::warn!(
                    "TUF RGB mode node exists at {} but cannot write: {}",
                    rgb_mode_path.display(),
                    e
                );
                Ok(None)
            }
        }
    }

    /// Creates an in-memory mock driver for headless tests.
    pub fn new_mock() -> (Self, TufPacketLog, TufBrightnessHandle) {
        let packets = Arc::new(Mutex::new(Vec::new()));
        let brightness = Arc::new(Mutex::new(3));
        let driver = Self {
            backend: TufBackend::Mock {
                written_packets: packets.clone(),
                brightness_level: brightness.clone(),
                simulate_failure: false,
            },
            cached_color: ColorRgb::new(255, 255, 255),
            cached_brightness: BrightnessPercent::new(100).unwrap(),
            policy: RgbTimeoutPolicy::Always,
            timeout: TimeoutDuration::default(),
            sleeping: false,
        };
        (driver, packets, brightness)
    }

    /// Creates a failing in-memory mock driver.
    pub fn new_mock_failing() -> Self {
        Self {
            backend: TufBackend::Mock {
                written_packets: Arc::new(Mutex::new(Vec::new())),
                brightness_level: Arc::new(Mutex::new(0)),
                simulate_failure: true,
            },
            cached_color: ColorRgb::new(255, 255, 255),
            cached_brightness: BrightnessPercent::new(100).unwrap(),
            policy: RgbTimeoutPolicy::Always,
            timeout: TimeoutDuration::default(),
            sleeping: false,
        }
    }
}

impl RgbDriver for TufSysfsRgbDriver {
    fn set_color(&mut self, color: ColorRgb) -> Result<(), DriverError> {
        self.cached_color = color;
        if self.sleeping {
            return Ok(());
        }

        let pkt =
            build_tuf_rgb_packet(TUF_MODE_STATIC, color.r, color.g, color.b, TUF_SPEED_NORMAL);
        self.backend.write_rgb_mode(&pkt)
    }

    fn set_brightness(&mut self, brightness: BrightnessPercent) -> Result<(), DriverError> {
        self.cached_brightness = brightness;
        if self.sleeping {
            return Ok(());
        }

        let level = percent_to_sysfs_level(brightness.value());
        self.backend.write_brightness(level)
    }

    fn get_brightness(&self) -> Result<BrightnessPercent, DriverError> {
        if let Ok(level) = self.backend.read_brightness() {
            let pct = sysfs_level_to_percent(level);
            return BrightnessPercent::new(pct)
                .map_err(|e| DriverError::InvalidParameter(e.to_string()));
        }
        Ok(self.cached_brightness)
    }

    fn set_timeout(
        &mut self,
        policy: RgbTimeoutPolicy,
        duration: TimeoutDuration,
    ) -> Result<(), DriverError> {
        self.policy = policy;
        self.timeout = duration;
        Ok(())
    }

    fn turn_off(&mut self) -> Result<(), DriverError> {
        self.sleeping = true;
        let _ = self.backend.write_brightness(0);
        Ok(())
    }

    fn wake(&mut self) -> Result<(), DriverError> {
        self.sleeping = false;
        let level = percent_to_sysfs_level(self.cached_brightness.value());
        let _ = self.backend.write_brightness(level);
        let pkt = build_tuf_rgb_packet(
            TUF_MODE_STATIC,
            self.cached_color.r,
            self.cached_color.g,
            self.cached_color.b,
            TUF_SPEED_NORMAL,
        );
        let _ = self.backend.write_rgb_mode(&pkt);
        Ok(())
    }

    fn is_sleeping(&self) -> bool {
        self.sleeping
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tuf_packet_structure() {
        let pkt = build_tuf_rgb_packet(TUF_MODE_STATIC, 255, 128, 64, TUF_SPEED_FAST);
        assert_eq!(pkt.len(), 6);
        assert_eq!(pkt[0], 1); // Fixed header
        assert_eq!(pkt[1], TUF_MODE_STATIC);
        assert_eq!(pkt[2], 255); // R
        assert_eq!(pkt[3], 128); // G
        assert_eq!(pkt[4], 64); // B
        assert_eq!(pkt[5], TUF_SPEED_FAST); // Speed
    }

    #[test]
    fn test_tuf_brightness_scaling() {
        assert_eq!(percent_to_sysfs_level(0), 0);
        assert_eq!(percent_to_sysfs_level(20), 1);
        assert_eq!(percent_to_sysfs_level(33), 1);
        assert_eq!(percent_to_sysfs_level(50), 2);
        assert_eq!(percent_to_sysfs_level(66), 2);
        assert_eq!(percent_to_sysfs_level(85), 3);
        assert_eq!(percent_to_sysfs_level(100), 3);

        assert_eq!(sysfs_level_to_percent(0), 0);
        assert_eq!(sysfs_level_to_percent(1), 33);
        assert_eq!(sysfs_level_to_percent(2), 66);
        assert_eq!(sysfs_level_to_percent(3), 100);
    }

    #[test]
    fn test_tuf_mock_driver_color_and_brightness() {
        let (mut driver, packets, brightness) = TufSysfsRgbDriver::new_mock();

        driver.set_color(ColorRgb::new(100, 150, 200)).unwrap();
        let logged = packets.lock().unwrap().clone();
        assert_eq!(logged.len(), 1);
        assert_eq!(
            logged[0],
            [1, TUF_MODE_STATIC, 100, 150, 200, TUF_SPEED_NORMAL]
        );

        driver
            .set_brightness(BrightnessPercent::new(50).unwrap())
            .unwrap();
        assert_eq!(*brightness.lock().unwrap(), 2);
        assert_eq!(driver.get_brightness().unwrap().value(), 66);
    }

    #[test]
    fn test_tuf_mock_sleep_wake() {
        let (mut driver, packets, brightness) = TufSysfsRgbDriver::new_mock();

        driver.turn_off().unwrap();
        assert!(driver.is_sleeping());
        assert_eq!(*brightness.lock().unwrap(), 0);

        // Writing color while sleeping is ignored
        packets.lock().unwrap().clear();
        driver.set_color(ColorRgb::new(255, 0, 0)).unwrap();
        assert_eq!(packets.lock().unwrap().len(), 0);

        // Wake restores cached state
        driver.wake().unwrap();
        assert!(!driver.is_sleeping());
        assert_eq!(*brightness.lock().unwrap(), 3); // Restores default 100%
        let logged = packets.lock().unwrap().clone();
        assert_eq!(logged.len(), 1);
        assert_eq!(logged[0], [1, TUF_MODE_STATIC, 255, 0, 0, TUF_SPEED_NORMAL]);
    }

    #[test]
    fn test_tuf_probe_missing_path() {
        let non_existent = Path::new("/tmp/nonexistent_tuf_sysfs_path_123");
        let res = TufSysfsRgbDriver::probe_with_paths(non_existent, non_existent);
        assert!(matches!(res, Ok(None)));
    }
}
