// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::domain::ThermalMode;
use crate::hardware::capabilities::UnavailableReason;
use crate::hardware::error::DriverError;
use crate::hardware::traits::ThermalDriver;
use crate::services::firmware_mode::{self, FirmwareMode};
use crate::services::telemetry;
use std::path::PathBuf;

const SUPPORTED_MODES: &[ThermalMode] = &[
    ThermalMode::Quiet,
    ThermalMode::Balanced,
    ThermalMode::Performance,
    ThermalMode::FullSpeed,
];

/// Thermal driver for ASUS platform thermal curve management via WMI / DebugFS.
#[derive(Debug)]
pub struct AsusWmiDriver {
    debugfs_base: PathBuf,
    hwmon_base: PathBuf,
}

impl Default for AsusWmiDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl AsusWmiDriver {
    /// Constructs driver with standard Linux kernel debugfs and hwmon paths.
    pub fn new() -> Self {
        Self {
            debugfs_base: PathBuf::from(firmware_mode::DEBUGFS_BASE),
            hwmon_base: PathBuf::from("/sys/class/hwmon"),
        }
    }

    /// Constructs driver with custom paths for mocking or isolated unit testing.
    pub fn with_paths(debugfs_base: PathBuf, hwmon_base: PathBuf) -> Self {
        Self {
            debugfs_base,
            hwmon_base,
        }
    }

    fn domain_to_fw(mode: ThermalMode) -> FirmwareMode {
        match mode {
            ThermalMode::Quiet => FirmwareMode::Quiet,
            ThermalMode::Balanced => FirmwareMode::Balanced,
            ThermalMode::Performance => FirmwareMode::High,
            ThermalMode::FullSpeed => FirmwareMode::Full,
        }
    }

    fn fw_to_domain(fw: FirmwareMode) -> Result<ThermalMode, DriverError> {
        match fw {
            FirmwareMode::Quiet => Ok(ThermalMode::Quiet),
            FirmwareMode::Balanced => Ok(ThermalMode::Balanced),
            FirmwareMode::High => Ok(ThermalMode::Performance),
            FirmwareMode::Full => Ok(ThermalMode::FullSpeed),
            FirmwareMode::Unknown(n) => Err(DriverError::Unsupported(format!(
                "Unknown firmware thermal mode nibble: 0x{n:02x}"
            ))),
        }
    }
}

impl ThermalDriver for AsusWmiDriver {
    fn set_mode(&mut self, mode: ThermalMode) -> Result<(), DriverError> {
        if !self.debugfs_base.exists() {
            return Err(DriverError::Unavailable(
                UnavailableReason::KernelInterfaceMissing,
            ));
        }

        let fw_mode = Self::domain_to_fw(mode);
        firmware_mode::set_firmware_mode_at(&self.debugfs_base, fw_mode)
            .map(|_| ())
            .map_err(|e| DriverError::Communication(e.to_string()))
    }

    fn get_mode(&self) -> Result<ThermalMode, DriverError> {
        if !self.debugfs_base.exists() {
            return Err(DriverError::Unavailable(
                UnavailableReason::KernelInterfaceMissing,
            ));
        }

        let fw_mode = firmware_mode::read_firmware_mode_at(&self.debugfs_base)
            .map_err(|e| DriverError::Communication(e.to_string()))?;

        Self::fw_to_domain(fw_mode)
    }

    fn supported_modes(&self) -> &[ThermalMode] {
        SUPPORTED_MODES
    }

    fn read_fan_speeds(&self) -> Result<Vec<u32>, DriverError> {
        let telemetry = telemetry::read_thermal_telemetry_from(&self.hwmon_base);
        let mut speeds = Vec::new();
        if telemetry.fan_rpm > 0 {
            speeds.push(telemetry.fan_rpm);
        }
        if let Some(rpm2) = telemetry.fan2_rpm.filter(|&r| r > 0) {
            speeds.push(rpm2);
        }
        Ok(speeds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_asus_wmi_driver_missing_debugfs() {
        let tmp = tempdir().unwrap();
        let non_existent = tmp.path().join("missing_debugfs");
        let mut driver = AsusWmiDriver::with_paths(non_existent, tmp.path().to_path_buf());

        assert!(matches!(
            driver.set_mode(ThermalMode::Balanced),
            Err(DriverError::Unavailable(
                UnavailableReason::KernelInterfaceMissing
            ))
        ));

        assert!(matches!(
            driver.get_mode(),
            Err(DriverError::Unavailable(
                UnavailableReason::KernelInterfaceMissing
            ))
        ));
    }

    #[test]
    fn test_asus_wmi_driver_supported_modes() {
        let driver = AsusWmiDriver::new();
        assert_eq!(
            driver.supported_modes(),
            &[
                ThermalMode::Quiet,
                ThermalMode::Balanced,
                ThermalMode::Performance,
                ThermalMode::FullSpeed,
            ]
        );
    }
}
