// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Clean-room implementation of ASUS ROG WMI hardware fan curve driver.
//!
//! Manages custom 8-point hardware fan curves via Linux hwmon sysfs attributes:
//! - Attribute match: `/sys/class/hwmon/hwmon*/name == "asus_custom_fan_curve"`
//! - Fans: `pwm1_` (CPU), `pwm2_` (GPU), `pwm3_` (Auxiliary / Mid)
//! - Points: Exactly 8 points (`pwm{fan}_auto_point{1..8}_pwm` and `pwm{fan}_auto_point{1..8}_temp`)
//! - Activation: `pwm{fan}_enable` ('1' for user curve, '2' for auto BIOS curve)

use crate::domain::ThermalMode;
use crate::hardware::capabilities::UnavailableReason;
use crate::hardware::error::DriverError;
use crate::hardware::traits::ThermalDriver;
use crate::services::firmware_mode::{self, FirmwareMode};
use crate::services::telemetry;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// A single coordinate point in a fan curve mapping temperature to PWM speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FanCurvePoint {
    /// Temperature threshold in degrees Celsius (0..=255).
    pub temp_c: u8,
    /// PWM fan speed value (0..=255, where 255 = 100% speed).
    pub pwm: u8,
}

impl FanCurvePoint {
    /// Creates a new curve point with raw temperature and PWM values.
    pub const fn new(temp_c: u8, pwm: u8) -> Self {
        Self { temp_c, pwm }
    }

    /// Converts the PWM value to an approximate percentage (0..=100%).
    pub fn pwm_percent(&self) -> u8 {
        ((self.pwm as u32 * 100 + 127) / 255).min(100) as u8
    }

    /// Creates a curve point from temperature and target speed percentage (0..=100%).
    pub fn from_percent(temp_c: u8, percent: u8) -> Self {
        let pwm = ((percent.min(100) as u32 * 255 + 50) / 100).min(255) as u8;
        Self { temp_c, pwm }
    }
}

/// An 8-point hardware fan curve.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FanCurve(pub [FanCurvePoint; 8]);

impl FanCurve {
    /// Validates monotonic progression across temperature and PWM speed.
    ///
    /// The ASUS EC firmware rejects fan curves unless:
    /// - `T_1 <= T_2 <= ... <= T_8`
    /// - `PWM_1 <= PWM_2 <= ... <= PWM_8`
    pub fn validate(&self) -> Result<(), DriverError> {
        for i in 0..7 {
            let p_curr = &self.0[i];
            let p_next = &self.0[i + 1];

            if p_curr.temp_c > p_next.temp_c {
                return Err(DriverError::InvalidParameter(format!(
                    "Non-monotonic temperature at point {} -> {}: {}C > {}C",
                    i + 1,
                    i + 2,
                    p_curr.temp_c,
                    p_next.temp_c
                )));
            }

            if p_curr.pwm > p_next.pwm {
                return Err(DriverError::InvalidParameter(format!(
                    "Non-monotonic fan PWM at point {} -> {}: {} > {}",
                    i + 1,
                    i + 2,
                    p_curr.pwm,
                    p_next.pwm
                )));
            }
        }
        Ok(())
    }
}

/// Supported platform profiles for ROG WMI.
const SUPPORTED_MODES: &[ThermalMode] = &[
    ThermalMode::Quiet,
    ThermalMode::Balanced,
    ThermalMode::Performance,
    ThermalMode::FullSpeed,
];

/// Type alias for the record of mock sysfs write operations.
pub type RogThermalWriteLog = Arc<Mutex<Vec<(String, String)>>>;

/// Communication backend for `RogWmiThermalDriver`.
#[derive(Debug)]
pub enum RogThermalBackend {
    /// Live kernel hwmon sysfs tree.
    Hwmon { hwmon_path: PathBuf },
    /// In-memory mock for headless testing.
    Mock {
        writes: RogThermalWriteLog,
        curves: Arc<Mutex<HashMap<u8, FanCurve>>>,
        enabled: Arc<Mutex<HashMap<u8, u8>>>,
        current_mode: Arc<Mutex<ThermalMode>>,
        simulate_failure: bool,
    },
}

impl RogThermalBackend {
    /// Writes a single point PWM value to `pwm{fan}_auto_point{point}_pwm`.
    pub fn write_pwm_point(&mut self, fan: u8, point: u8, pwm: u8) -> Result<(), DriverError> {
        let node_name = format!("pwm{fan}_auto_point{point}_pwm");
        match self {
            Self::Hwmon { hwmon_path } => {
                let p = hwmon_path.join(&node_name);
                std::fs::write(p.as_path(), format!("{pwm}\n")).map_err(|e| {
                    DriverError::Io(std::io::Error::new(
                        e.kind(),
                        format!("Failed to write {}: {e}", p.display()),
                    ))
                })
            }
            Self::Mock {
                writes,
                simulate_failure,
                ..
            } => {
                if *simulate_failure {
                    return Err(DriverError::Communication(
                        "Simulated ROG thermal failure".into(),
                    ));
                }
                let mut guard = writes.lock().map_err(|_| {
                    DriverError::Unavailable(UnavailableReason::HardwareError(
                        "Mock lock poisoned".into(),
                    ))
                })?;
                guard.push((node_name, pwm.to_string()));
                Ok(())
            }
        }
    }

    /// Writes a single point temperature value to `pwm{fan}_auto_point{point}_temp`.
    pub fn write_temp_point(&mut self, fan: u8, point: u8, temp: u8) -> Result<(), DriverError> {
        let node_name = format!("pwm{fan}_auto_point{point}_temp");
        match self {
            Self::Hwmon { hwmon_path } => {
                let p = hwmon_path.join(&node_name);
                std::fs::write(p.as_path(), format!("{temp}\n")).map_err(|e| {
                    DriverError::Io(std::io::Error::new(
                        e.kind(),
                        format!("Failed to write {}: {e}", p.display()),
                    ))
                })
            }
            Self::Mock {
                writes,
                simulate_failure,
                ..
            } => {
                if *simulate_failure {
                    return Err(DriverError::Communication(
                        "Simulated ROG thermal failure".into(),
                    ));
                }
                let mut guard = writes.lock().map_err(|_| {
                    DriverError::Unavailable(UnavailableReason::HardwareError(
                        "Mock lock poisoned".into(),
                    ))
                })?;
                guard.push((node_name, temp.to_string()));
                Ok(())
            }
        }
    }

    /// Writes the fan curve activation gate to `pwm{fan}_enable`.
    pub fn write_enable(&mut self, fan: u8, enable: u8) -> Result<(), DriverError> {
        let node_name = format!("pwm{fan}_enable");
        match self {
            Self::Hwmon { hwmon_path } => {
                let p = hwmon_path.join(&node_name);
                std::fs::write(p.as_path(), format!("{enable}\n")).map_err(|e| {
                    DriverError::Io(std::io::Error::new(
                        e.kind(),
                        format!("Failed to write {}: {e}", p.display()),
                    ))
                })
            }
            Self::Mock {
                writes,
                enabled,
                simulate_failure,
                ..
            } => {
                if *simulate_failure {
                    return Err(DriverError::Communication(
                        "Simulated ROG thermal failure".into(),
                    ));
                }
                let mut guard_w = writes.lock().map_err(|_| {
                    DriverError::Unavailable(UnavailableReason::HardwareError(
                        "Mock lock poisoned".into(),
                    ))
                })?;
                guard_w.push((node_name, enable.to_string()));

                let mut guard_e = enabled.lock().map_err(|_| {
                    DriverError::Unavailable(UnavailableReason::HardwareError(
                        "Mock lock poisoned".into(),
                    ))
                })?;
                guard_e.insert(fan, enable);
                Ok(())
            }
        }
    }

    /// Reads real-time tachometer fan RPMs if exposed under this hwmon node.
    pub fn read_fan_rpms(&self) -> Result<Vec<u32>, DriverError> {
        match self {
            Self::Hwmon { hwmon_path } => {
                let mut speeds = Vec::new();
                for fan_idx in 1..=3 {
                    let p = hwmon_path.join(format!("fan{fan_idx}_input"));
                    if let Ok(s) = std::fs::read_to_string(p)
                        && let Ok(val) = s.trim().parse::<u32>()
                        && val > 0
                    {
                        speeds.push(val);
                    }
                }
                Ok(speeds)
            }
            Self::Mock {
                simulate_failure, ..
            } => {
                if *simulate_failure {
                    return Err(DriverError::Communication(
                        "Simulated ROG thermal failure".into(),
                    ));
                }
                Ok(vec![2400, 2600])
            }
        }
    }
}

/// Hardware driver for ASUS ROG laptops supporting custom 8-point fan curves via WMI/hwmon.
#[derive(Debug)]
pub struct RogWmiThermalDriver {
    backend: RogThermalBackend,
    fan_count: u8,
    cached_mode: ThermalMode,
    platform_profile_path: PathBuf,
    debugfs_base: PathBuf,
}

impl RogWmiThermalDriver {
    /// Probes the system for the `asus_custom_fan_curve` hwmon driver under `/sys/class/hwmon`.
    ///
    /// Returns `Ok(Some(driver))` if present and writable, or `Ok(None)` if absent or restricted.
    pub fn probe() -> Result<Option<Self>, DriverError> {
        Self::probe_with_hwmon_dir(Path::new("/sys/class/hwmon"))
    }

    /// Probes with custom hwmon directory path (useful for tests or mocking).
    pub fn probe_with_hwmon_dir(hwmon_dir: &Path) -> Result<Option<Self>, DriverError> {
        if !hwmon_dir.exists() {
            return Ok(None);
        }

        let entries = match std::fs::read_dir(hwmon_dir) {
            Ok(e) => e,
            Err(_) => return Ok(None),
        };

        for entry in entries.flatten() {
            let name_file = entry.path().join("name");
            if let Ok(name) = std::fs::read_to_string(&name_file)
                && name.trim() == "asus_custom_fan_curve"
            {
                let pwm1_enable = entry.path().join("pwm1_enable");
                    match std::fs::OpenOptions::new().write(true).open(&pwm1_enable) {
                        Ok(_) => {
                            let fan_count = if entry.path().join("pwm3_enable").exists() {
                                3
                            } else if entry.path().join("pwm2_enable").exists() {
                                2
                            } else {
                                1
                            };

                            tracing::info!(
                                "Discovered ROG WMI custom fan curves at {} (fans: {})",
                                entry.path().display(),
                                fan_count
                            );

                            return Ok(Some(Self {
                                backend: RogThermalBackend::Hwmon {
                                    hwmon_path: entry.path(),
                                },
                                fan_count,
                                cached_mode: ThermalMode::Balanced,
                                platform_profile_path: PathBuf::from(
                                    "/sys/firmware/acpi/platform_profile",
                                ),
                                debugfs_base: PathBuf::from(
                                    crate::services::firmware_mode::DEBUGFS_BASE,
                                ),
                            }));
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Found asus_custom_fan_curve at {} but cannot write: {}",
                                entry.path().display(),
                                e
                            );
                            return Ok(None);
                        }
                    }
                }
            }

            Ok(None)
    }

    /// Constructs a mock driver recording writes in an in-memory buffer.
    pub fn new_mock() -> (Self, RogThermalWriteLog) {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let curves = Arc::new(Mutex::new(HashMap::new()));
        let enabled = Arc::new(Mutex::new(HashMap::new()));
        let current_mode = Arc::new(Mutex::new(ThermalMode::Balanced));

        let driver = Self {
            backend: RogThermalBackend::Mock {
                writes: writes.clone(),
                curves,
                enabled,
                current_mode,
                simulate_failure: false,
            },
            fan_count: 2,
            cached_mode: ThermalMode::Balanced,
            platform_profile_path: PathBuf::from("/nonexistent"),
            debugfs_base: PathBuf::from("/nonexistent"),
        };
        (driver, writes)
    }

    /// Returns the number of controllable fans exposed by this hardware.
    pub fn fan_count(&self) -> u8 {
        self.fan_count
    }

    /// Applies a custom 8-point hardware fan curve to the designated fan index (`1..=3`).
    ///
    /// Strict hardware sequencing invariant:
    /// 1. Validates monotonic curve progression ($T_1 \le \dots \le T_8$ and $\text{PWM}_1 \le \dots \le \text{PWM}_8$).
    /// 2. Writes all 8 PWM speed points.
    /// 3. Writes all 8 Temperature points.
    /// 4. Writes `pwm{fan}_enable = 1` last to commit and activate the user curve.
    pub fn apply_custom_curve(
        &mut self,
        fan_index: u8,
        curve: &FanCurve,
    ) -> Result<(), DriverError> {
        if fan_index < 1 || fan_index > self.fan_count {
            return Err(DriverError::InvalidParameter(format!(
                "Fan index {fan_index} out of bounds (1..={})",
                self.fan_count
            )));
        }

        curve.validate()?;

        // 1. Write 8 PWM values
        for (i, point) in curve.0.iter().enumerate() {
            self.backend
                .write_pwm_point(fan_index, (i + 1) as u8, point.pwm)?;
        }

        // 2. Write 8 Temperature values
        for (i, point) in curve.0.iter().enumerate() {
            self.backend
                .write_temp_point(fan_index, (i + 1) as u8, point.temp_c)?;
        }

        // 3. Commit activation: write '1' to enable user custom curve
        self.backend.write_enable(fan_index, 1)?;

        Ok(())
    }

    /// Resets all controllable fans to firmware BIOS automatic management (`pwm{fan}_enable = 2`).
    pub fn reset_curves_to_auto(&mut self) -> Result<(), DriverError> {
        for fan in 1..=self.fan_count {
            self.backend.write_enable(fan, 2)?;
        }
        Ok(())
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

impl ThermalDriver for RogWmiThermalDriver {
    fn set_mode(&mut self, mode: ThermalMode) -> Result<(), DriverError> {
        self.cached_mode = mode;

        match &self.backend {
            RogThermalBackend::Hwmon { .. } => {
                // Check ACPI platform_profile first
                if self.platform_profile_path.exists() {
                    let s = match mode {
                        ThermalMode::Quiet => "quiet",
                        ThermalMode::Balanced => "balanced",
                        ThermalMode::Performance | ThermalMode::FullSpeed => "performance",
                    };
                    if std::fs::write(&self.platform_profile_path, format!("{s}\n")).is_ok() {
                        return Ok(());
                    }
                }

                // Fallback to WMI DebugFS interface
                if self.debugfs_base.exists() {
                    let fw = Self::domain_to_fw(mode);
                    return firmware_mode::set_firmware_mode_at(&self.debugfs_base, fw)
                        .map(|_| ())
                        .map_err(|e| DriverError::Communication(e.to_string()));
                }

                Ok(())
            }
            RogThermalBackend::Mock {
                current_mode,
                simulate_failure,
                ..
            } => {
                if *simulate_failure {
                    return Err(DriverError::Communication(
                        "Simulated ROG thermal failure".into(),
                    ));
                }
                let mut guard = current_mode.lock().map_err(|_| {
                    DriverError::Unavailable(UnavailableReason::HardwareError(
                        "Mock lock poisoned".into(),
                    ))
                })?;
                *guard = mode;
                Ok(())
            }
        }
    }

    fn get_mode(&self) -> Result<ThermalMode, DriverError> {
        match &self.backend {
            RogThermalBackend::Hwmon { .. } => {
                if self.platform_profile_path.exists()
                    && let Ok(content) = std::fs::read_to_string(&self.platform_profile_path)
                {
                    match content.trim() {
                        "quiet" | "low-power" => return Ok(ThermalMode::Quiet),
                        "balanced" => return Ok(ThermalMode::Balanced),
                        "performance" => return Ok(ThermalMode::Performance),
                        _ => {}
                    }
                }

                if self.debugfs_base.exists()
                    && let Ok(fw) = firmware_mode::read_firmware_mode_at(&self.debugfs_base)
                {
                    return Self::fw_to_domain(fw);
                }

                Ok(self.cached_mode)
            }
            RogThermalBackend::Mock {
                current_mode,
                simulate_failure,
                ..
            } => {
                if *simulate_failure {
                    return Err(DriverError::Communication(
                        "Simulated ROG thermal failure".into(),
                    ));
                }
                let guard = current_mode.lock().map_err(|_| {
                    DriverError::Unavailable(UnavailableReason::HardwareError(
                        "Mock lock poisoned".into(),
                    ))
                })?;
                Ok(*guard)
            }
        }
    }

    fn supported_modes(&self) -> &[ThermalMode] {
        SUPPORTED_MODES
    }

    fn read_fan_speeds(&self) -> Result<Vec<u32>, DriverError> {
        let direct_speeds = self.backend.read_fan_rpms()?;
        if !direct_speeds.is_empty() {
            return Ok(direct_speeds);
        }

        // Fallback to standard telemetry reader across /sys/class/hwmon
        let t = telemetry::read_thermal_telemetry_from(Path::new("/sys/class/hwmon"));
        let mut speeds = Vec::new();
        if t.fan_rpm > 0 {
            speeds.push(t.fan_rpm);
        }
        if let Some(r2) = t.fan2_rpm.filter(|&r| r > 0) {
            speeds.push(r2);
        }
        Ok(speeds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_test_curve() -> FanCurve {
        FanCurve([
            FanCurvePoint::new(30, 20),
            FanCurvePoint::new(40, 40),
            FanCurvePoint::new(50, 70),
            FanCurvePoint::new(60, 100),
            FanCurvePoint::new(70, 140),
            FanCurvePoint::new(80, 180),
            FanCurvePoint::new(90, 220),
            FanCurvePoint::new(100, 255),
        ])
    }

    #[test]
    fn test_monotonic_curve_validation_success() {
        let curve = valid_test_curve();
        assert!(curve.validate().is_ok());
    }

    #[test]
    fn test_monotonic_curve_validation_non_monotonic_temp() {
        let mut points = valid_test_curve().0;
        // Non-monotonic temp: point 3 (45C) is less than point 2 (50C)
        points[3] = FanCurvePoint::new(45, 100);
        let curve = FanCurve(points);
        let res = curve.validate();
        assert!(res.is_err());
        assert!(
            res.unwrap_err()
                .to_string()
                .contains("Non-monotonic temperature")
        );
    }

    #[test]
    fn test_monotonic_curve_validation_non_monotonic_pwm() {
        let mut points = valid_test_curve().0;
        // Non-monotonic pwm: point 5 (120) is less than point 4 (140)
        points[5] = FanCurvePoint::new(80, 120);
        let curve = FanCurve(points);
        let res = curve.validate();
        assert!(res.is_err());
        assert!(
            res.unwrap_err()
                .to_string()
                .contains("Non-monotonic fan PWM")
        );
    }

    #[test]
    fn test_curve_point_percent_conversions() {
        let p = FanCurvePoint::from_percent(50, 100);
        assert_eq!(p.pwm, 255);
        assert_eq!(p.pwm_percent(), 100);

        let p_half = FanCurvePoint::from_percent(50, 50);
        assert_eq!(p_half.pwm_percent(), 50);

        let p_zero = FanCurvePoint::from_percent(30, 0);
        assert_eq!(p_zero.pwm, 0);
        assert_eq!(p_zero.pwm_percent(), 0);
    }

    #[test]
    fn test_write_sequence_invariant_enable_is_last() {
        let (mut driver, writes) = RogWmiThermalDriver::new_mock();
        let curve = valid_test_curve();

        driver.apply_custom_curve(1, &curve).unwrap();

        let logged = writes.lock().unwrap().clone();
        // 8 PWM writes + 8 Temp writes + 1 Enable write = 17 total operations
        assert_eq!(logged.len(), 17);

        // First 8 writes must be pwm1_auto_point{1..8}_pwm
        for i in 0..8 {
            assert_eq!(logged[i].0, format!("pwm1_auto_point{}_pwm", i + 1));
            assert_eq!(logged[i].1, curve.0[i].pwm.to_string());
        }

        // Next 8 writes must be pwm1_auto_point{1..8}_temp
        for i in 0..8 {
            assert_eq!(logged[8 + i].0, format!("pwm1_auto_point{}_temp", i + 1));
            assert_eq!(logged[8 + i].1, curve.0[i].temp_c.to_string());
        }

        // The 17th write (index 16) MUST strictly be pwm1_enable with value '1'
        assert_eq!(logged[16].0, "pwm1_enable");
        assert_eq!(logged[16].1, "1");
    }

    #[test]
    fn test_reset_curves_to_auto() {
        let (mut driver, writes) = RogWmiThermalDriver::new_mock();
        driver.reset_curves_to_auto().unwrap();

        let logged = writes.lock().unwrap().clone();
        assert_eq!(logged.len(), 2); // 2 fans in mock
        assert_eq!(logged[0], ("pwm1_enable".to_string(), "2".to_string()));
        assert_eq!(logged[1], ("pwm2_enable".to_string(), "2".to_string()));
    }

    #[test]
    fn test_probe_missing_dir_returns_none() {
        let non_existent = Path::new("/tmp/nonexistent_rog_hwmon_dir_999");
        let res = RogWmiThermalDriver::probe_with_hwmon_dir(non_existent);
        assert!(matches!(res, Ok(None)));
    }
}
