// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Clean-room implementation of ASUS Armoury platform attributes driver.
//!
//! Controls CPU/GPU power limits (PPT / Dynamic Boost) and graphics/display switches
//! via `/sys/class/firmware-attributes/asus-armoury/attributes/`.

use crate::hardware::capabilities::UnavailableReason;
use crate::hardware::error::DriverError;
use crate::hardware::traits::PlatformPowerDriver;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Default sysfs root path for the `asus-armoury` kernel interface.
pub const ARMOURY_ATTRIBUTES_BASE: &str = "/sys/class/firmware-attributes/asus-armoury/attributes";

/// Represents an Armoury attribute node with hardware limits and resolution.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ArmouryAttribute {
    pub name: String,
    pub current_value: u32,
    pub min_value: u32,
    pub max_value: u32,
    pub default_value: u32,
    pub scalar_increment: u32,
}

impl ArmouryAttribute {
    /// Clamps a target value into the hardware safe operating window `[min_value, max_value]`.
    pub fn clamp_value(&self, val: u32) -> u32 {
        val.clamp(self.min_value, self.max_value)
    }

    /// Checks if a candidate value is within the hardware bounds `[min_value, max_value]`.
    pub fn is_within_bounds(&self, val: u32) -> bool {
        val >= self.min_value && val <= self.max_value
    }
}

/// In-memory representation of an attribute for mocking and headless unit tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockAttribute {
    pub current_value: u32,
    pub min_value: u32,
    pub max_value: u32,
    pub default_value: u32,
    pub scalar_increment: u32,
}

/// Record of write operations executed by the mock backend.
pub type ArmouryWriteLog = Arc<Mutex<Vec<(String, u32)>>>;

/// Communication backend for `ArmouryPlatformDriver`.
#[derive(Debug)]
pub enum ArmouryBackend {
    /// Live kernel sysfs attribute directory tree.
    Sysfs { base_path: PathBuf },
    /// In-memory mock for headless testing.
    Mock {
        attributes: Arc<Mutex<HashMap<String, MockAttribute>>>,
        write_log: ArmouryWriteLog,
        simulate_failure: bool,
    },
}

impl ArmouryBackend {
    /// Reads and parses an attribute from the backend.
    pub fn read_attribute(&self, name: &str) -> Result<ArmouryAttribute, DriverError> {
        match self {
            Self::Sysfs { base_path } => {
                let attr_dir = base_path.join(name);
                if !attr_dir.exists() {
                    return Err(DriverError::Unavailable(
                        UnavailableReason::KernelInterfaceMissing,
                    ));
                }

                let read_u32 = |file_name: &str, default_val: u32| -> u32 {
                    std::fs::read_to_string(attr_dir.join(file_name))
                        .ok()
                        .and_then(|s| s.trim().parse::<u32>().ok())
                        .unwrap_or(default_val)
                };

                let current_value = read_u32("current_value", 0);
                let min_value = read_u32("min_value", 0);
                let max_value = read_u32("max_value", 100);
                let default_value = read_u32("default_value", current_value);
                let scalar_increment = read_u32("scalar_increment", 1);

                Ok(ArmouryAttribute {
                    name: name.to_string(),
                    current_value,
                    min_value,
                    max_value,
                    default_value,
                    scalar_increment,
                })
            }
            Self::Mock {
                attributes,
                simulate_failure,
                ..
            } => {
                if *simulate_failure {
                    return Err(DriverError::Communication(
                        "Simulated Armoury failure".into(),
                    ));
                }
                let guard = attributes.lock().map_err(|_| {
                    DriverError::Unavailable(UnavailableReason::HardwareError(
                        "Mock lock poisoned".into(),
                    ))
                })?;
                let mock_attr = guard
                    .get(name)
                    .ok_or_else(|| DriverError::Unsupported(format!("Unknown attribute {name}")))?;

                Ok(ArmouryAttribute {
                    name: name.to_string(),
                    current_value: mock_attr.current_value,
                    min_value: mock_attr.min_value,
                    max_value: mock_attr.max_value,
                    default_value: mock_attr.default_value,
                    scalar_increment: mock_attr.scalar_increment,
                })
            }
        }
    }

    /// Writes a value to an attribute's `current_value` sub-node.
    pub fn write_attribute(&mut self, name: &str, value: u32) -> Result<(), DriverError> {
        match self {
            Self::Sysfs { base_path } => {
                let target = base_path.join(name).join("current_value");
                std::fs::write(target.as_path(), format!("{value}\n")).map_err(|e| {
                    DriverError::Io(std::io::Error::new(
                        e.kind(),
                        format!("Failed to write to {}: {e}", target.display()),
                    ))
                })
            }
            Self::Mock {
                attributes,
                write_log,
                simulate_failure,
            } => {
                if *simulate_failure {
                    return Err(DriverError::Communication(
                        "Simulated Armoury failure".into(),
                    ));
                }
                let mut guard = attributes.lock().map_err(|_| {
                    DriverError::Unavailable(UnavailableReason::HardwareError(
                        "Mock lock poisoned".into(),
                    ))
                })?;
                let mock_attr = guard
                    .get_mut(name)
                    .ok_or_else(|| DriverError::Unsupported(format!("Unknown attribute {name}")))?;
                mock_attr.current_value = value;

                let mut log_guard = write_log.lock().map_err(|_| {
                    DriverError::Unavailable(UnavailableReason::HardwareError(
                        "Mock lock poisoned".into(),
                    ))
                })?;
                log_guard.push((name.to_string(), value));
                Ok(())
            }
        }
    }

    /// Lists all attribute names exposed by the platform.
    pub fn list_attributes(&self) -> Vec<String> {
        match self {
            Self::Sysfs { base_path } => {
                let Ok(entries) = std::fs::read_dir(base_path) else {
                    return Vec::new();
                };
                let mut names: Vec<String> = entries
                    .flatten()
                    .filter(|e| e.path().is_dir())
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect();
                names.sort();
                names
            }
            Self::Mock { attributes, .. } => {
                let Ok(guard) = attributes.lock() else {
                    return Vec::new();
                };
                let mut names: Vec<String> = guard.keys().cloned().collect();
                names.sort();
                names
            }
        }
    }
}

/// Hardware driver for ASUS platform attributes (PPT, Dynamic Boost, GPU MUX).
#[derive(Debug)]
pub struct ArmouryPlatformDriver {
    backend: ArmouryBackend,
}

impl ArmouryPlatformDriver {
    /// Probes the system for the `asus-armoury` attributes interface.
    ///
    /// Returns `Ok(Some(driver))` if present and populated, or `Ok(None)` if absent.
    pub fn probe() -> Result<Option<Self>, DriverError> {
        Self::probe_with_path(Path::new(ARMOURY_ATTRIBUTES_BASE))
    }

    /// Probes a specific base path (useful for testing or mock sysfs environments).
    pub fn probe_with_path(base_path: &Path) -> Result<Option<Self>, DriverError> {
        if !base_path.exists() {
            return Ok(None);
        }

        let Ok(entries) = std::fs::read_dir(base_path) else {
            return Ok(None);
        };

        let mut has_entries = false;
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                has_entries = true;
                break;
            }
        }

        if !has_entries {
            return Ok(None);
        }

        tracing::info!(
            "Discovered asus-armoury firmware attributes at {}",
            base_path.display()
        );
        Ok(Some(Self {
            backend: ArmouryBackend::Sysfs {
                base_path: base_path.to_path_buf(),
            },
        }))
    }

    /// Constructs an in-memory mock driver for headless testing.
    pub fn new_mock() -> (Self, ArmouryWriteLog) {
        let mut map = HashMap::new();
        map.insert(
            "ppt_pl1_spl".to_string(),
            MockAttribute {
                current_value: 80,
                min_value: 35,
                max_value: 125,
                default_value: 80,
                scalar_increment: 1,
            },
        );
        map.insert(
            "ppt_pl2_sppt".to_string(),
            MockAttribute {
                current_value: 90,
                min_value: 45,
                max_value: 135,
                default_value: 90,
                scalar_increment: 1,
            },
        );
        map.insert(
            "ppt_fppt".to_string(),
            MockAttribute {
                current_value: 100,
                min_value: 55,
                max_value: 140,
                default_value: 100,
                scalar_increment: 1,
            },
        );
        map.insert(
            "nv_dynamic_boost".to_string(),
            MockAttribute {
                current_value: 25,
                min_value: 5,
                max_value: 25,
                default_value: 25,
                scalar_increment: 1,
            },
        );
        map.insert(
            "gpu_mux_mode".to_string(),
            MockAttribute {
                current_value: 1,
                min_value: 0,
                max_value: 1,
                default_value: 1,
                scalar_increment: 1,
            },
        );
        map.insert(
            "panel_od".to_string(),
            MockAttribute {
                current_value: 0,
                min_value: 0,
                max_value: 1,
                default_value: 0,
                scalar_increment: 1,
            },
        );

        let writes = Arc::new(Mutex::new(Vec::new()));
        let driver = Self {
            backend: ArmouryBackend::Mock {
                attributes: Arc::new(Mutex::new(map)),
                write_log: writes.clone(),
                simulate_failure: false,
            },
        };
        (driver, writes)
    }

    /// Reads an attribute structure.
    pub fn get_attribute(&self, name: &str) -> Result<ArmouryAttribute, DriverError> {
        self.backend.read_attribute(name)
    }

    /// Writes a value to an attribute, clamping strictly within hardware bounds.
    pub fn set_attribute(&mut self, name: &str, val: u32) -> Result<(), DriverError> {
        let attr = self.backend.read_attribute(name)?;
        let clamped = attr.clamp_value(val);
        self.backend.write_attribute(name, clamped)
    }

    /// Lists all available attribute names.
    pub fn list_attributes(&self) -> Vec<String> {
        self.backend.list_attributes()
    }

    // High-level CPU PPT helpers

    /// Reads CPU Sustained Power Limit (SPL / PL1) in Watts.
    pub fn get_cpu_spl(&self) -> Result<u32, DriverError> {
        self.get_attribute("ppt_pl1_spl").map(|a| a.current_value)
    }

    /// Commits CPU Sustained Power Limit (SPL / PL1) in Watts.
    pub fn set_cpu_spl(&mut self, watts: u32) -> Result<(), DriverError> {
        self.set_attribute("ppt_pl1_spl", watts)
    }

    /// Reads CPU Slow Package Power Tracking (SPPT / PL2) in Watts.
    pub fn get_cpu_sppt(&self) -> Result<u32, DriverError> {
        self.get_attribute("ppt_pl2_sppt").map(|a| a.current_value)
    }

    /// Commits CPU Slow Package Power Tracking (SPPT / PL2) in Watts.
    pub fn set_cpu_sppt(&mut self, watts: u32) -> Result<(), DriverError> {
        self.set_attribute("ppt_pl2_sppt", watts)
    }

    /// Reads CPU Fast Package Power Tracking (FPPT / PL3) in Watts.
    pub fn get_cpu_fppt(&self) -> Result<u32, DriverError> {
        self.get_attribute("ppt_fppt")
            .or_else(|_| self.get_attribute("ppt_pl3_fppt"))
            .map(|a| a.current_value)
    }

    /// Commits CPU Fast Package Power Tracking (FPPT / PL3) in Watts.
    pub fn set_cpu_fppt(&mut self, watts: u32) -> Result<(), DriverError> {
        if self.get_attribute("ppt_fppt").is_ok() {
            self.set_attribute("ppt_fppt", watts)
        } else {
            self.set_attribute("ppt_pl3_fppt", watts)
        }
    }

    // High-level GPU Boost helpers

    /// Reads NVIDIA Dynamic Boost power shift ceiling in Watts.
    pub fn get_gpu_dynamic_boost(&self) -> Result<u32, DriverError> {
        self.get_attribute("nv_dynamic_boost")
            .map(|a| a.current_value)
    }

    /// Commits NVIDIA Dynamic Boost power shift ceiling in Watts.
    pub fn set_gpu_dynamic_boost(&mut self, watts: u32) -> Result<(), DriverError> {
        self.set_attribute("nv_dynamic_boost", watts)
    }

    // High-level Platform switches

    /// Reads GPU MUX Mode (0 = Discrete, 1 = Optimus/Hybrid).
    pub fn get_gpu_mux_mode(&self) -> Result<u32, DriverError> {
        self.get_attribute("gpu_mux_mode").map(|a| a.current_value)
    }

    /// Commits GPU MUX Mode (0 = Discrete, 1 = Optimus/Hybrid).
    pub fn set_gpu_mux_mode(&mut self, mode: u32) -> Result<(), DriverError> {
        self.set_attribute("gpu_mux_mode", mode)
    }

    /// Reads panel overdrive status.
    pub fn get_panel_od(&self) -> Result<bool, DriverError> {
        self.get_attribute("panel_od").map(|a| a.current_value == 1)
    }

    /// Commits panel overdrive status (0 = Off, 1 = On).
    pub fn set_panel_od(&mut self, enabled: bool) -> Result<(), DriverError> {
        self.set_attribute("panel_od", if enabled { 1 } else { 0 })
    }
}

impl PlatformPowerDriver for ArmouryPlatformDriver {
    fn get_attribute(&self, name: &str) -> Result<ArmouryAttribute, DriverError> {
        self.get_attribute(name)
    }

    fn set_attribute(&mut self, name: &str, value: u32) -> Result<(), DriverError> {
        self.set_attribute(name, value)
    }

    fn list_attributes(&self) -> Vec<String> {
        self.list_attributes()
    }

    fn get_gpu_mux_mode(&self) -> Result<u32, DriverError> {
        self.get_gpu_mux_mode()
    }

    fn set_gpu_mux_mode(&mut self, mode: u32) -> Result<(), DriverError> {
        self.set_gpu_mux_mode(mode)
    }

    fn get_panel_od(&self) -> Result<bool, DriverError> {
        self.get_panel_od()
    }

    fn set_panel_od(&mut self, enabled: bool) -> Result<(), DriverError> {
        self.set_panel_od(enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attribute_clamping_logic() {
        let attr = ArmouryAttribute {
            name: "ppt_pl1_spl".to_string(),
            current_value: 80,
            min_value: 35,
            max_value: 125,
            default_value: 80,
            scalar_increment: 1,
        };

        // Value under minimum is clamped to min
        assert_eq!(attr.clamp_value(20), 35);
        // Value over maximum is clamped to max
        assert_eq!(attr.clamp_value(150), 125);
        // In-range value remains unchanged
        assert_eq!(attr.clamp_value(95), 95);

        assert!(!attr.is_within_bounds(20));
        assert!(!attr.is_within_bounds(150));
        assert!(attr.is_within_bounds(80));
    }

    #[test]
    fn test_mock_armoury_driver_ppt_writes_clamping() {
        let (mut driver, writes) = ArmouryPlatformDriver::new_mock();

        // Write value exceeding maximum (125W ceiling)
        driver.set_cpu_spl(200).unwrap();

        let logged = writes.lock().unwrap().clone();
        assert_eq!(logged.len(), 1);
        assert_eq!(logged[0].0, "ppt_pl1_spl");
        assert_eq!(logged[0].1, 125); // Clamped to 125!

        // Current value reflects clamped commit
        assert_eq!(driver.get_cpu_spl().unwrap(), 125);
    }

    #[test]
    fn test_mock_armoury_driver_gpu_mux_and_od() {
        let (mut driver, writes) = ArmouryPlatformDriver::new_mock();

        // Default MUX is 1 (Hybrid)
        assert_eq!(driver.get_gpu_mux_mode().unwrap(), 1);

        // Switch to 0 (Discrete)
        driver.set_gpu_mux_mode(0).unwrap();
        assert_eq!(driver.get_gpu_mux_mode().unwrap(), 0);

        // Turn on panel overdrive
        assert!(!driver.get_panel_od().unwrap());
        driver.set_panel_od(true).unwrap();
        assert!(driver.get_panel_od().unwrap());

        let logged = writes.lock().unwrap().clone();
        assert_eq!(logged.len(), 2);
        assert_eq!(logged[0], ("gpu_mux_mode".to_string(), 0));
        assert_eq!(logged[1], ("panel_od".to_string(), 1));
    }

    #[test]
    fn test_probe_missing_path() {
        let non_existent = Path::new("/tmp/nonexistent_armoury_path_9999");
        let res = ArmouryPlatformDriver::probe_with_path(non_existent);
        assert!(matches!(res, Ok(None)));
    }
}
