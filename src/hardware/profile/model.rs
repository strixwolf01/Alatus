// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use serde::{Deserialize, Serialize};

/// Declarative hardware device definition matching physical machine DMI metadata to driver capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceProfile {
    #[serde(alias = "match")]
    pub device: DeviceMeta,
    #[serde(default)]
    pub capabilities: DeviceProfileCapabilities,
}

impl DeviceProfile {
    /// Deserializes a `DeviceProfile` from a TOML document string.
    pub fn from_toml_str(s: &str) -> Result<Self, toml::de::Error> {
        #[derive(Deserialize)]
        struct RawProfile {
            #[serde(alias = "match")]
            device: DeviceMeta,
            #[serde(default)]
            capabilities: DeviceProfileCapabilities,
            #[serde(default)]
            rgb: Option<RgbProfileConfig>,
            #[serde(default)]
            thermal: Option<ThermalProfileConfig>,
            #[serde(default)]
            battery: Option<BatteryProfileConfig>,
            #[serde(default)]
            display: Option<DisplayProfileConfig>,
        }

        let raw: RawProfile = toml::from_str(s)?;
        let mut capabilities = raw.capabilities;
        if capabilities.rgb.is_none() {
            capabilities.rgb = raw.rgb;
        }
        if capabilities.thermal.is_none() {
            capabilities.thermal = raw.thermal;
        }
        if capabilities.battery.is_none() {
            capabilities.battery = raw.battery;
        }
        if capabilities.display.is_none() {
            capabilities.display = raw.display;
        }
        Ok(Self {
            device: raw.device,
            capabilities,
        })
    }

    /// Serializes the profile to a pretty TOML string.
    pub fn to_toml_string(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }
}

/// Identification and DMI match criteria for a hardware profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceMeta {
    #[serde(default = "default_device_name")]
    pub name: String,
    pub vendor: String,
    #[serde(alias = "product_name")]
    pub match_product: Vec<String>,
    #[serde(default, alias = "board_name", skip_serializing_if = "Option::is_none")]
    pub match_board: Option<Vec<String>>,
}

fn default_device_name() -> String {
    "ASUS Laptop".to_string()
}

fn default_true() -> bool {
    true
}

/// Declarative hardware subsystem driver bindings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DeviceProfileCapabilities {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rgb: Option<RgbProfileConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thermal: Option<ThermalProfileConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub battery: Option<BatteryProfileConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<DisplayProfileConfig>,
}

/// Profile configuration for keyboard RGB backlighting hardware.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RgbProfileConfig {
    pub driver: String,
    pub zones: u8,
    #[serde(default = "default_true")]
    pub supports_timeout: bool,
    #[serde(
        default,
        alias = "default_timeout_seconds",
        skip_serializing_if = "Option::is_none"
    )]
    pub default_timeout_seconds: Option<u32>,
    #[serde(
        default,
        alias = "default_timeout_policy",
        skip_serializing_if = "Option::is_none"
    )]
    pub default_timeout_policy: Option<String>,
}

/// Profile configuration for thermal curves and fan control.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThermalProfileConfig {
    pub driver: String,
    #[serde(alias = "modes")]
    pub profiles: Vec<String>,
    #[serde(default, alias = "has_fan_curves")]
    pub has_fan_curve: bool,
}

/// Profile configuration for battery charge limits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatteryProfileConfig {
    #[serde(default = "default_battery_driver")]
    pub driver: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sysfs_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub charge_control: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<[u32; 2]>,
}

fn default_battery_driver() -> String {
    "asus_charge_control".to_string()
}

/// Profile configuration for OLED and display characteristics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisplayProfileConfig {
    #[serde(default = "default_true", alias = "oled_care")]
    pub has_oled: bool,
    #[serde(default = "default_true", alias = "flicker_free_dimming")]
    pub supports_flicker_free: bool,
    pub refresh_rates: Vec<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_mux_mode: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panel_od: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_profile_toml_roundtrip() {
        let toml_content = r#"
[device]
name = "ASUS Vivobook S 15 OLED"
vendor = "ASUSTeK COMPUTER INC."
match_product = ["S5506MA", "Vivobook_ASUSLaptop_S5506MA"]
match_board = ["S5506MA"]

[capabilities.rgb]
driver = "ite5570"
zones = 1
supports_timeout = true
default_timeout_policy = "Always"

[capabilities.thermal]
driver = "asus_wmi_debugfs"
profiles = ["quiet", "balanced", "performance", "full_speed"]
has_fan_curve = false

[capabilities.display]
has_oled = true
supports_flicker_free = true
refresh_rates = [60, 120]

[capabilities.battery]
driver = "asus_charge_control"
"#;

        let profile: DeviceProfile = DeviceProfile::from_toml_str(toml_content).unwrap();
        assert_eq!(profile.device.name, "ASUS Vivobook S 15 OLED");
        assert_eq!(profile.device.vendor, "ASUSTeK COMPUTER INC.");
        assert_eq!(
            profile.device.match_product,
            vec!["S5506MA", "Vivobook_ASUSLaptop_S5506MA"]
        );
        assert_eq!(
            profile.device.match_board,
            Some(vec!["S5506MA".to_string()])
        );

        let rgb = profile.capabilities.rgb.as_ref().unwrap();
        assert_eq!(rgb.driver, "ite5570");
        assert_eq!(rgb.zones, 1);
        assert!(rgb.supports_timeout);
        assert_eq!(rgb.default_timeout_policy.as_deref(), Some("Always"));

        let thermal = profile.capabilities.thermal.as_ref().unwrap();
        assert_eq!(thermal.driver, "asus_wmi_debugfs");
        assert_eq!(
            thermal.profiles,
            vec!["quiet", "balanced", "performance", "full_speed"]
        );
        assert!(!thermal.has_fan_curve);

        let display = profile.capabilities.display.as_ref().unwrap();
        assert!(display.has_oled);
        assert!(display.supports_flicker_free);
        assert_eq!(display.refresh_rates, vec![60, 120]);

        let battery = profile.capabilities.battery.as_ref().unwrap();
        assert_eq!(battery.driver, "asus_charge_control");
        assert_eq!(battery.sysfs_path, None);

        // Verify serializing back to TOML produces an equivalent object
        let serialized = profile.to_toml_string().unwrap();
        let deserialized: DeviceProfile = DeviceProfile::from_toml_str(&serialized).unwrap();
        assert_eq!(profile, deserialized);
    }
}
