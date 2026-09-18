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

#[test]
fn test_rog_zephyrus_with_aura_hid_and_fallback() {
    use alatus::hardware::drivers::AuraHidDriver;
    use alatus::hardware::{
        AsusctlProxyDriver, CapabilityState, DeviceContext, DeviceProfile, RgbDriver,
    };
    use std::fs;
    use std::path::Path;

    let path = Path::new("assets/devices/rog_zephyrus_g14.toml");
    let content = fs::read_to_string(path).expect("read rog_zephyrus_g14.toml");
    let profile = DeviceProfile::from_toml_str(&content).expect("parse rog_zephyrus_g14.toml");

    // Headless / non-ROG environment: native Aura HID probe returns None, so proxy takes over
    let mock_proxy = AsusctlProxyDriver::new_mock();
    let ctx = DeviceContext::from_profile_with_fallback(profile.clone(), Some(mock_proxy));

    match &ctx.capabilities.rgb {
        CapabilityState::Supported(details) => {
            assert_eq!(details.max_brightness, 100);
            assert!(details.supports_custom_color);
            assert!(details.supports_inactivity_timeout);
            assert_eq!(
                details.supported_zones,
                vec!["zone1", "zone2", "zone3", "zone4"]
            );
        }
        other => panic!("Expected Supported RGB capability via fallback, got {other:?}"),
    }
    assert!(ctx.rgb.is_some());

    // Without proxy and in headless environment, RGB becomes Unavailable(KernelInterfaceMissing)
    let ctx_no_proxy = DeviceContext::from_profile_with_fallback(profile, None);
    // If running on actual ROG hardware it could be Supported; if not, it must be gracefully Unavailable without panicking
    assert!(
        ctx_no_proxy.capabilities.rgb.is_supported()
            || ctx_no_proxy.capabilities.rgb.is_unavailable()
    );

    // Standalone mock driver verification
    let (mut driver, packets) = AuraHidDriver::new_mock();
    assert_eq!(driver.zones(), 4);
    driver
        .set_color(alatus::domain::ColorRgb::new(255, 0, 0))
        .unwrap();
    let logged = packets.lock().unwrap().clone();
    assert_eq!(logged.len(), 3);
    assert_eq!(logged[0][0], 0x5d); // Report ID
    assert_eq!(logged[0][1], 0xb3); // CMD_BUILTIN_MODE
    assert_eq!(logged[1][1], 0xb5); // CMD_SET
    assert_eq!(logged[2][1], 0xb4); // CMD_APPLY
}

#[test]
fn test_tuf_gaming_with_sysfs_and_fallback() {
    use alatus::hardware::drivers::TufSysfsRgbDriver;
    use alatus::hardware::{
        AsusctlProxyDriver, CapabilityState, DeviceContext, DeviceProfile, RgbDriver,
    };
    use std::fs;
    use std::path::Path;

    let path = Path::new("assets/devices/tuf_gaming_a15.toml");
    let content = fs::read_to_string(path).expect("read tuf_gaming_a15.toml");
    let profile = DeviceProfile::from_toml_str(&content).expect("parse tuf_gaming_a15.toml");

    // Headless / non-TUF environment: native TUF sysfs probe returns None, so proxy takes over
    let mock_proxy = AsusctlProxyDriver::new_mock();
    let ctx = DeviceContext::from_profile_with_fallback(profile.clone(), Some(mock_proxy));

    match &ctx.capabilities.rgb {
        CapabilityState::Supported(details) => {
            assert_eq!(details.max_brightness, 100);
            assert!(details.supports_custom_color);
            assert!(details.supports_inactivity_timeout);
            assert_eq!(details.supported_zones, vec!["keyboard"]);
        }
        other => panic!("Expected Supported RGB capability via fallback, got {other:?}"),
    }
    assert!(ctx.rgb.is_some());

    // Without proxy and in headless environment, RGB becomes Unavailable(KernelInterfaceMissing)
    let ctx_no_proxy = DeviceContext::from_profile_with_fallback(profile, None);
    assert!(
        ctx_no_proxy.capabilities.rgb.is_supported()
            || ctx_no_proxy.capabilities.rgb.is_unavailable()
    );

    // Standalone mock driver verification
    let (mut driver, packets, brightness) = TufSysfsRgbDriver::new_mock();
    driver
        .set_color(alatus::domain::ColorRgb::new(0, 255, 128))
        .unwrap();
    let logged = packets.lock().unwrap().clone();
    assert_eq!(logged.len(), 1);
    assert_eq!(logged[0], [1, 0, 0, 255, 128, 1]);

    driver
        .set_brightness(alatus::domain::BrightnessPercent::new(60).unwrap())
        .unwrap();
    assert_eq!(*brightness.lock().unwrap(), 2);
}

#[test]
fn test_rog_wmi_fan_curve_and_fallback() {
    use alatus::hardware::drivers::{FanCurve, FanCurvePoint, RogWmiThermalDriver};
    use alatus::hardware::{AsusctlProxyDriver, CapabilityState, DeviceContext, DeviceProfile};
    use std::fs;
    use std::path::Path;

    let path = Path::new("assets/devices/rog_zephyrus_g14.toml");
    let content = fs::read_to_string(path).expect("read rog_zephyrus_g14.toml");
    let profile = DeviceProfile::from_toml_str(&content).expect("parse rog_zephyrus_g14.toml");

    // Headless / non-ROG environment: native thermal probe falls back to proxy or standard driver
    let mock_proxy = AsusctlProxyDriver::new_mock();
    let ctx = DeviceContext::from_profile_with_fallback(profile.clone(), Some(mock_proxy));

    match &ctx.capabilities.thermal {
        CapabilityState::Supported(details) => {
            assert!(details.supported_modes.len() >= 3);
            assert!(details.fan_count >= 2);
        }
        other => panic!("Expected Supported Thermal capability via fallback, got {other:?}"),
    }
    assert!(ctx.thermal.is_some());

    // Standalone mock driver verification
    let (mut driver, writes) = RogWmiThermalDriver::new_mock();
    assert_eq!(driver.fan_count(), 2);

    let curve = FanCurve([
        FanCurvePoint::new(35, 30),
        FanCurvePoint::new(45, 60),
        FanCurvePoint::new(55, 90),
        FanCurvePoint::new(65, 120),
        FanCurvePoint::new(75, 150),
        FanCurvePoint::new(85, 190),
        FanCurvePoint::new(95, 225),
        FanCurvePoint::new(105, 255),
    ]);
    assert!(curve.validate().is_ok());

    // Apply curve to fan 1
    driver.apply_custom_curve(1, &curve).unwrap();

    let logged = writes.lock().unwrap().clone();
    assert_eq!(logged.len(), 17);
    // 8 PWM points
    assert_eq!(logged[0].0, "pwm1_auto_point1_pwm");
    assert_eq!(logged[7].0, "pwm1_auto_point8_pwm");
    // 8 Temp points
    assert_eq!(logged[8].0, "pwm1_auto_point1_temp");
    assert_eq!(logged[15].0, "pwm1_auto_point8_temp");
    // Enable point must be last
    assert_eq!(logged[16].0, "pwm1_enable");
    assert_eq!(logged[16].1, "1");

    // Reset to auto
    writes.lock().unwrap().clear();
    driver.reset_curves_to_auto().unwrap();
    let logged_reset = writes.lock().unwrap().clone();
    assert_eq!(logged_reset.len(), 2);
    assert_eq!(
        logged_reset[0],
        ("pwm1_enable".to_string(), "2".to_string())
    );
    assert_eq!(
        logged_reset[1],
        ("pwm2_enable".to_string(), "2".to_string())
    );
}

#[test]
fn test_armoury_platform_driver_device_context() {
    use alatus::hardware::capabilities::{CapabilityState, PowerLimitCapabilities};
    use alatus::hardware::context::DeviceContext;
    use alatus::hardware::drivers::{ArmouryPlatformDriver, AsusctlProxyDriver};
    use alatus::hardware::DriverError;

    // 1. Standalone mock Armoury driver validation
    let (mut driver, writes) = ArmouryPlatformDriver::new_mock();
    let attrs = driver.list_attributes();
    assert!(attrs.contains(&"ppt_pl1_spl".to_string()));
    assert!(attrs.contains(&"ppt_pl2_sppt".to_string()));
    assert!(attrs.contains(&"ppt_fppt".to_string()));
    assert!(attrs.contains(&"nv_dynamic_boost".to_string()));
    assert!(attrs.contains(&"gpu_mux_mode".to_string()));
    assert!(attrs.contains(&"panel_od".to_string()));

    // Safe clamping checks on CPU SPL
    driver.set_cpu_spl(250).expect("write clamped SPL");
    assert_eq!(driver.get_cpu_spl().unwrap(), 125); // Clamped to max

    driver.set_cpu_spl(10).expect("write clamped SPL");
    assert_eq!(driver.get_cpu_spl().unwrap(), 35); // Clamped to min

    // SPPT & FPPT tuning
    driver.set_cpu_sppt(100).expect("write SPPT");
    assert_eq!(driver.get_cpu_sppt().unwrap(), 100);

    driver.set_cpu_fppt(115).expect("write FPPT");
    assert_eq!(driver.get_cpu_fppt().unwrap(), 115);

    // GPU Dynamic Boost tuning
    driver
        .set_gpu_dynamic_boost(25)
        .expect("write dynamic boost");
    assert_eq!(driver.get_gpu_dynamic_boost().unwrap(), 25);

    // GPU MUX and Panel Overdrive switches
    driver.set_gpu_mux_mode(0).expect("set discrete MUX");
    assert_eq!(driver.get_gpu_mux_mode().unwrap(), 0);

    driver.set_panel_od(true).expect("enable panel OD");
    assert!(driver.get_panel_od().unwrap());

    let logged = writes.lock().unwrap().clone();
    assert_eq!(logged.len(), 7);

    // 2. Integration with DeviceContext when platform is unsupported
    let path = Path::new("assets/devices/rog_strix_g16.toml");
    let content = fs::read_to_string(path).expect("read rog_strix_g16.toml");
    let profile = DeviceProfile::from_toml_str(&content).expect("parse rog_strix_g16.toml");

    let mock_proxy = AsusctlProxyDriver::new_mock();
    let mut ctx = DeviceContext::from_profile_with_fallback(profile.clone(), Some(mock_proxy));

    // In a test environment without physical /sys/class/firmware-attributes/asus-armoury,
    // platform capability defaults cleanly to Unsupported without panicking.
    if !ctx.capabilities.platform.is_supported() {
        assert!(matches!(
            ctx.platform_mut(),
            Err(DriverError::Unsupported(_))
        ));
        assert!(ctx.platform().is_none());
    }

    // 3. Simulated DeviceContext with active ArmouryPlatformDriver
    ctx.platform = Some(Box::new(driver));
    ctx.capabilities.platform = CapabilityState::Supported(PowerLimitCapabilities {
        supported_attributes: attrs,
        has_cpu_ppt: true,
        has_gpu_boost: true,
        has_gpu_mux: true,
        has_panel_od: true,
    });

    assert!(ctx.capabilities.platform.is_supported());
    assert!(ctx.platform().is_some());

    let plat = ctx.platform_mut().expect("platform driver access");
    assert_eq!(plat.get_gpu_mux_mode().unwrap(), 0);
    assert!(plat.get_panel_od().unwrap());
    plat.set_panel_od(false).unwrap();
    assert!(!plat.get_panel_od().unwrap());
}
