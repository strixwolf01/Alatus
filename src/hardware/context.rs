// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::hardware::capabilities::{
    BatteryCapabilityDetails, CapabilityState, DisplayCapabilityDetails, RgbCapabilityDetails,
    SystemCapabilities, ThermalCapabilityDetails, UnavailableReason,
};
use crate::hardware::drivers::{
    AsusWmiDriver, Ite5570Driver, OledDisplayDriver, SysfsBatteryDriver,
};
use crate::hardware::error::DriverError;
use crate::hardware::profile::{DeviceMeta, DeviceProfile, DeviceProfileCapabilities, DmiMatcher};
use crate::hardware::traits::{BatteryDriver, DisplayDriver, RgbDriver, ThermalDriver};
use std::path::Path;

const S5506MA_TOML: &str = include_str!("../../assets/devices/s5506ma.toml");
const ZENBOOK_UM5302_TOML: &str = include_str!("../../assets/devices/zenbook_um5302.toml");
const ROG_G14_TOML: &str = include_str!("../../assets/devices/rog_g14.toml");

/// Returns the compiled-in device profiles.
pub fn builtin_profiles() -> Vec<DeviceProfile> {
    let mut profiles = Vec::new();
    if let Ok(p) = DeviceProfile::from_toml_str(S5506MA_TOML) {
        profiles.push(p);
    }
    if let Ok(p) = DeviceProfile::from_toml_str(ZENBOOK_UM5302_TOML) {
        profiles.push(p);
    }
    if let Ok(p) = DeviceProfile::from_toml_str(ROG_G14_TOML) {
        profiles.push(p);
    }
    profiles
}

/// Fallback profile for unrecognized ASUS laptop platforms.
pub fn fallback_profile() -> DeviceProfile {
    DeviceProfile {
        device: DeviceMeta {
            name: "Generic ASUS Laptop".to_string(),
            vendor: "ASUSTeK COMPUTER INC.".to_string(),
            match_product: vec!["*".to_string()],
            match_board: None,
        },
        capabilities: DeviceProfileCapabilities {
            rgb: Some(crate::hardware::profile::model::RgbProfileConfig {
                driver: "ite5570".to_string(),
                zones: 1,
                supports_timeout: true,
                default_timeout_policy: Some("Always".to_string()),
            }),
            thermal: Some(crate::hardware::profile::model::ThermalProfileConfig {
                driver: "asus_wmi_debugfs".to_string(),
                profiles: vec![
                    "quiet".to_string(),
                    "balanced".to_string(),
                    "performance".to_string(),
                    "full_speed".to_string(),
                ],
                has_fan_curve: false,
            }),
            battery: Some(crate::hardware::profile::model::BatteryProfileConfig {
                driver: "asus_charge_control".to_string(),
                sysfs_path: None,
            }),
            display: Some(crate::hardware::profile::model::DisplayProfileConfig {
                has_oled: true,
                supports_flicker_free: true,
                refresh_rates: vec![60, 120],
            }),
        },
    }
}

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
        let profiles = builtin_profiles();
        let matched = DmiMatcher::read_dmi(dmi_root)
            .ok()
            .and_then(|(prod, board)| {
                DmiMatcher::match_profile(&profiles, &prod, board.as_deref()).cloned()
            });

        let profile = matched.unwrap_or_else(fallback_profile);
        Self::from_profile(profile)
    }

    /// Initializes hardware drivers according to the declared profile capabilities.
    pub fn from_profile(profile: DeviceProfile) -> Self {
        let mut capabilities = SystemCapabilities {
            schema_version: 1,
            rgb: CapabilityState::Unsupported,
            thermal: CapabilityState::Unsupported,
            battery: CapabilityState::Unsupported,
            display: CapabilityState::Unsupported,
        };

        // 1. RGB Subsystem
        let rgb: Option<Box<dyn RgbDriver>> = match &profile.capabilities.rgb {
            None => {
                capabilities.rgb = CapabilityState::Unsupported;
                None
            }
            Some(rgb_cfg) => {
                let has_hid = crate::services::alatus_rgb_wrapper::discover().is_ok();
                let has_sysfs = Path::new(crate::services::rgb::SYS_KBD_BACKLIGHT)
                    .join("brightness")
                    .exists();

                if has_hid || has_sysfs {
                    capabilities.rgb = CapabilityState::Supported(RgbCapabilityDetails {
                        max_brightness: 100,
                        supports_custom_color: true,
                        supports_inactivity_timeout: rgb_cfg.supports_timeout,
                        supported_zones: vec!["keyboard".to_string()],
                    });
                    Some(Box::new(Ite5570Driver::new()))
                } else {
                    capabilities.rgb =
                        CapabilityState::Unavailable(UnavailableReason::KernelInterfaceMissing);
                    None
                }
            }
        };

        // 2. Thermal Subsystem
        let thermal: Option<Box<dyn ThermalDriver>> = match &profile.capabilities.thermal {
            None => {
                capabilities.thermal = CapabilityState::Unsupported;
                None
            }
            Some(_th_cfg) => {
                let debugfs_path = Path::new(crate::services::firmware_mode::DEBUGFS_BASE);
                let platform_path = Path::new("/sys/devices/platform/asus-nb-wmi");

                if debugfs_path.exists() || platform_path.exists() {
                    let driver = AsusWmiDriver::new();
                    capabilities.thermal = CapabilityState::Supported(ThermalCapabilityDetails {
                        supported_modes: driver.supported_modes().to_vec(),
                        fan_count: 2,
                        supports_fan_telemetry: true,
                    });
                    Some(Box::new(driver))
                } else {
                    capabilities.thermal =
                        CapabilityState::Unavailable(UnavailableReason::KernelInterfaceMissing);
                    None
                }
            }
        };

        // 3. Battery Subsystem
        let battery: Option<Box<dyn BatteryDriver>> = match &profile.capabilities.battery {
            None => {
                capabilities.battery = CapabilityState::Unsupported;
                None
            }
            Some(_bat_cfg) => {
                let driver = SysfsBatteryDriver::new();
                if driver.get_charge_threshold().is_ok() {
                    capabilities.battery = CapabilityState::Supported(BatteryCapabilityDetails {
                        min_threshold: 50,
                        max_threshold: 100,
                        supports_charge_threshold: true,
                    });
                    Some(Box::new(driver))
                } else {
                    capabilities.battery =
                        CapabilityState::Unavailable(UnavailableReason::KernelInterfaceMissing);
                    None
                }
            }
        };

        // 4. Display Subsystem
        let display: Option<Box<dyn DisplayDriver>> = match &profile.capabilities.display {
            None => {
                capabilities.display = CapabilityState::Unsupported;
                None
            }
            Some(disp_cfg) => {
                let driver = OledDisplayDriver::with_refresh_rates(disp_cfg.refresh_rates.clone());
                capabilities.display = CapabilityState::Supported(DisplayCapabilityDetails {
                    supports_flicker_free_dimming: disp_cfg.supports_flicker_free,
                    supported_refresh_rates: disp_cfg.refresh_rates.clone(),
                });
                Some(Box::new(driver))
            }
        };

        Self {
            profile,
            capabilities,
            rgb,
            thermal,
            battery,
            display,
        }
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
            CapabilityState::Unsupported => Err(DriverError::Unsupported(
                "RGB illumination unsupported on this device".to_string(),
            )),
            CapabilityState::Unavailable(reason) => Err(DriverError::Unavailable(reason.clone())),
            CapabilityState::Supported(_) => self.rgb.as_deref_mut().ok_or_else(|| {
                DriverError::Unavailable(UnavailableReason::HardwareError(
                    "RGB driver missing".to_string(),
                ))
            }),
        }
    }

    /// Returns a mutable reference to the thermal driver if supported and operational.
    pub fn thermal_mut(&mut self) -> Result<&mut (dyn ThermalDriver + 'static), DriverError> {
        match &self.capabilities.thermal {
            CapabilityState::Unsupported => Err(DriverError::Unsupported(
                "Thermal management unsupported on this device".to_string(),
            )),
            CapabilityState::Unavailable(reason) => Err(DriverError::Unavailable(reason.clone())),
            CapabilityState::Supported(_) => self.thermal.as_deref_mut().ok_or_else(|| {
                DriverError::Unavailable(UnavailableReason::HardwareError(
                    "Thermal driver missing".to_string(),
                ))
            }),
        }
    }

    /// Returns a mutable reference to the battery driver if supported and operational.
    pub fn battery_mut(&mut self) -> Result<&mut (dyn BatteryDriver + 'static), DriverError> {
        match &self.capabilities.battery {
            CapabilityState::Unsupported => Err(DriverError::Unsupported(
                "Battery charge limit unsupported on this device".to_string(),
            )),
            CapabilityState::Unavailable(reason) => Err(DriverError::Unavailable(reason.clone())),
            CapabilityState::Supported(_) => self.battery.as_deref_mut().ok_or_else(|| {
                DriverError::Unavailable(UnavailableReason::HardwareError(
                    "Battery driver missing".to_string(),
                ))
            }),
        }
    }

    /// Returns a mutable reference to the display driver if supported and operational.
    pub fn display_mut(&mut self) -> Result<&mut (dyn DisplayDriver + 'static), DriverError> {
        match &self.capabilities.display {
            CapabilityState::Unsupported => Err(DriverError::Unsupported(
                "Display management unsupported on this device".to_string(),
            )),
            CapabilityState::Unavailable(reason) => Err(DriverError::Unavailable(reason.clone())),
            CapabilityState::Supported(_) => self.display.as_deref_mut().ok_or_else(|| {
                DriverError::Unavailable(UnavailableReason::HardwareError(
                    "Display driver missing".to_string(),
                ))
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{BrightnessPercent, ChargeThreshold, ColorRgb, ThermalMode};
    use crate::hardware::mock::{
        MockBatteryDriver, MockDisplayDriver, MockRgbDriver, MockThermalDriver,
    };
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
}
