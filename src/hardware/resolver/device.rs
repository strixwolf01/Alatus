// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Platform DMI matching and device profile discovery.

use crate::hardware::profile::{DeviceMeta, DeviceProfile, DeviceProfileCapabilities, DmiMatcher};
use std::path::Path;

const S5506MA_TOML: &str = include_str!("../../../assets/devices/s5506ma.toml");
const ZENBOOK_UM5302_TOML: &str = include_str!("../../../assets/devices/zenbook_um5302.toml");
const ROG_G14_TOML: &str = include_str!("../../../assets/devices/rog_g14.toml");

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

/// Resolves the device profile matching host DMI tables or falls back to generic profile.
pub fn resolve_profile_from_dmi(dmi_root: &Path) -> DeviceProfile {
    let profiles = builtin_profiles();
    let matched = DmiMatcher::read_dmi(dmi_root)
        .ok()
        .and_then(|(prod, board)| {
            DmiMatcher::match_profile(&profiles, &prod, board.as_deref()).cloned()
        });

    matched.unwrap_or_else(fallback_profile)
}
