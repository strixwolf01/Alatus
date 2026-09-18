// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Clean-room implementation of ASUS ROG Aura USB HID keyboard backlighting driver.
//!
//! Controls illumination via 64-byte padded HID output reports (Report ID `0x5d`)
//! over Linux `/dev/hidraw*` devices.

use crate::domain::{BrightnessPercent, ColorRgb, RgbTimeoutPolicy, TimeoutDuration};
use crate::hardware::capabilities::UnavailableReason;
use crate::hardware::error::DriverError;
use crate::hardware::traits::RgbDriver;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// ASUS USB Vendor ID.
pub const AURA_USB_VID: u16 = 0x0b05;

/// Known ASUS ROG Aura USB Product IDs.
pub const AURA_ROG_PIDS: &[u16] = &[
    0x1866, // GL504, GL704
    0x18c6, // G531, G731
    0x19b6, // GA401, GA402, G513, G533, G733, G614, G814
    0x1a30, // GV301, G634, G834
    0x1ce6, // 2024-2026 ROG Zephyrus & Strix
];

/// Mandatory Aura HID Report ID.
pub const REPORT_ID: u8 = 0x5d;

/// Built-in mode configuration command.
pub const CMD_BUILTIN_MODE: u8 = 0xb3;
/// Direct custom/multi-zone color command.
pub const CMD_DIRECT_COLOR: u8 = 0xbc;
/// Brightness control command.
pub const CMD_BRIGHTNESS: u8 = 0xba;
/// Firmware commit SET command.
pub const CMD_SET: u8 = 0xb5;
/// Firmware commit APPLY command.
pub const CMD_APPLY: u8 = 0xb4;

/// Pure constructor for a 64-byte Aura built-in static color report (`0xb3`).
pub fn build_static_color_packet(r: u8, g: u8, b: u8) -> [u8; 64] {
    let mut pkt = [0u8; 64];
    pkt[0] = REPORT_ID;
    pkt[1] = CMD_BUILTIN_MODE;
    pkt[2] = 0x00; // Zone: All / Global
    pkt[3] = 0x00; // Mode: Static
    pkt[4] = r;
    pkt[5] = g;
    pkt[6] = b;
    pkt[7] = 0x00; // Speed: Slow
    pkt[8] = 0x00; // Direction: Right
    pkt
}

/// Pure constructor for a 64-byte Aura direct custom color report (`0xbc`) for 1 or 4 zones.
pub fn build_direct_color_packet(r: u8, g: u8, b: u8, zones: u8) -> [u8; 64] {
    let mut pkt = [0u8; 64];
    pkt[0] = REPORT_ID;
    pkt[1] = CMD_DIRECT_COLOR;
    pkt[2] = 0x01;
    pkt[3] = 0x01;
    if zones == 4 {
        pkt[4] = 0x04;
        for z in 0..4 {
            let offset = 9 + z * 3;
            pkt[offset] = r;
            pkt[offset + 1] = g;
            pkt[offset + 2] = b;
        }
    } else {
        pkt[4] = 0x00;
        pkt[9] = r;
        pkt[10] = g;
        pkt[11] = b;
    }
    pkt
}

/// Pure constructor for a 64-byte SET command packet (`0xb5`).
pub fn build_set_packet() -> [u8; 64] {
    let mut pkt = [0u8; 64];
    pkt[0] = REPORT_ID;
    pkt[1] = CMD_SET;
    pkt
}

/// Pure constructor for a 64-byte APPLY command packet (`0xb4`).
pub fn build_apply_packet() -> [u8; 64] {
    let mut pkt = [0u8; 64];
    pkt[0] = REPORT_ID;
    pkt[1] = CMD_APPLY;
    pkt
}

/// Pure constructor for a 64-byte brightness packet (`0xba`).
pub fn build_brightness_packet(level: u8) -> [u8; 64] {
    let mut pkt = [0u8; 64];
    pkt[0] = REPORT_ID;
    pkt[1] = CMD_BRIGHTNESS;
    pkt[2] = 0xc5;
    pkt[3] = 0xc4;
    pkt[4] = level.min(3);
    pkt
}

/// Scales percentage (0..=100) to discrete Aura hardware brightness level (0..=3).
pub fn percent_to_aura_level(percent: u8) -> u8 {
    match percent {
        0 => 0,
        1..=33 => 1,
        34..=66 => 2,
        _ => 3,
    }
}

/// Converts discrete Aura hardware brightness level (0..=3) to percentage.
pub fn aura_level_to_percent(level: u8) -> u8 {
    match level {
        0 => 0,
        1 => 33,
        2 => 66,
        _ => 100,
    }
}

/// Communication backend for `AuraHidDriver`.
#[derive(Debug)]
pub enum AuraBackend {
    /// Physical `/dev/hidraw*` node.
    Device { file: File, path: PathBuf },
    /// In-memory mock for headless testing.
    Mock {
        written_packets: Arc<Mutex<Vec<[u8; 64]>>>,
        simulate_failure: bool,
    },
}

impl AuraBackend {
    /// Writes a 64-byte report to the backend.
    pub fn send_packet(&mut self, pkt: &[u8; 64]) -> Result<(), DriverError> {
        match self {
            Self::Device { file, path } => {
                use std::io::Write;
                file.write_all(pkt).map_err(|e| {
                    DriverError::Io(std::io::Error::new(
                        e.kind(),
                        format!("Failed to write HID packet to {}: {e}", path.display()),
                    ))
                })?;
                let _ = file.flush();
                Ok(())
            }
            Self::Mock {
                written_packets,
                simulate_failure,
            } => {
                if *simulate_failure {
                    return Err(DriverError::Communication(
                        "Simulated Aura HID failure".into(),
                    ));
                }
                let mut guard = written_packets.lock().map_err(|_| {
                    DriverError::Unavailable(UnavailableReason::HardwareError(
                        "Mock lock poisoned".into(),
                    ))
                })?;
                guard.push(*pkt);
                Ok(())
            }
        }
    }
}

/// Hardware driver for ASUS ROG Aura USB HID keyboard backlighting.
#[derive(Debug)]
pub struct AuraHidDriver {
    backend: AuraBackend,
    cached_color: ColorRgb,
    cached_brightness: BrightnessPercent,
    policy: RgbTimeoutPolicy,
    timeout: TimeoutDuration,
    sleeping: bool,
    sysfs_backlight_path: PathBuf,
    zones: u8,
}

impl AuraHidDriver {
    /// Discovers host Aura HID nodes by scanning `/sys/class/hidraw`.
    ///
    /// Returns `Ok(Some(driver))` if a matching ROG Aura controller is found and accessible,
    /// or `Ok(None)` if absent or if permissions prevent access.
    pub fn probe() -> Result<Option<Self>, DriverError> {
        Self::probe_with_paths(
            Path::new("/sys/class/hidraw"),
            Path::new("/dev"),
            Path::new(crate::services::rgb::SYS_KBD_BACKLIGHT),
        )
    }

    /// Probes for Aura HID hardware with explicit paths (useful for test environments).
    pub fn probe_with_paths(
        hidraw_class_dir: &Path,
        dev_dir: &Path,
        sysfs_backlight_path: &Path,
    ) -> Result<Option<Self>, DriverError> {
        if !hidraw_class_dir.exists() {
            return Ok(None);
        }

        let entries = match std::fs::read_dir(hidraw_class_dir) {
            Ok(e) => e,
            Err(_) => return Ok(None),
        };

        for entry in entries.flatten() {
            let uevent_path = entry.path().join("device/uevent");
            let Ok(content) = std::fs::read_to_string(&uevent_path) else {
                continue;
            };

            let Some((vid, pid)) = Self::parse_uevent_ids(&content) else {
                continue;
            };

            if vid != AURA_USB_VID || !AURA_ROG_PIDS.contains(&pid) {
                continue;
            }

            // Avoid matching LampArray ITE5570 which is driven by Ite5570Driver
            if content.contains("ITE5570") {
                continue;
            }

            let file_name = entry.file_name();
            let dev_path = dev_dir.join(file_name);

            match std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&dev_path)
            {
                Ok(file) => {
                    tracing::info!(
                        "Discovered Aura HID controller at {} (VID: {:#06x}, PID: {:#06x})",
                        dev_path.display(),
                        vid,
                        pid
                    );
                    return Ok(Some(Self {
                        backend: AuraBackend::Device {
                            file,
                            path: dev_path,
                        },
                        cached_color: ColorRgb::new(255, 255, 255),
                        cached_brightness: BrightnessPercent::new(100).unwrap(),
                        policy: RgbTimeoutPolicy::Always,
                        timeout: TimeoutDuration::default(),
                        sleeping: false,
                        sysfs_backlight_path: sysfs_backlight_path.to_path_buf(),
                        zones: 4,
                    }));
                }
                Err(e) => {
                    tracing::warn!(
                        "Aura HID device found at {} but failed to open: {}",
                        dev_path.display(),
                        e
                    );
                    return Ok(None);
                }
            }
        }

        Ok(None)
    }

    /// Parses HID Vendor and Product IDs from a sysfs `uevent` file content.
    pub fn parse_uevent_ids(content: &str) -> Option<(u16, u16)> {
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix("HID_ID=") {
                let parts: Vec<&str> = rest.split(':').collect();
                if parts.len() >= 3 {
                    let vid = u32::from_str_radix(parts[1], 16).ok().map(|v| v as u16)?;
                    let pid = u32::from_str_radix(parts[2], 16).ok().map(|v| v as u16)?;
                    return Some((vid, pid));
                }
            }
        }
        None
    }

    /// Constructs a mock Aura HID driver for headless testing and verification.
    pub fn new_mock() -> (Self, Arc<Mutex<Vec<[u8; 64]>>>) {
        let packets = Arc::new(Mutex::new(Vec::new()));
        let driver = Self {
            backend: AuraBackend::Mock {
                written_packets: packets.clone(),
                simulate_failure: false,
            },
            cached_color: ColorRgb::new(255, 255, 255),
            cached_brightness: BrightnessPercent::new(100).unwrap(),
            policy: RgbTimeoutPolicy::Always,
            timeout: TimeoutDuration::default(),
            sleeping: false,
            sysfs_backlight_path: PathBuf::from("/nonexistent"),
            zones: 4,
        };
        (driver, packets)
    }

    /// Constructs a failing mock Aura HID driver.
    pub fn new_mock_failing() -> Self {
        let packets = Arc::new(Mutex::new(Vec::new()));
        Self {
            backend: AuraBackend::Mock {
                written_packets: packets,
                simulate_failure: true,
            },
            cached_color: ColorRgb::new(255, 255, 255),
            cached_brightness: BrightnessPercent::new(100).unwrap(),
            policy: RgbTimeoutPolicy::Always,
            timeout: TimeoutDuration::default(),
            sleeping: false,
            sysfs_backlight_path: PathBuf::from("/nonexistent"),
            zones: 4,
        }
    }

    fn write_sysfs_brightness(&self, level: u32) -> Result<(), DriverError> {
        let path = self.sysfs_backlight_path.join("brightness");
        if path.exists() {
            std::fs::write(&path, format!("{level}\n")).map_err(DriverError::Io)?;
        }
        Ok(())
    }

    fn read_sysfs_brightness(&self) -> Option<u32> {
        let path = self.sysfs_backlight_path.join("brightness");
        if path.exists() {
            std::fs::read_to_string(path)
                .ok()
                .and_then(|c| c.trim().parse::<u32>().ok())
        } else {
            None
        }
    }

    /// Sets the number of keyboard lighting zones configured.
    pub fn set_zones(&mut self, zones: u8) {
        self.zones = zones;
    }

    /// Returns the number of keyboard lighting zones configured.
    pub fn zones(&self) -> u8 {
        self.zones
    }
}

impl RgbDriver for AuraHidDriver {
    fn set_color(&mut self, color: ColorRgb) -> Result<(), DriverError> {
        self.cached_color = color;
        if self.sleeping {
            return Ok(());
        }

        let color_pkt = build_static_color_packet(color.r, color.g, color.b);
        let set_pkt = build_set_packet();
        let apply_pkt = build_apply_packet();

        self.backend.send_packet(&color_pkt)?;
        self.backend.send_packet(&set_pkt)?;
        self.backend.send_packet(&apply_pkt)?;
        Ok(())
    }

    fn set_brightness(&mut self, brightness: BrightnessPercent) -> Result<(), DriverError> {
        self.cached_brightness = brightness;
        if self.sleeping {
            return Ok(());
        }

        let level = percent_to_aura_level(brightness.value());
        let _ = self.write_sysfs_brightness(level as u32);

        let bright_pkt = build_brightness_packet(level);
        let set_pkt = build_set_packet();
        let apply_pkt = build_apply_packet();

        self.backend.send_packet(&bright_pkt)?;
        self.backend.send_packet(&set_pkt)?;
        self.backend.send_packet(&apply_pkt)?;
        Ok(())
    }

    fn get_brightness(&self) -> Result<BrightnessPercent, DriverError> {
        if let Some(level) = self.read_sysfs_brightness() {
            let pct = aura_level_to_percent(level as u8);
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
        let _ = self.write_sysfs_brightness(0);

        let bright_pkt = build_brightness_packet(0);
        let set_pkt = build_set_packet();
        let apply_pkt = build_apply_packet();

        let _ = self.backend.send_packet(&bright_pkt);
        let _ = self.backend.send_packet(&set_pkt);
        let _ = self.backend.send_packet(&apply_pkt);
        Ok(())
    }

    fn wake(&mut self) -> Result<(), DriverError> {
        self.sleeping = false;
        let level = percent_to_aura_level(self.cached_brightness.value());
        let _ = self.write_sysfs_brightness(level as u32);

        let bright_pkt = build_brightness_packet(level);
        let color_pkt = build_static_color_packet(
            self.cached_color.r,
            self.cached_color.g,
            self.cached_color.b,
        );
        let set_pkt = build_set_packet();
        let apply_pkt = build_apply_packet();

        let _ = self.backend.send_packet(&bright_pkt);
        let _ = self.backend.send_packet(&color_pkt);
        let _ = self.backend.send_packet(&set_pkt);
        let _ = self.backend.send_packet(&apply_pkt);
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
    fn test_packet_invariants_length_and_report_id() {
        let static_pkt = build_static_color_packet(255, 128, 0);
        assert_eq!(static_pkt.len(), 64);
        assert_eq!(static_pkt[0], REPORT_ID);
        assert_eq!(static_pkt[1], CMD_BUILTIN_MODE);
        assert_eq!(static_pkt[2], 0x00); // Zone
        assert_eq!(static_pkt[3], 0x00); // Mode static
        assert_eq!(static_pkt[4], 255);
        assert_eq!(static_pkt[5], 128);
        assert_eq!(static_pkt[6], 0);
        // Trailing padding must be zero
        assert!(static_pkt[9..64].iter().all(|&b| b == 0));

        let set_pkt = build_set_packet();
        assert_eq!(set_pkt.len(), 64);
        assert_eq!(set_pkt[0], REPORT_ID);
        assert_eq!(set_pkt[1], CMD_SET);
        assert!(set_pkt[2..64].iter().all(|&b| b == 0));

        let apply_pkt = build_apply_packet();
        assert_eq!(apply_pkt.len(), 64);
        assert_eq!(apply_pkt[0], REPORT_ID);
        assert_eq!(apply_pkt[1], CMD_APPLY);
        assert!(apply_pkt[2..64].iter().all(|&b| b == 0));

        let bright_pkt = build_brightness_packet(2);
        assert_eq!(bright_pkt.len(), 64);
        assert_eq!(bright_pkt[0], REPORT_ID);
        assert_eq!(bright_pkt[1], CMD_BRIGHTNESS);
        assert_eq!(bright_pkt[2], 0xc5);
        assert_eq!(bright_pkt[3], 0xc4);
        assert_eq!(bright_pkt[4], 2);
        assert!(bright_pkt[5..64].iter().all(|&b| b == 0));
    }

    #[test]
    fn test_direct_color_packet_4zones() {
        let pkt = build_direct_color_packet(10, 20, 30, 4);
        assert_eq!(pkt.len(), 64);
        assert_eq!(pkt[0], REPORT_ID);
        assert_eq!(pkt[1], CMD_DIRECT_COLOR);
        assert_eq!(pkt[4], 0x04);
        // Zone 1
        assert_eq!(pkt[9], 10);
        assert_eq!(pkt[10], 20);
        assert_eq!(pkt[11], 30);
        // Zone 4
        assert_eq!(pkt[18], 10);
        assert_eq!(pkt[19], 20);
        assert_eq!(pkt[20], 30);
        assert!(pkt[21..64].iter().all(|&b| b == 0));
    }

    #[test]
    fn test_brightness_scaling() {
        assert_eq!(percent_to_aura_level(0), 0);
        assert_eq!(percent_to_aura_level(15), 1);
        assert_eq!(percent_to_aura_level(33), 1);
        assert_eq!(percent_to_aura_level(50), 2);
        assert_eq!(percent_to_aura_level(66), 2);
        assert_eq!(percent_to_aura_level(80), 3);
        assert_eq!(percent_to_aura_level(100), 3);

        assert_eq!(aura_level_to_percent(0), 0);
        assert_eq!(aura_level_to_percent(1), 33);
        assert_eq!(aura_level_to_percent(2), 66);
        assert_eq!(aura_level_to_percent(3), 100);
    }

    #[test]
    fn test_driver_mock_color_commit_sequence() {
        let (mut driver, packets) = AuraHidDriver::new_mock();
        let color = ColorRgb::new(255, 0, 128);

        driver.set_color(color).unwrap();

        let logged = packets.lock().unwrap().clone();
        assert_eq!(logged.len(), 3);
        assert_eq!(logged[0][1], CMD_BUILTIN_MODE);
        assert_eq!(logged[0][4], 255);
        assert_eq!(logged[0][5], 0);
        assert_eq!(logged[0][6], 128);
        assert_eq!(logged[1][1], CMD_SET);
        assert_eq!(logged[2][1], CMD_APPLY);
    }

    #[test]
    fn test_driver_mock_brightness_and_sleep_wake() {
        let (mut driver, packets) = AuraHidDriver::new_mock();

        driver
            .set_brightness(BrightnessPercent::new(50).unwrap())
            .unwrap();
        {
            let logged = packets.lock().unwrap().clone();
            assert_eq!(logged.len(), 3);
            assert_eq!(logged[0][1], CMD_BRIGHTNESS);
            assert_eq!(logged[0][4], 2); // 50% -> level 2
            assert_eq!(logged[1][1], CMD_SET);
            assert_eq!(logged[2][1], CMD_APPLY);
        }

        // Turn off
        packets.lock().unwrap().clear();
        driver.turn_off().unwrap();
        assert!(driver.is_sleeping());
        {
            let logged = packets.lock().unwrap().clone();
            assert_eq!(logged.len(), 3);
            assert_eq!(logged[0][1], CMD_BRIGHTNESS);
            assert_eq!(logged[0][4], 0); // 0 level
        }

        // Color updates while sleeping do not emit packets
        packets.lock().unwrap().clear();
        driver.set_color(ColorRgb::new(0, 255, 0)).unwrap();
        assert_eq!(packets.lock().unwrap().len(), 0);

        // Wake restores state
        packets.lock().unwrap().clear();
        driver.wake().unwrap();
        assert!(!driver.is_sleeping());
        {
            let logged = packets.lock().unwrap().clone();
            assert_eq!(logged.len(), 4);
            assert_eq!(logged[0][1], CMD_BRIGHTNESS);
            assert_eq!(logged[0][4], 2); // restored 50%
            assert_eq!(logged[1][1], CMD_BUILTIN_MODE);
            assert_eq!(logged[1][5], 255); // green color
            assert_eq!(logged[2][1], CMD_SET);
            assert_eq!(logged[3][1], CMD_APPLY);
        }
    }

    #[test]
    fn test_parse_uevent_ids() {
        let sample = "DRIVER=hid-generic\nHID_ID=0003:00000B05:000019B6\nHID_NAME=N-KEY Device\n";
        let (vid, pid) = AuraHidDriver::parse_uevent_ids(sample).unwrap();
        assert_eq!(vid, 0x0b05);
        assert_eq!(pid, 0x19b6);

        let invalid = "DRIVER=hid-generic\nSOME_OTHER_LINE=123\n";
        assert!(AuraHidDriver::parse_uevent_ids(invalid).is_none());
    }

    #[test]
    fn test_probe_with_missing_dir_returns_none() {
        let non_existent = Path::new("/tmp/nonexistent_hidraw_class_dir_9999");
        let res =
            AuraHidDriver::probe_with_paths(non_existent, Path::new("/dev"), Path::new("/tmp"));
        assert!(matches!(res, Ok(None)));
    }
}
