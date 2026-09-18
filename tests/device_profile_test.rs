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

#[test]
fn test_all_device_profiles_deserialize_validly() {
    use alatus::hardware::{AsusctlProxyDriver, CapabilityState, DeviceContext};

    let dir = Path::new("assets/devices");
    assert!(dir.exists(), "assets/devices directory must exist");

    let entries = fs::read_dir(dir).expect("assets/devices must be readable");
    let mut profile_count = 0;

    for entry in entries {
        let entry = entry.expect("valid directory entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }

        let content = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()));
        let profile = DeviceProfile::from_toml_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {e}", path.display()));

        assert!(
            !profile.device.name.is_empty(),
            "{}: Device name must not be empty",
            path.display()
        );
        assert!(
            !profile.device.vendor.is_empty(),
            "{}: Device vendor must not be empty",
            path.display()
        );
        assert!(
            !profile.device.match_product.is_empty(),
            "{}: match_product must not be empty",
            path.display()
        );

        // Verify fallback logic:
        // When native probing fails (e.g. no physical device on host), injecting AsusctlProxyDriver
        // must satisfy any declared subsystems (RGB, thermal, battery) without panic.
        let mock_proxy = AsusctlProxyDriver::new_mock();
        let ctx = DeviceContext::from_profile_with_fallback(profile.clone(), Some(mock_proxy));

        if profile.capabilities.rgb.is_some() {
            assert!(
                ctx.capabilities.rgb.is_supported(),
                "{}: Expected RGB capability to be supported via native or fallback proxy",
                path.display()
            );
        } else {
            assert!(
                ctx.capabilities.rgb.is_unsupported(),
                "{}: Expected omitted RGB capability to be unsupported",
                path.display()
            );
        }

        if profile.capabilities.thermal.is_some() {
            assert!(
                ctx.capabilities.thermal.is_supported(),
                "{}: Expected thermal capability to be supported via native or fallback proxy",
                path.display()
            );
        }

        if profile.capabilities.battery.is_some() {
            assert!(
                ctx.capabilities.battery.is_supported(),
                "{}: Expected battery capability to be supported via native or fallback proxy",
                path.display()
            );
        }

        // Without proxy, verify graceful tagging (either supported natively or marked unavailable, never panicked)
        let ctx_no_proxy = DeviceContext::from_profile_with_fallback(profile.clone(), None);
        if profile.capabilities.rgb.is_none() {
            assert!(ctx_no_proxy.capabilities.rgb.is_unsupported());
        } else {
            assert!(
                ctx_no_proxy.capabilities.rgb.is_supported()
                    || matches!(
                        ctx_no_proxy.capabilities.rgb,
                        CapabilityState::Unavailable(_)
                    )
            );
        }

        profile_count += 1;
    }

    assert!(
        profile_count >= 7,
        "Expected at least 7 device profiles in assets/devices/, found {profile_count}"
    );
}

#[test]
fn test_rog_zephyrus_g14_declarative_profile() {
    let path = Path::new("assets/devices/rog_zephyrus_g14.toml");
    let content = fs::read_to_string(path).expect("read rog_zephyrus_g14.toml");
    let profile = DeviceProfile::from_toml_str(&content).expect("parse rog_zephyrus_g14.toml");

    assert_eq!(profile.device.name, "ROG Zephyrus G14");
    assert!(
        profile
            .device
            .match_product
            .iter()
            .any(|p| p.contains("GA401"))
    );
    assert!(
        profile
            .device
            .match_product
            .iter()
            .any(|p| p.contains("GA402"))
    );
    assert!(
        profile
            .device
            .match_product
            .iter()
            .any(|p| p.contains("GA403"))
    );

    let rgb = profile.capabilities.rgb.as_ref().expect("rgb config");
    assert_eq!(rgb.driver, "aura_hid");
    assert_eq!(rgb.zones, 4);

    let thermal = profile
        .capabilities
        .thermal
        .as_ref()
        .expect("thermal config");
    assert!(thermal.has_fan_curve);

    let disp = profile
        .capabilities
        .display
        .as_ref()
        .expect("display config");
    assert!(!disp.has_oled);
    assert_eq!(disp.gpu_mux_mode, Some(true));
    assert_eq!(disp.panel_od, Some(true));

    // DMI wildcard matching verification
    let profiles = vec![profile];
    let matched_401 = DmiMatcher::match_profile(&profiles, "GA401IV", Some("GA401IV"));
    assert!(matched_401.is_some());
    let matched_403 = DmiMatcher::match_profile(&profiles, "ROG Zephyrus G14 GA403UI", None);
    assert!(matched_403.is_some());
}

#[test]
fn test_rog_strix_g16_declarative_profile() {
    let path = Path::new("assets/devices/rog_strix_g16.toml");
    let content = fs::read_to_string(path).expect("read rog_strix_g16.toml");
    let profile = DeviceProfile::from_toml_str(&content).expect("parse rog_strix_g16.toml");

    assert_eq!(profile.device.name, "ROG Strix G16 / G18");
    let rgb = profile.capabilities.rgb.as_ref().expect("rgb config");
    assert_eq!(rgb.driver, "aura_hid");
    assert_eq!(rgb.zones, 4);

    let thermal = profile
        .capabilities
        .thermal
        .as_ref()
        .expect("thermal config");
    assert!(thermal.has_fan_curve);

    let disp = profile
        .capabilities
        .display
        .as_ref()
        .expect("display config");
    assert_eq!(disp.gpu_mux_mode, Some(true));
    assert_eq!(disp.panel_od, Some(true));
    assert_eq!(disp.refresh_rates, vec![60, 165, 240]);

    // DMI matching
    let profiles = vec![profile];
    let matched = DmiMatcher::match_profile(&profiles, "G614JZ", Some("G614JZ"));
    assert!(matched.is_some());
    let matched_strix = DmiMatcher::match_profile(&profiles, "ROG Strix G18 G814JIR", None);
    assert!(matched_strix.is_some());
}

#[test]
fn test_tuf_gaming_a15_declarative_profile() {
    let path = Path::new("assets/devices/tuf_gaming_a15.toml");
    let content = fs::read_to_string(path).expect("read tuf_gaming_a15.toml");
    let profile = DeviceProfile::from_toml_str(&content).expect("parse tuf_gaming_a15.toml");

    assert_eq!(profile.device.name, "ASUS TUF Gaming A15 / F15");
    let rgb = profile.capabilities.rgb.as_ref().expect("rgb config");
    assert_eq!(rgb.driver, "tuf_sysfs");
    assert_eq!(rgb.zones, 1);

    let thermal = profile
        .capabilities
        .thermal
        .as_ref()
        .expect("thermal config");
    assert!(!thermal.has_fan_curve);

    let disp = profile
        .capabilities
        .display
        .as_ref()
        .expect("display config");
    assert!(!disp.has_oled);
    assert_eq!(disp.refresh_rates, vec![60, 144]);

    // DMI matching
    let profiles = vec![profile];
    let matched = DmiMatcher::match_profile(&profiles, "FA507NV", Some("FA507NV"));
    assert!(matched.is_some());
    let matched_tuf = DmiMatcher::match_profile(&profiles, "TUF Gaming A15 FA506QM", None);
    assert!(matched_tuf.is_some());
}

#[test]
fn test_zenbook_oled_declarative_profile() {
    let path = Path::new("assets/devices/zenbook_oled.toml");
    let content = fs::read_to_string(path).expect("read zenbook_oled.toml");
    let profile = DeviceProfile::from_toml_str(&content).expect("parse zenbook_oled.toml");

    assert_eq!(profile.device.name, "ASUS Zenbook OLED Series");
    let rgb = profile.capabilities.rgb.as_ref().expect("rgb config");
    assert_eq!(rgb.driver, "ite5570");
    assert_eq!(rgb.zones, 1);

    let thermal = profile
        .capabilities
        .thermal
        .as_ref()
        .expect("thermal config");
    assert!(!thermal.has_fan_curve);

    let disp = profile
        .capabilities
        .display
        .as_ref()
        .expect("display config");
    assert!(disp.has_oled);
    assert!(disp.supports_flicker_free);
    assert_eq!(disp.refresh_rates, vec![60, 90, 120]);

    // DMI matching
    let profiles = vec![profile];
    let matched = DmiMatcher::match_profile(&profiles, "UX3402VA", Some("UX3402VA"));
    assert!(matched.is_some());
    let matched_zen = DmiMatcher::match_profile(&profiles, "Zenbook OLED UX5401ZA", None);
    assert!(matched_zen.is_some());
}
