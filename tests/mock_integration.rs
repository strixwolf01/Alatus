// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use std::collections::HashMap;
use std::fs;
use std::sync::atomic::Ordering;
use tempfile::tempdir;

use alatus::services::desktop_session::{MutterModeSpec, SessionState, pick_mutter_mode};
use alatus::services::hardware_resolver::{
    HardwareResolveError, ThermalRegister, match_asus_profile, resolve_register_from_dmi,
    resolve_register_from_dmi_dir,
};
use alatus::services::telemetry::{read_battery_telemetry_from, read_thermal_telemetry_from};

// ============================================================================
// 1. Hardware Resolver / DMI Edge Cases & Platform Rejection
// ============================================================================

#[test]
fn test_dmi_malformed_non_ascii_handling() {
    let tmp = tempdir().expect("creates tempdir");
    let bad_file = tmp.path().join("malformed_product_name");

    // Write invalid UTF-8 byte sequences
    fs::write(&bad_file, [0xFF, 0xFE, 0xFD, 0x80, 0x00]).expect("writes invalid UTF-8");

    let res = resolve_register_from_dmi(bad_file.to_str().unwrap());
    assert!(res.is_err());
    match res.unwrap_err() {
        HardwareResolveError::DmiReadError(msg) => {
            assert!(
                msg.contains("invalid utf-8") || msg.contains("stream did not contain valid UTF-8"),
                "Expected UTF-8 error, got: {msg}"
            );
        }
        other => panic!("Expected DmiReadError, got: {other:?}"),
    }
}

#[test]
fn test_dmi_missing_file_handling() {
    let tmp = tempdir().expect("creates tempdir");
    let missing_file = tmp.path().join("non_existent_node");

    let res = resolve_register_from_dmi(missing_file.to_str().unwrap());
    assert!(res.is_err());
    match res.unwrap_err() {
        HardwareResolveError::DmiReadError(msg) => {
            assert!(msg.contains("No such file") || msg.contains("cannot find"));
        }
        other => panic!("Expected DmiReadError, got: {other:?}"),
    }
}

#[test]
fn test_dmi_rejection_of_non_asus_platforms() {
    // 1. Lenovo ThinkPad
    let lenovo = match_asus_profile(
        "ThinkPad X1 Carbon Gen 10",
        Some("21CBCTO1WW"),
        Some("LENOVO"),
    );
    assert!(matches!(
        lenovo,
        Err(HardwareResolveError::UnsupportedHardware(_))
    ));
    if let Err(HardwareResolveError::UnsupportedHardware(msg)) = lenovo {
        assert!(msg.contains("Non-ASUS vendor 'Some(\"LENOVO\")'"));
    }

    // 2. Dell XPS
    let dell = match_asus_profile("XPS 15 9520", Some("0M1234"), Some("Dell Inc."));
    assert!(matches!(
        dell,
        Err(HardwareResolveError::UnsupportedHardware(_))
    ));
    if let Err(HardwareResolveError::UnsupportedHardware(msg)) = dell {
        assert!(msg.contains("Non-ASUS vendor 'Some(\"Dell Inc.\")'"));
    }

    // 3. HP Spectre
    let hp = match_asus_profile("HP Spectre x360 14", Some("8765"), Some("HP"));
    assert!(matches!(
        hp,
        Err(HardwareResolveError::UnsupportedHardware(_))
    ));
    if let Err(HardwareResolveError::UnsupportedHardware(msg)) = hp {
        assert!(msg.contains("Non-ASUS vendor 'Some(\"HP\")'"));
    }

    // 4. Unknown platform without vendor
    let generic = match_asus_profile("Generic PC", None, None);
    assert!(matches!(
        generic,
        Err(HardwareResolveError::UnsupportedHardware(_))
    ));
}

#[test]
fn test_dmi_directory_mocking_resolution() {
    let tmp = tempdir().expect("creates tempdir");

    // Test ASUS Vivobook mock DMI tree
    fs::write(
        tmp.path().join("product_name"),
        "Vivobook S 15 OLED S5506MA\n",
    )
    .expect("writes product_name");
    fs::write(tmp.path().join("board_name"), "S5506MA\n").expect("writes board_name");
    fs::write(tmp.path().join("sys_vendor"), "ASUSTeK COMPUTER INC.\n").expect("writes sys_vendor");

    let res = resolve_register_from_dmi_dir(tmp.path());
    assert!(res.is_ok(), "Expected resolution success for Vivobook S 15");
    assert_eq!(res.unwrap(), ThermalRegister::Default(0x110019));

    // Test ASUS ROG Zephyrus GA401 mock DMI tree -> V1 (0x5002f)
    fs::write(
        tmp.path().join("product_name"),
        "ROG Zephyrus G14 GA401IV\n",
    )
    .expect("writes product_name");
    fs::write(tmp.path().join("board_name"), "GA401IV\n").expect("writes board_name");
    let res_rog = resolve_register_from_dmi_dir(tmp.path());
    assert!(res_rog.is_ok(), "Expected resolution success for ROG GA401");
    assert_eq!(res_rog.unwrap(), ThermalRegister::V1(0x5002f));

    // Test ASUS TUF Gaming mock DMI tree -> V1 (0x5002f)
    fs::write(
        tmp.path().join("product_name"),
        "ASUS TUF Gaming A15 FA506IV\n",
    )
    .expect("writes product_name");
    fs::write(tmp.path().join("board_name"), "FA506IV\n").expect("writes board_name");
    let res_tuf = resolve_register_from_dmi_dir(tmp.path());
    assert!(res_tuf.is_ok(), "Expected resolution success for TUF FA506");
    assert_eq!(res_tuf.unwrap(), ThermalRegister::V1(0x5002f));

    // Now modify sys_vendor to non-ASUS and verify rejection
    fs::write(tmp.path().join("sys_vendor"), "Lenovo\n").expect("overwrites sys_vendor");
    let rejected = resolve_register_from_dmi_dir(tmp.path());
    assert!(
        matches!(rejected, Err(HardwareResolveError::UnsupportedHardware(_))),
        "Expected rejection when vendor is Lenovo"
    );
}

// ============================================================================
// 2. Power & Telemetry Sysfs Mocking
// ============================================================================

#[test]
fn test_battery_telemetry_fallback_current_now_without_power_now() {
    let tmp = tempdir().expect("creates tempdir");
    let bat_dir = tmp.path().join("BAT0");
    fs::create_dir_all(&bat_dir).expect("creates bat_dir");

    fs::write(bat_dir.join("type"), "Battery\n").expect("writes type");
    fs::write(bat_dir.join("status"), "Discharging\n").expect("writes status");
    fs::write(bat_dir.join("capacity"), "72\n").expect("writes capacity");
    // 11.4V = 11,400,000 uV
    fs::write(bat_dir.join("voltage_now"), "11400000\n").expect("writes voltage_now");
    // 1.5A = 1,500,000 uA (no power_now node provided)
    fs::write(bat_dir.join("current_now"), "1500000\n").expect("writes current_now");

    let bat = read_battery_telemetry_from(tmp.path()).expect("returns battery telemetry");
    assert_eq!(bat.status, "Discharging");
    assert_eq!(bat.capacity, 72);
    assert!((bat.voltage_v - 11.4).abs() < 0.001);
    assert!((bat.current_a - 1.5).abs() < 0.001);
    // Calculated: 11.4 V * 1.5 A = 17.1 W
    assert!((bat.rate_watts - 17.1).abs() < 0.001);
}

#[test]
fn test_battery_telemetry_prefers_power_now() {
    let tmp = tempdir().expect("creates tempdir");
    let bat_dir = tmp.path().join("BAT0");
    fs::create_dir_all(&bat_dir).expect("creates bat_dir");

    fs::write(bat_dir.join("type"), "Battery\n").expect("writes type");
    fs::write(bat_dir.join("status"), "Charging\n").expect("writes status");
    fs::write(bat_dir.join("capacity"), "80\n").expect("writes capacity");
    fs::write(bat_dir.join("voltage_now"), "12000000\n").expect("writes voltage_now");
    fs::write(bat_dir.join("current_now"), "2000000\n").expect("writes current_now");
    // Direct power node: 25.5 W = 25,500,000 uW
    fs::write(bat_dir.join("power_now"), "25500000\n").expect("writes power_now");

    let bat = read_battery_telemetry_from(tmp.path()).expect("returns battery telemetry");
    assert_eq!(bat.status, "Charging");
    assert_eq!(bat.capacity, 80);
    // Directly parsed power_now overrides V * A
    assert!((bat.rate_watts - 25.5).abs() < 0.001);
}

#[test]
fn test_battery_telemetry_missing_or_non_battery_supply() {
    let tmp = tempdir().expect("creates tempdir");

    // 1. Completely empty power_supply directory
    assert!(read_battery_telemetry_from(tmp.path()).is_none());

    // 2. Only AC mains adapter present (no battery node)
    let ac_dir = tmp.path().join("AC0");
    fs::create_dir_all(&ac_dir).expect("creates ac_dir");
    fs::write(ac_dir.join("type"), "Mains\n").expect("writes type");
    fs::write(ac_dir.join("online"), "1\n").expect("writes online");

    assert!(read_battery_telemetry_from(tmp.path()).is_none());
}

#[test]
fn test_thermal_telemetry_missing_sensors_fallback() {
    let tmp = tempdir().expect("creates tempdir");

    // 1. Empty hwmon directory -> returns safe defaults (0 RPM, 0°C)
    let empty_telemetry = read_thermal_telemetry_from(tmp.path());
    assert_eq!(empty_telemetry.fan_rpm, 0);
    assert_eq!(empty_telemetry.fan2_rpm, None);
    assert_eq!(empty_telemetry.temp_c, 0);

    // 2. Hwmon with only single fan and no temperature sensor
    let hwmon0 = tmp.path().join("hwmon0");
    fs::create_dir_all(&hwmon0).expect("creates hwmon0");
    fs::write(hwmon0.join("name"), "asus\n").expect("writes name");
    fs::write(hwmon0.join("fan1_input"), "2800\n").expect("writes fan1_input");

    let fan_only = read_thermal_telemetry_from(tmp.path());
    assert_eq!(fan_only.fan_rpm, 2800);
    assert_eq!(fan_only.fan2_rpm, None);
    assert_eq!(fan_only.temp_c, 0);

    // 3. Dual fans and CPU coretemp
    fs::write(hwmon0.join("fan2_input"), "2950\n").expect("writes fan2_input");
    let hwmon1 = tmp.path().join("hwmon1");
    fs::create_dir_all(&hwmon1).expect("creates hwmon1");
    fs::write(hwmon1.join("name"), "coretemp\n").expect("writes name");
    fs::write(hwmon1.join("temp1_input"), "58000\n").expect("writes temp1_input"); // 58°C

    let dual_fan_telemetry = read_thermal_telemetry_from(tmp.path());
    assert_eq!(dual_fan_telemetry.fan_rpm, 2800);
    assert_eq!(dual_fan_telemetry.fan2_rpm, Some(2950));
    assert_eq!(dual_fan_telemetry.temp_c, 58);
}

// ============================================================================
// 3. Desktop Session Resilience & Edge Cases
// ============================================================================

#[test]
fn test_pick_mutter_mode_unusual_and_fractional_refresh_rates() {
    let dummy_props = HashMap::new();
    let modes: Vec<MutterModeSpec> = vec![
        (
            "mode-59_94".to_string(),
            2880,
            1620,
            59.940,
            1.0,
            vec![],
            dummy_props.clone(),
        ),
        (
            "mode-90".to_string(),
            2880,
            1620,
            90.0,
            1.0,
            vec![],
            dummy_props.clone(),
        ),
        (
            "mode-144".to_string(),
            2880,
            1620,
            144.0,
            1.0,
            vec![],
            dummy_props.clone(),
        ),
        (
            "other-res".to_string(),
            1920,
            1080,
            60.0,
            1.0,
            vec![],
            dummy_props,
        ),
    ];

    // On AC: Highest refresh rate must be selected deterministically (144Hz)
    let ac_mode = pick_mutter_mode(&modes, (2880, 1620), true);
    assert!(ac_mode.is_some());
    assert_eq!(ac_mode.unwrap().0, "mode-144");
    assert_eq!(ac_mode.unwrap().3, 144.0);

    // On Battery: Closest mode to 60.0Hz must be selected (59.940Hz)
    let bat_mode = pick_mutter_mode(&modes, (2880, 1620), false);
    assert!(bat_mode.is_some());
    assert_eq!(bat_mode.unwrap().0, "mode-59_94");
    assert_eq!(bat_mode.unwrap().3, 59.940);
}

#[test]
fn test_pick_mutter_mode_when_all_modes_above_60hz() {
    let dummy_props = HashMap::new();
    // System only supports high refresh rates (e.g. 90Hz, 120Hz, 165Hz)
    let modes: Vec<MutterModeSpec> = vec![
        (
            "mode-90".to_string(),
            2560,
            1440,
            90.0,
            1.0,
            vec![],
            dummy_props.clone(),
        ),
        (
            "mode-120".to_string(),
            2560,
            1440,
            120.0,
            1.0,
            vec![],
            dummy_props.clone(),
        ),
        (
            "mode-165".to_string(),
            2560,
            1440,
            165.0,
            1.0,
            vec![],
            dummy_props,
        ),
    ];

    // On AC: Picks 165Hz
    let ac = pick_mutter_mode(&modes, (2560, 1440), true).unwrap();
    assert_eq!(ac.0, "mode-165");

    // On Battery: Picks 90Hz (closest to 60Hz)
    let bat = pick_mutter_mode(&modes, (2560, 1440), false).unwrap();
    assert_eq!(bat.0, "mode-90");
}

#[test]
fn test_oled_care_double_dimming_cache_protection() {
    let state = SessionState::new(true, true, true);

    // Verify initial state
    assert!(!state.is_dimmed.load(Ordering::SeqCst));
    assert_eq!(state.cached_panel_brightness.load(Ordering::Relaxed), -1);
    assert_eq!(state.cached_rgb_brightness.load(Ordering::Relaxed), 100);

    // Simulate user setting normal pre-dim brightness
    state.cached_panel_brightness.store(85, Ordering::Relaxed);
    state.cached_rgb_brightness.store(75, Ordering::Relaxed);

    // 1. First Idle Event: Transition to dimmed state
    let first_dim = state.try_enter_dimmed();
    assert!(first_dim, "First dimming attempt should succeed");
    assert!(state.is_dimmed.load(Ordering::SeqCst));

    // 2. Second Duplicate/Race Idle Event: Attempt to dim again while already dimmed
    let second_dim = state.try_enter_dimmed();
    assert!(
        !second_dim,
        "Second dimming attempt must be rejected to prevent cache clobbering"
    );

    // Verify cache integrity: cached values remain untouched
    assert_eq!(
        state.cached_panel_brightness.load(Ordering::Relaxed),
        85,
        "Pre-dim panel brightness must not be clobbered"
    );
    assert_eq!(
        state.cached_rgb_brightness.load(Ordering::Relaxed),
        75,
        "Pre-dim RGB brightness must not be clobbered"
    );

    // 3. User Returns / Wake-Up Event: Transition out of dimmed state
    let first_wake = state.try_exit_dimmed();
    assert!(first_wake, "Wake-up transition should succeed");
    assert!(!state.is_dimmed.load(Ordering::SeqCst));

    // 4. Duplicate Wake-Up Event: Attempt to exit when already not dimmed
    let second_wake = state.try_exit_dimmed();
    assert!(
        !second_wake,
        "Redundant wake-up transition should return false"
    );
}
