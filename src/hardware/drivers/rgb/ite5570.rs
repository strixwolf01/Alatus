// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::domain::{BrightnessPercent, ColorRgb, RgbTimeoutPolicy, TimeoutDuration};
use crate::hardware::capabilities::UnavailableReason;
use crate::hardware::error::DriverError;
use crate::hardware::traits::RgbDriver;
use crate::services::alatus_rgb_wrapper::{self, RgbDevice};
use crate::services::rgb as rgb_service;
use std::fs;
use std::path::PathBuf;

/// Hardware driver for ASUS ITE5570 LampArray keyboard backlighting.
#[derive(Debug)]
pub struct Ite5570Driver {
    cached_color: ColorRgb,
    cached_brightness: BrightnessPercent,
    policy: RgbTimeoutPolicy,
    timeout: TimeoutDuration,
    sleeping: bool,
    device: Option<RgbDevice>,
    sysfs_backlight_path: PathBuf,
}

impl Default for Ite5570Driver {
    fn default() -> Self {
        Self::new()
    }
}

impl Ite5570Driver {
    /// Discovers host ITE5570 hardware nodes and initializes driver state.
    pub fn new() -> Self {
        let device = alatus_rgb_wrapper::discover().ok();

        Self {
            cached_color: ColorRgb::new(255, 255, 255),
            cached_brightness: BrightnessPercent::new(100).unwrap(),
            policy: RgbTimeoutPolicy::Always,
            timeout: TimeoutDuration::default(),
            sleeping: false,
            device,
            sysfs_backlight_path: PathBuf::from(rgb_service::SYS_KBD_BACKLIGHT),
        }
    }

    /// Constructs an `Ite5570Driver` with explicit configuration for testing or non-standard sysfs layouts.
    pub fn with_paths(device: Option<RgbDevice>, sysfs_backlight_path: PathBuf) -> Self {
        Self {
            cached_color: ColorRgb::new(255, 255, 255),
            cached_brightness: BrightnessPercent::new(100).unwrap(),
            policy: RgbTimeoutPolicy::Always,
            timeout: TimeoutDuration::default(),
            sleeping: false,
            device,
            sysfs_backlight_path,
        }
    }

    fn percent_to_sysfs(pct: u8) -> u32 {
        match pct {
            0 => 0,
            1..=33 => 1,
            34..=66 => 2,
            _ => 3,
        }
    }

    fn sysfs_to_percent(sys: u32) -> u8 {
        match sys {
            0 => 0,
            1 => 33,
            2 => 66,
            _ => 100,
        }
    }

    fn write_sysfs_brightness(&self, sysfs_level: u32) -> Result<(), DriverError> {
        let brightness_file = self.sysfs_backlight_path.join("brightness");
        if !brightness_file.exists() {
            return Err(DriverError::Unavailable(
                UnavailableReason::KernelInterfaceMissing,
            ));
        }

        fs::write(&brightness_file, format!("{sysfs_level}\n")).map_err(DriverError::Io)
    }

    /// Refreshes the ITE5570 `/dev/hidraw*` device node handle.
    /// Useful after USB bus re-enumeration or system resume.
    pub fn re_enumerate(&mut self) {
        self.device = alatus_rgb_wrapper::discover().ok();
    }
}

impl RgbDriver for Ite5570Driver {
    fn set_color(&mut self, color: ColorRgb) -> Result<(), DriverError> {
        self.cached_color = color;

        // Perform WMI hardware unlock if available
        let _ = rgb_service::wmi_unlock();

        if self.device.is_none() {
            self.device = alatus_rgb_wrapper::discover().ok();
        }

        if let Some(ref dev) = self.device {
            alatus_rgb_wrapper::set_static(
                dev,
                color.r,
                color.g,
                color.b,
                self.cached_brightness.value() as u32,
            )
            .map_err(DriverError::Communication)?;
            return Ok(());
        }

        // If hidraw device is not found on this system
        Err(DriverError::Unavailable(
            UnavailableReason::KernelInterfaceMissing,
        ))
    }

    fn set_brightness(&mut self, brightness: BrightnessPercent) -> Result<(), DriverError> {
        self.cached_brightness = brightness;
        if self.sleeping {
            return Ok(());
        }

        let sysfs_val = Self::percent_to_sysfs(brightness.value());
        self.write_sysfs_brightness(sysfs_val)
    }

    fn get_brightness(&self) -> Result<BrightnessPercent, DriverError> {
        let brightness_file = self.sysfs_backlight_path.join("brightness");
        if let Some(parsed) = fs::read_to_string(brightness_file)
            .ok()
            .and_then(|c| c.trim().parse::<u32>().ok())
        {
            let pct = Self::sysfs_to_percent(parsed);
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
        Ok(())
    }

    fn wake(&mut self) -> Result<(), DriverError> {
        self.sleeping = false;

        // Perform WMI hardware unlock if available
        let _ = rgb_service::wmi_unlock();

        if self.device.is_none() {
            self.device = alatus_rgb_wrapper::discover().ok();
        }

        if let Some(ref dev) = self.device {
            // Wake handshake: switch out of firmware autonomous mode and apply active color
            let _ = alatus_rgb_wrapper::set_firmware_mode(dev, false);
            let _ = alatus_rgb_wrapper::set_static(
                dev,
                self.cached_color.r,
                self.cached_color.g,
                self.cached_color.b,
                self.cached_brightness.value() as u32,
            );
        }

        let sysfs_val = Self::percent_to_sysfs(self.cached_brightness.value());
        let _ = self.write_sysfs_brightness(sysfs_val);
        Ok(())
    }

    fn is_sleeping(&self) -> bool {
        self.sleeping
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_ite5570_driver_mock_sysfs() {
        let tmp = tempdir().unwrap();
        let brightness_file = tmp.path().join("brightness");
        fs::write(&brightness_file, "3\n").unwrap();

        let mut driver = Ite5570Driver::with_paths(None, tmp.path().to_path_buf());

        assert_eq!(driver.get_brightness().unwrap().value(), 100);

        driver
            .set_brightness(BrightnessPercent::new(50).unwrap())
            .unwrap();
        let readback = fs::read_to_string(&brightness_file).unwrap();
        assert_eq!(readback.trim(), "2");

        driver.turn_off().unwrap();
        assert!(driver.is_sleeping());
        let readback_off = fs::read_to_string(&brightness_file).unwrap();
        assert_eq!(readback_off.trim(), "0");

        driver.wake().unwrap();
        assert!(!driver.is_sleeping());
        let readback_wake = fs::read_to_string(&brightness_file).unwrap();
        assert_eq!(readback_wake.trim(), "2");
    }

    #[test]
    fn test_ite5570_driver_missing_sysfs() {
        let tmp = tempdir().unwrap();
        let non_existent = tmp.path().join("non_existent");
        let mut driver = Ite5570Driver::with_paths(None, non_existent);

        assert!(matches!(
            driver.set_brightness(BrightnessPercent::new(50).unwrap()),
            Err(DriverError::Unavailable(
                UnavailableReason::KernelInterfaceMissing
            ))
        ));
    }
}
