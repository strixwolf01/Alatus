// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Passive runtime container holding resolved drivers and dynamic capability states.

use crate::hardware::capabilities::SystemCapabilities;
use crate::hardware::drivers::AsusctlProxyDriver;
use crate::hardware::error::DriverError;
use crate::hardware::profile::DeviceProfile;
use crate::hardware::resolver::{DriverFactory, resolve_profile_from_dmi};
use crate::hardware::traits::{BatteryDriver, DisplayDriver, RgbDriver, ThermalDriver};
use std::path::Path;

pub use crate::hardware::resolver::{builtin_profiles, fallback_profile};

/// Single owner of host hardware platform drivers and dynamic capability states.
pub struct DeviceContext {
    pub profile: DeviceProfile,
    pub capabilities: SystemCapabilities,
    pub rgb: Option<Box<dyn RgbDriver>>,
    pub thermal: Option<Box<dyn ThermalDriver>>,
    pub battery: Option<Box<dyn BatteryDriver>>,
    pub display: Option<Box<dyn DisplayDriver>>,
}

impl Default for DeviceContext {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceContext {
    /// Probes system DMI hardware metadata and initializes platform drivers.
    pub fn new() -> Self {
        Self::new_with_dmi_root(Path::new("/sys/class/dmi/id"))
    }

    /// Probes system with a custom DMI root path for non-standard environments or testing.
    pub fn new_with_dmi_root(dmi_root: &Path) -> Self {
        let profile = resolve_profile_from_dmi(dmi_root);
        Self::from_profile(profile)
    }

    /// Initializes hardware drivers according to the declared profile capabilities.
    pub fn from_profile(profile: DeviceProfile) -> Self {
        DriverFactory::build_context(profile, None)
    }

    /// Initializes hardware drivers with an optional pre-configured fallback proxy.
    pub fn from_profile_with_fallback(
        profile: DeviceProfile,
        fallback_proxy: Option<AsusctlProxyDriver>,
    ) -> Self {
        DriverFactory::build_context(profile, fallback_proxy)
    }

    /// Builder for testing, mock injection, and custom capability configurations.
    pub fn with_drivers(
        profile: DeviceProfile,
        capabilities: SystemCapabilities,
        rgb: Option<Box<dyn RgbDriver>>,
        thermal: Option<Box<dyn ThermalDriver>>,
        battery: Option<Box<dyn BatteryDriver>>,
        display: Option<Box<dyn DisplayDriver>>,
    ) -> Self {
        Self {
            profile,
            capabilities,
            rgb,
            thermal,
            battery,
            display,
        }
    }

    /// Returns a mutable reference to the RGB driver if supported and operational.
    pub fn rgb_mut(&mut self) -> Result<&mut (dyn RgbDriver + 'static), DriverError> {
        match &self.capabilities.rgb {
            crate::hardware::capabilities::CapabilityState::Unsupported => Err(DriverError::Unsupported(
                "RGB illumination unsupported on this device".to_string(),
            )),
            crate::hardware::capabilities::CapabilityState::Unavailable(reason) => {
                Err(DriverError::Unavailable(reason.clone()))
            }
            crate::hardware::capabilities::CapabilityState::Supported(_) => self.rgb.as_deref_mut().ok_or_else(|| {
                DriverError::Unavailable(crate::hardware::capabilities::UnavailableReason::HardwareError(
                    "RGB driver missing".to_string(),
                ))
            }),
        }
    }

    /// Returns a mutable reference to the thermal driver if supported and operational.
    pub fn thermal_mut(&mut self) -> Result<&mut (dyn ThermalDriver + 'static), DriverError> {
        match &self.capabilities.thermal {
            crate::hardware::capabilities::CapabilityState::Unsupported => Err(DriverError::Unsupported(
                "Thermal management unsupported on this device".to_string(),
            )),
            crate::hardware::capabilities::CapabilityState::Unavailable(reason) => {
                Err(DriverError::Unavailable(reason.clone()))
            }
            crate::hardware::capabilities::CapabilityState::Supported(_) => {
                self.thermal.as_deref_mut().ok_or_else(|| {
                    DriverError::Unavailable(crate::hardware::capabilities::UnavailableReason::HardwareError(
                        "Thermal driver missing".to_string(),
                    ))
                })
            }
        }
    }

    /// Returns a mutable reference to the battery driver if supported and operational.
    pub fn battery_mut(&mut self) -> Result<&mut (dyn BatteryDriver + 'static), DriverError> {
        match &self.capabilities.battery {
            crate::hardware::capabilities::CapabilityState::Unsupported => Err(DriverError::Unsupported(
                "Battery charge limit unsupported on this device".to_string(),
            )),
            crate::hardware::capabilities::CapabilityState::Unavailable(reason) => {
                Err(DriverError::Unavailable(reason.clone()))
            }
            crate::hardware::capabilities::CapabilityState::Supported(_) => {
                self.battery.as_deref_mut().ok_or_else(|| {
                    DriverError::Unavailable(crate::hardware::capabilities::UnavailableReason::HardwareError(
                        "Battery driver missing".to_string(),
                    ))
                })
            }
        }
    }

    /// Returns a mutable reference to the display driver if supported and operational.
    pub fn display_mut(&mut self) -> Result<&mut (dyn DisplayDriver + 'static), DriverError> {
        match &self.capabilities.display {
            crate::hardware::capabilities::CapabilityState::Unsupported => Err(DriverError::Unsupported(
                "Display management unsupported on this device".to_string(),
            )),
            crate::hardware::capabilities::CapabilityState::Unavailable(reason) => {
                Err(DriverError::Unavailable(reason.clone()))
            }
            crate::hardware::capabilities::CapabilityState::Supported(_) => {
                self.display.as_deref_mut().ok_or_else(|| {
                    DriverError::Unavailable(crate::hardware::capabilities::UnavailableReason::HardwareError(
                        "Display driver missing".to_string(),
                    ))
                })
            }
        }
    }

    /// Re-probes the RGB backlighting controller and refreshes the driver instance
    /// while preserving active brightness if possible.
    pub fn re_enumerate_rgb(&mut self) {
        DriverFactory::re_enumerate_rgb(self);
    }

    /// Re-probes platform hardware drivers and refreshes capability state.
    pub fn re_enumerate(&mut self) {
        DriverFactory::re_enumerate(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{BrightnessPercent, ChargeThreshold, ColorRgb, ThermalMode};
    use crate::hardware::capabilities::{
        BatteryCapabilityDetails, CapabilityState, DisplayCapabilityDetails, RgbCapabilityDetails,
        ThermalCapabilityDetails,
    };
    use crate::hardware::mock::{
        MockBatteryDriver, MockDisplayDriver, MockRgbDriver, MockThermalDriver,
    };
    use crate::hardware::profile::{DeviceMeta, DeviceProfileCapabilities};
    use tempfile::tempdir;

    #[test]
    fn test_empty_profile_yields_unsupported() {
        let profile = DeviceProfile {
            device: DeviceMeta {
                name: "Empty Device".to_string(),
                vendor: "Test".to_string(),
                match_product: vec!["EMPTY".to_string()],
                match_board: None,
            },
            capabilities: DeviceProfileCapabilities::default(),
        };

        let mut ctx = DeviceContext::from_profile(profile);
        assert!(ctx.capabilities.rgb.is_unsupported());
        assert!(ctx.capabilities.thermal.is_unsupported());
        assert!(ctx.capabilities.battery.is_unsupported());
        assert!(ctx.capabilities.display.is_unsupported());

        assert!(matches!(ctx.rgb_mut(), Err(DriverError::Unsupported(_))));
        assert!(matches!(
            ctx.thermal_mut(),
            Err(DriverError::Unsupported(_))
        ));
        assert!(matches!(
            ctx.battery_mut(),
            Err(DriverError::Unsupported(_))
        ));
        assert!(matches!(
            ctx.display_mut(),
            Err(DriverError::Unsupported(_))
        ));
    }

    #[test]
    fn test_with_mock_drivers_lifecycle() {
        let profile = fallback_profile();
        let capabilities = SystemCapabilities {
            schema_version: 1,
            rgb: CapabilityState::Supported(RgbCapabilityDetails {
                max_brightness: 100,
                supports_custom_color: true,
                supports_inactivity_timeout: true,
                supported_zones: vec!["keyboard".to_string()],
            }),
            thermal: CapabilityState::Supported(ThermalCapabilityDetails {
                supported_modes: vec![ThermalMode::Quiet, ThermalMode::Balanced],
                fan_count: 2,
                supports_fan_telemetry: true,
            }),
            battery: CapabilityState::Supported(BatteryCapabilityDetails {
                min_threshold: 50,
                max_threshold: 100,
                supports_charge_threshold: true,
            }),
            display: CapabilityState::Supported(DisplayCapabilityDetails {
                supports_flicker_free_dimming: true,
                supported_refresh_rates: vec![60, 120],
            }),
        };

        let mock_rgb = MockRgbDriver::new();
        let mock_thermal = MockThermalDriver::new();
        let mock_battery = MockBatteryDriver::new();
        let mock_display = MockDisplayDriver::new();

        let mut ctx = DeviceContext::with_drivers(
            profile,
            capabilities,
            Some(Box::new(mock_rgb)),
            Some(Box::new(mock_thermal)),
            Some(Box::new(mock_battery)),
            Some(Box::new(mock_display)),
        );

        assert!(ctx.capabilities.rgb.is_supported());
        assert!(ctx.capabilities.thermal.is_supported());
        assert!(ctx.capabilities.battery.is_supported());
        assert!(ctx.capabilities.display.is_supported());

        // Test mutating RGB driver through context
        let rgb = ctx.rgb_mut().expect("RGB driver is available");
        assert!(rgb.set_color(ColorRgb::new(255, 0, 128)).is_ok());
        assert!(
            rgb.set_brightness(BrightnessPercent::new(75).unwrap())
                .is_ok()
        );
        assert_eq!(rgb.get_brightness().unwrap().value(), 75);

        // Test mutating Thermal driver through context
        let thermal = ctx.thermal_mut().expect("Thermal driver is available");
        assert!(thermal.set_mode(ThermalMode::Quiet).is_ok());
        assert_eq!(thermal.get_mode().unwrap(), ThermalMode::Quiet);

        // Test mutating Battery driver through context
        let battery = ctx.battery_mut().expect("Battery driver is available");
        assert_eq!(battery.get_charge_threshold().unwrap().value(), 80);
        assert!(
            battery
                .set_charge_threshold(ChargeThreshold::new(60).unwrap())
                .is_ok()
        );
        assert_eq!(battery.get_charge_threshold().unwrap().value(), 60);

        // Test mutating Display driver through context
        let display = ctx.display_mut().expect("Display driver is available");
        assert!(display.set_refresh_rate(60).is_ok());
        assert_eq!(display.supported_refresh_rates(), &[60, 120]);
    }

    #[test]
    fn test_dmi_matcher_integration_with_context() {
        let tmp = tempdir().unwrap();
        let dmi_dir = tmp.path();
        std::fs::write(dmi_dir.join("product_name"), "S5506MA\n").unwrap();
        std::fs::write(dmi_dir.join("board_name"), "S5506MA\n").unwrap();

        let ctx = DeviceContext::new_with_dmi_root(dmi_dir);
        assert_eq!(ctx.profile.device.name, "ASUS Vivobook S 15 OLED");
        assert_eq!(ctx.capabilities.schema_version, 1);
    }

    #[test]
    fn test_device_context_falls_back_to_mock_proxy_when_native_fails() {
        let mut profile = fallback_profile();
        if let Some(ref mut bat) = profile.capabilities.battery {
            bat.sysfs_path = Some(
                "/sys/class/power_supply/NONEXISTENT/charge_control_end_threshold".to_string(),
            );
        }
        if let Some(ref mut rgb) = profile.capabilities.rgb {
            rgb.driver = "asus_aura_hid".to_string();
        }

        let mock_proxy = AsusctlProxyDriver::new_mock();
        let mut ctx = DeviceContext::from_profile_with_fallback(profile, Some(mock_proxy));

        // When native nodes are absent/fail, DeviceContext should gracefully fallback to the mock proxy!
        assert!(ctx.capabilities.thermal.is_supported());
        assert!(ctx.capabilities.battery.is_supported());
        assert!(ctx.capabilities.rgb.is_supported());

        // Test mutating Thermal driver via proxy fallback
        let thermal = ctx
            .thermal_mut()
            .expect("Thermal driver should be available via proxy");
        assert!(thermal.set_mode(ThermalMode::Quiet).is_ok());
        assert_eq!(thermal.get_mode().unwrap(), ThermalMode::Quiet);

        // Test mutating Battery driver via proxy fallback
        let battery = ctx
            .battery_mut()
            .expect("Battery driver should be available via proxy");
        assert_eq!(battery.get_charge_threshold().unwrap().value(), 80);
        assert!(
            battery
                .set_charge_threshold(ChargeThreshold::new(65).unwrap())
                .is_ok()
        );
        assert_eq!(battery.get_charge_threshold().unwrap().value(), 65);

        // Test mutating RGB driver via proxy fallback
        let rgb = ctx
            .rgb_mut()
            .expect("RGB driver should be available via proxy");
        assert!(
            rgb.set_brightness(BrightnessPercent::new(50).unwrap())
                .is_ok()
        );
        assert_eq!(rgb.get_brightness().unwrap().value(), 66);
    }

    #[test]
    fn test_rog_g14_profile_falls_back_to_proxy() {
        let profiles = builtin_profiles();
        let g14 = profiles
            .iter()
            .find(|p| p.device.name == "ROG Zephyrus G14")
            .cloned()
            .expect("ROG G14 profile should exist");

        let mock_proxy = AsusctlProxyDriver::new_mock();
        let mut ctx = DeviceContext::from_profile_with_fallback(g14, Some(mock_proxy));

        // When asus_aura_hid is declared and native ITE is not used, RGB falls back to proxy!
        assert!(ctx.capabilities.rgb.is_supported());
        assert!(ctx.capabilities.thermal.is_supported());

        // Mutate RGB via proxy
        let rgb = ctx.rgb_mut().unwrap();
        assert!(
            rgb.set_brightness(BrightnessPercent::new(100).unwrap())
                .is_ok()
        );
    }

    #[test]
    fn test_device_context_re_enumerate() {
        let profile = fallback_profile();
        let mut ctx = DeviceContext::from_profile(profile.clone());
        let orig_cap = ctx.capabilities.clone();

        // Mutate capabilities to something else
        ctx.capabilities.thermal = CapabilityState::Unsupported;
        assert!(ctx.capabilities.thermal.is_unsupported());

        // Re-enumerate should restore capability matching profile probe
        ctx.re_enumerate();
        assert_eq!(ctx.capabilities.thermal, orig_cap.thermal);
    }

    #[test]
    fn test_device_context_re_enumerate_rgb() {
        let profile = fallback_profile();
        let mut ctx = DeviceContext::from_profile(profile);
        let orig_rgb = ctx.capabilities.rgb.clone();

        // Mutate RGB capability
        ctx.capabilities.rgb = CapabilityState::Unsupported;
        assert!(ctx.capabilities.rgb.is_unsupported());

        // Re-enumerate RGB should restore RGB state
        ctx.re_enumerate_rgb();
        assert_eq!(ctx.capabilities.rgb, orig_rgb);
    }
}
