// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use alatus::hardware::profile::{DeviceProfile, DmiMatcher};
use std::fs;
use std::path::Path;

#[test]
fn test_s5506ma_reference_profile_asset_parsing() {
    let asset_path = Path::new("assets/devices/s5506ma.toml");
    let content = fs::read_to_string(asset_path)
        .unwrap_or_else(|e| panic!("Failed to read asset {}: {e}", asset_path.display()));

    let profile: DeviceProfile = DeviceProfile::from_toml_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {e}", asset_path.display()));

    // Device metadata assertions
    assert_eq!(profile.device.name, "ASUS Vivobook S 15 OLED");
    assert_eq!(profile.device.vendor, "ASUSTeK COMPUTER INC.");
    assert!(
        profile
            .device
            .match_product
            .contains(&"S5506MA".to_string())
    );
    assert!(
        profile
            .device
            .match_product
            .contains(&"Vivobook_ASUSLaptop_S5506MA".to_string())
    );
    assert_eq!(
        profile.device.match_board,
        Some(vec!["S5506MA".to_string()])
    );

    // RGB capabilities
    let rgb = profile
        .capabilities
        .rgb
        .as_ref()
        .expect("RGB capability present");
    assert_eq!(rgb.driver, "ite5570");
    assert_eq!(rgb.zones, 1);
    assert!(rgb.supports_timeout);
    assert_eq!(rgb.default_timeout_policy.as_deref(), Some("Always"));

    // Thermal capabilities
    let thermal = profile
        .capabilities
        .thermal
        .as_ref()
        .expect("Thermal capability present");
    assert_eq!(thermal.driver, "asus_wmi_debugfs");
    assert_eq!(
        thermal.profiles,
        vec!["quiet", "balanced", "performance", "full_speed"]
    );
    assert!(!thermal.has_fan_curve);

    // Display capabilities
    let display = profile
        .capabilities
        .display
        .as_ref()
        .expect("Display capability present");
    assert!(display.has_oled);
    assert!(display.supports_flicker_free);
    assert_eq!(display.refresh_rates, vec![60, 120]);

    // Battery capabilities
    let battery = profile
        .capabilities
        .battery
        .as_ref()
        .expect("Battery capability present");
    assert_eq!(battery.driver, "asus_charge_control");

    // DMI Matching verification
    let profiles = vec![profile];
    let matched = DmiMatcher::match_profile(&profiles, "S5506MA", Some("S5506MA"));
    assert!(matched.is_some());
    assert_eq!(matched.unwrap().device.name, "ASUS Vivobook S 15 OLED");
}

#[test]
fn test_zenbook_um5302_profile_and_capabilities() {
    use alatus::hardware::DeviceContext;

    let asset_path = Path::new("assets/devices/zenbook_um5302.toml");
    let content = fs::read_to_string(asset_path)
        .unwrap_or_else(|e| panic!("Failed to read asset {}: {e}", asset_path.display()));

    let profile: DeviceProfile = DeviceProfile::from_toml_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {e}", asset_path.display()));

    assert_eq!(profile.device.name, "ASUS Zenbook S 13 OLED");
    assert!(
        profile.capabilities.rgb.is_none(),
        "Zenbook S 13 must not declare RGB capability"
    );

    // DMI Matcher test
    let profiles = alatus::hardware::context::builtin_profiles();
    let matched = DmiMatcher::match_profile(&profiles, "UM5302TA", Some("UM5302TA"));
    assert!(matched.is_some());
    assert_eq!(matched.unwrap().device.name, "ASUS Zenbook S 13 OLED");

    // Derive DeviceContext: RGB must be strictly Unsupported without runtime panics
    let ctx = DeviceContext::from_profile(profile);
    assert!(
        ctx.capabilities.rgb.is_unsupported(),
        "RGB subsystem must be marked Unsupported for Zenbook S 13 OLED"
    );
    assert!(ctx.rgb.is_none());
}

#[test]
fn test_rog_g14_profile_and_capabilities() {
    use alatus::hardware::DeviceContext;

    let asset_path = Path::new("assets/devices/rog_g14.toml");
    let content = fs::read_to_string(asset_path)
        .unwrap_or_else(|e| panic!("Failed to read asset {}: {e}", asset_path.display()));

    let profile: DeviceProfile = DeviceProfile::from_toml_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {e}", asset_path.display()));

    assert_eq!(profile.device.name, "ROG Zephyrus G14");
    let disp = profile
        .capabilities
        .display
        .as_ref()
        .expect("Display capability present");
    assert!(
        !disp.has_oled,
        "ROG Zephyrus G14 must not be configured as OLED panel"
    );
    assert!(
        !disp.supports_flicker_free,
        "Non-OLED must not have flicker-free dimming enabled"
    );
    assert_eq!(disp.refresh_rates, vec![60, 120, 144]);

    // DMI Matcher test
    let profiles = alatus::hardware::context::builtin_profiles();
    let matched = DmiMatcher::match_profile(&profiles, "ROG Zephyrus G14", Some("GA402"));
    assert!(matched.is_some());
    assert_eq!(matched.unwrap().device.name, "ROG Zephyrus G14");

    // Derive DeviceContext: Display flicker-free must be false
    let ctx = DeviceContext::from_profile(profile);
    if let alatus::hardware::CapabilityState::Supported(ref d) = ctx.capabilities.display {
        assert!(!d.supports_flicker_free_dimming);
        assert_eq!(d.supported_refresh_rates, vec![60, 120, 144]);
    } else {
        panic!("Display capability expected to be supported");
    }
}
