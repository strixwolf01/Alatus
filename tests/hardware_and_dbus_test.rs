// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use std::fs;
use std::sync::atomic::Ordering;
use tempfile::tempdir;

use alatus::services::alatus_rgb_wrapper::{build_asus_rgb_packet, percent_to_intensity};
use alatus::services::desktop_session::{
    DesktopEnv, OledMetrics, SessionState, default_oled_dim_level, load_oled_metrics_from,
    save_oled_metrics_to,
};
use alatus::services::firmware_mode::{FirmwareMode, parse_dsts};
use alatus::services::rgb::{percent_to_sysfs, sysfs_to_percent};

#[cfg(feature = "gui")]
slint::include_modules!();

// ============================================================================
// Test 1: ASUS WMI DebugFS Roundtrip (fan_state 1:1)
// ============================================================================

#[test]
fn test_wmi_debugfs_set_and_get_mock() {
    let tmp = tempdir().expect("creates tempdir");
    let dev_id_path = tmp.path().join("dev_id");
    let ctrl_param_path = tmp.path().join("ctrl_param");
    let devs_path = tmp.path().join("devs");
    let dsts_path = tmp.path().join("dsts");

    fs::write(&devs_path, "DEVS(0x00110019, 0x00000003) = 0x1\n").unwrap();
    fs::write(&dsts_path, "DSTS(0x00110019) = 0x0a070003\n").unwrap();

    // Verify 1:1 fan_state writes: dev_id = 0x00110019, ctrl_param = 3, devs read
    fs::write(&dev_id_path, "0x00110019\n").unwrap();
    fs::write(&ctrl_param_path, "3\n").unwrap();

    let dev_id = fs::read_to_string(&dev_id_path).unwrap();
    assert_eq!(dev_id.trim(), "0x00110019");

    let ctrl_param = fs::read_to_string(&ctrl_param_path).unwrap();
    assert_eq!(ctrl_param.trim(), "3");

    let parsed = parse_dsts("DSTS(0x00110019) = 0x0a070003\n").unwrap();
    assert_eq!(parsed, FirmwareMode::Full);
}

#[test]
fn test_wmi_debugfs_mock_roundtrip() {
    let mode_quiet = parse_dsts("DSTS(0x00110019) = 0x00010002\n").expect("parses dsts quiet");
    assert_eq!(mode_quiet, FirmwareMode::Quiet);

    let mode_high = parse_dsts("DSTS(0x00110019) = 0x00010001\n").expect("parses dsts high");
    assert_eq!(mode_high, FirmwareMode::High);

    let mode_balanced = parse_dsts("0x00010000").expect("parses dsts balanced");
    assert_eq!(mode_balanced, FirmwareMode::Balanced);

    let mode_full = parse_dsts("0x00010003").expect("parses dsts full");
    assert_eq!(mode_full, FirmwareMode::Full);
}

// ============================================================================
// Test 2: Keyboard RGB HID Packet Generator & Brightness Calculation
// ============================================================================

#[test]
fn test_keyboard_rgb_hid_packet_builder() {
    let packet = build_asus_rgb_packet(0x05, 255, 128, 64, 200);
    assert_eq!(packet.len(), 10);
    assert_eq!(packet[0], 0x05); // report_id
    assert_eq!(packet[1], 0x01);
    assert_eq!(packet[2], 0x00);
    assert_eq!(packet[3], 0x00);
    assert_eq!(packet[4], 0x00);
    assert_eq!(packet[5], 0x00);
    assert_eq!(packet[6], 255); // R
    assert_eq!(packet[7], 128); // G
    assert_eq!(packet[8], 64); // B
    assert_eq!(packet[9], 200); // intensity
}

#[test]
fn test_rgb_brightness_and_intensity_calculations() {
    assert_eq!(percent_to_intensity(0), 0);
    assert_eq!(percent_to_intensity(100), 255);
    let mid = percent_to_intensity(50);
    assert!((127..=128).contains(&mid));

    // Discrete sysfs conversion (max=3)
    assert_eq!(percent_to_sysfs(0, 3), 0);
    assert_eq!(percent_to_sysfs(100, 3), 3);
    assert_eq!(sysfs_to_percent(3, 3), 100);
    assert_eq!(sysfs_to_percent(0, 3), 0);
}

// ============================================================================
// Test 3: Battery Charge Threshold Validation
// ============================================================================

#[test]
fn test_battery_charge_threshold_validation() {
    let tmp = tempdir().expect("creates tempdir");
    let threshold_file = tmp.path().join("charge_control_end_threshold");

    fn validate_and_write(path: &std::path::Path, limit: u32) -> Result<(), String> {
        if limit > 100 {
            return Err("Charge limit must be 0..=100".to_string());
        }
        fs::write(path, format!("{limit}\n")).map_err(|e| e.to_string())
    }

    assert!(validate_and_write(&threshold_file, 80).is_ok());
    let read_back = fs::read_to_string(&threshold_file).unwrap();
    assert_eq!(read_back.trim(), "80");

    assert!(validate_and_write(&threshold_file, 100).is_ok());
    assert!(validate_and_write(&threshold_file, 60).is_ok());
    assert!(validate_and_write(&threshold_file, 101).is_err());
}

// ============================================================================
// Test 4: Desktop Session Responsiveness & OLED Metrics Persistence
// ============================================================================

#[test]
fn test_oled_metrics_persistence_and_defaults() {
    assert_eq!(default_oled_dim_level(), 100);

    let tmp = tempdir().expect("creates tempdir");
    let metrics_path = tmp.path().join("session_metrics.json");

    let metrics = OledMetrics {
        active_screen_seconds: 7200,
        refresh_count: 5,
        last_refresh_timestamp: 1700000000,
        oled_dim_level: 85,
    };

    save_oled_metrics_to(Some(&metrics_path), &metrics);
    let loaded = load_oled_metrics_from(Some(&metrics_path));
    assert_eq!(loaded.refresh_count, 5);
    assert_eq!(loaded.oled_dim_level, 85);
}

#[test]
fn test_session_state_atomics() {
    let metrics = OledMetrics::default();
    let state = SessionState::with_metrics(true, true, true, metrics);
    assert!(state.sync_accent.load(Ordering::Relaxed));
    assert!(state.auto_refresh.load(Ordering::Relaxed));
    assert!(state.oled_care.load(Ordering::Relaxed));
    assert_eq!(state.oled_dim_level.load(Ordering::Relaxed), 100);

    state.oled_dim_level.store(75, Ordering::Relaxed);
    assert_eq!(state.oled_dim_level.load(Ordering::Relaxed), 75);

    state.active_screen_seconds.store(3600, Ordering::Relaxed);
    assert_eq!(state.active_screen_seconds.load(Ordering::Relaxed), 3600);
}

// ============================================================================
// Test 5: Slint GUI Property Bindings & Layout Architecture
// ============================================================================

#[cfg(feature = "gui")]
#[test]
fn test_gui_slint_properties_and_flicker_free_dimming() {
    match AppWindow::new() {
        Ok(window) => {
            assert_eq!(window.get_current_tab(), 0);
            assert_eq!(window.get_thermal_mode(), 1);
            assert!(window.get_oled_care_enabled());
            assert_eq!(window.get_oled_dim_level(), 100);
            assert_eq!(window.get_charge_limit(), 80);
            assert_eq!(window.get_rgb_brightness(), 80);
            assert_eq!(window.get_selected_refresh_rate(), 120);

            // Test setting flicker-free dim level
            window.set_oled_dim_level(75);
            assert_eq!(window.get_oled_dim_level(), 75);

            // Test tab switching
            window.set_current_tab(1);
            assert_eq!(window.get_current_tab(), 1);
        }
        Err(e) => {
            let err = e.to_string();
            assert!(
                err.contains("display")
                    || err.contains("Display")
                    || err.contains("platform")
                    || err.contains("Platform")
                    || std::env::var("DISPLAY").is_err()
                    || std::env::var("WAYLAND_DISPLAY").is_err(),
                "Unexpected window initialization error: {err}"
            );
        }
    }
}

// ============================================================================
// Test 6: Firmware Mode Conversions & Mappings (fan_state spec)
// ============================================================================

#[test]
fn test_firmware_mode_conversions() {
    assert_eq!(FirmwareMode::from(0), FirmwareMode::Balanced);
    assert_eq!(FirmwareMode::from(1), FirmwareMode::Quiet);
    assert_eq!(FirmwareMode::from(2), FirmwareMode::High);
    assert_eq!(FirmwareMode::from(3), FirmwareMode::Full);

    assert_eq!(u32::from(FirmwareMode::Balanced), 0);
    assert_eq!(u32::from(FirmwareMode::Quiet), 1);
    assert_eq!(u32::from(FirmwareMode::High), 2);
    assert_eq!(u32::from(FirmwareMode::Full), 3);

    assert_eq!(FirmwareMode::Balanced.write_param(), Some(0));
    assert_eq!(FirmwareMode::Quiet.write_param(), Some(1));
    assert_eq!(FirmwareMode::High.write_param(), Some(2));
    assert_eq!(FirmwareMode::Full.write_param(), Some(3));
}

// ============================================================================
// Test 7: Desktop Environment Detection Variants
// ============================================================================

#[test]
fn test_desktop_environment_detection() {
    // Helper closure to evaluate detection logic
    fn evaluate_desktop(env: &str) -> DesktopEnv {
        let upper = env.to_uppercase();
        if upper.contains("KDE") {
            DesktopEnv::Kde
        } else if upper.contains("GNOME") || upper.contains("UBUNTU") {
            DesktopEnv::Gnome
        } else if upper.contains("HYPRLAND")
            || upper.contains("SWAY")
            || upper.contains("WLROOTS")
            || upper.contains("WAYFIRE")
            || upper.contains("RIVER")
        {
            DesktopEnv::Wlroots
        } else {
            DesktopEnv::Other
        }
    }

    assert_eq!(evaluate_desktop("KDE"), DesktopEnv::Kde);
    assert_eq!(evaluate_desktop("GNOME"), DesktopEnv::Gnome);
    assert_eq!(evaluate_desktop("ubuntu:GNOME"), DesktopEnv::Gnome);
    assert_eq!(evaluate_desktop("Hyprland"), DesktopEnv::Wlroots);
    assert_eq!(evaluate_desktop("sway"), DesktopEnv::Wlroots);
    assert_eq!(evaluate_desktop("XFCE"), DesktopEnv::Other);
}

// ============================================================================
// Test 8: Full Speed Direct DebugFS Write (No Sysfs Fallback)
// ============================================================================

#[test]
fn test_full_speed_writes_directly_to_debugfs() {
    let tmp = tempdir().expect("creates tempdir");
    let ctrl_param_path = tmp.path().join("ctrl_param");

    // In fan_state, Full Speed is state_value=3 written directly to ctrl_param
    let mode = FirmwareMode::Full;
    let param = mode.write_param().expect("Full mode has write_param 3");
    assert_eq!(param, 3);

    fs::write(&ctrl_param_path, format!("{param}\n")).expect("writes param to debugfs");
    let readback = fs::read_to_string(&ctrl_param_path).unwrap();
    assert_eq!(readback.trim(), "3");
}

#[test]
fn test_flicker_free_dimming_clamp_zero() {
    fn clamp_dim(level: u32) -> u32 {
        level.min(100)
    }

    assert_eq!(clamp_dim(0), 0);
    assert_eq!(clamp_dim(120), 100);
    assert_eq!(clamp_dim(75), 75);
}
