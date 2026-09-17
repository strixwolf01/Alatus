// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::domain::ThermalMode;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Structured diagnostics identifying why a hardware capability is unavailable on the host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnavailableReason {
    /// The required kernel module or user-space driver is missing.
    DriverMissing,
    /// The expected kernel sysfs, hidraw, or debugfs interface is absent.
    KernelInterfaceMissing,
    /// Insufficient OS privileges to communicate with the hardware node.
    PermissionDenied,
    /// Hardware discovery or protocol handshake failed during probing.
    ProbeFailed,
    /// Unhandled platform hardware error with diagnostic detail.
    HardwareError(String),
}

impl fmt::Display for UnavailableReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DriverMissing => write!(f, "Driver missing"),
            Self::KernelInterfaceMissing => write!(f, "Kernel interface missing"),
            Self::PermissionDenied => write!(f, "Permission denied"),
            Self::ProbeFailed => write!(f, "Hardware probe failed"),
            Self::HardwareError(msg) => write!(f, "Hardware error: {msg}"),
        }
    }
}

/// Tri-state capability model separating hardware existence from operational availability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "details", rename_all = "snake_case")]
pub enum CapabilityState<T> {
    /// Supported and fully operational on this machine.
    Supported(T),
    /// Permanently absent from or unsupported by this physical machine.
    Unsupported,
    /// Present in platform hardware, but temporarily or configurationally unavailable.
    Unavailable(UnavailableReason),
}

impl<T> CapabilityState<T> {
    /// Returns `true` if the feature is currently supported and operational.
    pub fn is_supported(&self) -> bool {
        matches!(self, Self::Supported(_))
    }

    /// Returns `true` if the feature is unsupported by this platform hardware.
    pub fn is_unsupported(&self) -> bool {
        matches!(self, Self::Unsupported)
    }

    /// Returns `true` if the feature exists on this platform but is currently unavailable.
    pub fn is_unavailable(&self) -> bool {
        matches!(self, Self::Unavailable(_))
    }

    /// Returns a reference to the inner capability details if supported.
    pub fn as_supported(&self) -> Option<&T> {
        match self {
            Self::Supported(val) => Some(val),
            _ => None,
        }
    }

    /// Returns a mutable reference to the inner capability details if supported.
    pub fn as_supported_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::Supported(val) => Some(val),
            _ => None,
        }
    }

    /// Returns the reason for unavailability, if in the `Unavailable` state.
    pub fn unavailable_reason(&self) -> Option<&UnavailableReason> {
        match self {
            Self::Unavailable(reason) => Some(reason),
            _ => None,
        }
    }
}

/// Probed hardware capabilities for keyboard RGB illumination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RgbCapabilityDetails {
    pub max_brightness: u8,
    pub supports_custom_color: bool,
    pub supports_inactivity_timeout: bool,
    pub supported_zones: Vec<String>,
}

/// Probed hardware capabilities for thermal and fan management.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThermalCapabilityDetails {
    pub supported_modes: Vec<ThermalMode>,
    pub fan_count: u32,
    pub supports_fan_telemetry: bool,
}

/// Probed hardware capabilities for battery charging controls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatteryCapabilityDetails {
    pub min_threshold: u8,
    pub max_threshold: u8,
    pub supports_charge_threshold: bool,
}

/// Probed hardware capabilities for laptop display management.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisplayCapabilityDetails {
    pub supports_flicker_free_dimming: bool,
    pub supported_refresh_rates: Vec<u32>,
}

/// Versioned platform capabilities model reporting hardware support state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemCapabilities {
    /// Schema version for wire and storage backward-compatibility.
    pub schema_version: u32,
    pub rgb: CapabilityState<RgbCapabilityDetails>,
    pub thermal: CapabilityState<ThermalCapabilityDetails>,
    pub battery: CapabilityState<BatteryCapabilityDetails>,
    pub display: CapabilityState<DisplayCapabilityDetails>,
}

impl Default for SystemCapabilities {
    fn default() -> Self {
        Self {
            schema_version: 1,
            rgb: CapabilityState::Unsupported,
            thermal: CapabilityState::Unsupported,
            battery: CapabilityState::Unsupported,
            display: CapabilityState::Unsupported,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_state_helpers() {
        let supported: CapabilityState<u32> = CapabilityState::Supported(42);
        assert!(supported.is_supported());
        assert!(!supported.is_unsupported());
        assert!(!supported.is_unavailable());
        assert_eq!(supported.as_supported(), Some(&42));
        assert_eq!(supported.unavailable_reason(), None);

        let unsupported: CapabilityState<u32> = CapabilityState::Unsupported;
        assert!(!unsupported.is_supported());
        assert!(unsupported.is_unsupported());
        assert!(!unsupported.is_unavailable());
        assert_eq!(unsupported.as_supported(), None);

        let unavailable: CapabilityState<u32> =
            CapabilityState::Unavailable(UnavailableReason::PermissionDenied);
        assert!(!unavailable.is_supported());
        assert!(!unavailable.is_unsupported());
        assert!(unavailable.is_unavailable());
        assert_eq!(
            unavailable.unavailable_reason(),
            Some(&UnavailableReason::PermissionDenied)
        );
    }

    #[test]
    fn test_system_capabilities_json_serialization_roundtrip() {
        let caps = SystemCapabilities {
            schema_version: 1,
            rgb: CapabilityState::Supported(RgbCapabilityDetails {
                max_brightness: 100,
                supports_custom_color: true,
                supports_inactivity_timeout: true,
                supported_zones: vec!["keyboard".to_string()],
            }),
            thermal: CapabilityState::Supported(ThermalCapabilityDetails {
                supported_modes: vec![
                    ThermalMode::Quiet,
                    ThermalMode::Balanced,
                    ThermalMode::Performance,
                    ThermalMode::FullSpeed,
                ],
                fan_count: 2,
                supports_fan_telemetry: true,
            }),
            battery: CapabilityState::Unavailable(UnavailableReason::KernelInterfaceMissing),
            display: CapabilityState::Unsupported,
        };

        let json = serde_json::to_string_pretty(&caps).unwrap();
        let de: SystemCapabilities = serde_json::from_str(&json).unwrap();
        assert_eq!(caps, de);
    }

    #[test]
    fn test_system_capabilities_toml_serialization_roundtrip() {
        let caps = SystemCapabilities {
            schema_version: 1,
            rgb: CapabilityState::Supported(RgbCapabilityDetails {
                max_brightness: 100,
                supports_custom_color: true,
                supports_inactivity_timeout: true,
                supported_zones: vec!["keyboard".to_string()],
            }),
            thermal: CapabilityState::Unsupported,
            battery: CapabilityState::Supported(BatteryCapabilityDetails {
                min_threshold: 50,
                max_threshold: 100,
                supports_charge_threshold: true,
            }),
            display: CapabilityState::Unavailable(UnavailableReason::DriverMissing),
        };

        let toml_str = toml::to_string_pretty(&caps).unwrap();
        let de: SystemCapabilities = toml::from_str(&toml_str).unwrap();
        assert_eq!(caps, de);
    }
}
