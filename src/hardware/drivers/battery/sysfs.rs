// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::domain::ChargeThreshold;
use crate::hardware::capabilities::UnavailableReason;
use crate::hardware::error::DriverError;
use crate::hardware::traits::BatteryDriver;
use std::fs;
use std::path::PathBuf;

const DEFAULT_SYSFS_CANDIDATES: &[&str] = &[
    "/sys/class/power_supply/BAT0/charge_control_end_threshold",
    "/sys/class/power_supply/BAT1/charge_control_end_threshold",
    "/sys/class/power_supply/BATC/charge_control_end_threshold",
    "/sys/class/power_supply/BATT/charge_control_end_threshold",
];

/// Battery driver managing charging limits via Linux kernel sysfs power supply nodes.
#[derive(Debug)]
pub struct SysfsBatteryDriver {
    threshold_path: Option<PathBuf>,
}

impl Default for SysfsBatteryDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl SysfsBatteryDriver {
    /// Auto-detects the active battery threshold sysfs node on the system.
    pub fn new() -> Self {
        let threshold_path = DEFAULT_SYSFS_CANDIDATES
            .iter()
            .map(PathBuf::from)
            .find(|p| p.exists());

        Self { threshold_path }
    }

    /// Constructs driver with an explicit sysfs threshold path for testing.
    pub fn with_path(path: PathBuf) -> Self {
        Self {
            threshold_path: Some(path),
        }
    }
}

impl BatteryDriver for SysfsBatteryDriver {
    fn set_charge_threshold(&mut self, threshold: ChargeThreshold) -> Result<(), DriverError> {
        let path = self
            .threshold_path
            .as_ref()
            .ok_or(DriverError::Unavailable(
                UnavailableReason::KernelInterfaceMissing,
            ))?;

        if !path.exists() {
            return Err(DriverError::Unavailable(
                UnavailableReason::KernelInterfaceMissing,
            ));
        }

        fs::write(path, format!("{}\n", threshold.value())).map_err(DriverError::Io)
    }

    fn get_charge_threshold(&self) -> Result<ChargeThreshold, DriverError> {
        let path = self
            .threshold_path
            .as_ref()
            .ok_or(DriverError::Unavailable(
                UnavailableReason::KernelInterfaceMissing,
            ))?;

        if !path.exists() {
            return Err(DriverError::Unavailable(
                UnavailableReason::KernelInterfaceMissing,
            ));
        }

        let raw = fs::read_to_string(path).map_err(DriverError::Io)?;
        let val = raw
            .trim()
            .parse::<u8>()
            .map_err(|e| DriverError::InvalidParameter(e.to_string()))?;

        ChargeThreshold::new(val).map_err(|e| DriverError::InvalidParameter(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_sysfs_battery_driver_read_and_write() {
        let tmp = tempdir().unwrap();
        let file = tmp.path().join("charge_control_end_threshold");
        fs::write(&file, "80\n").unwrap();

        let mut driver = SysfsBatteryDriver::with_path(file.clone());

        assert_eq!(driver.get_charge_threshold().unwrap().value(), 80);

        let new_thresh = ChargeThreshold::new(60).unwrap();
        driver.set_charge_threshold(new_thresh).unwrap();

        let written = fs::read_to_string(&file).unwrap();
        assert_eq!(written.trim(), "60");
        assert_eq!(driver.get_charge_threshold().unwrap().value(), 60);
    }

    #[test]
    fn test_sysfs_battery_driver_missing_file() {
        let tmp = tempdir().unwrap();
        let missing = tmp.path().join("missing_threshold");
        let mut driver = SysfsBatteryDriver::with_path(missing);

        assert!(matches!(
            driver.get_charge_threshold(),
            Err(DriverError::Unavailable(
                UnavailableReason::KernelInterfaceMissing
            ))
        ));

        assert!(matches!(
            driver.set_charge_threshold(ChargeThreshold::new(80).unwrap()),
            Err(DriverError::Unavailable(
                UnavailableReason::KernelInterfaceMissing
            ))
        ));
    }
}
