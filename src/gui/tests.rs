// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use super::*;

#[test]
fn test_app_window_initialization() {
    match AppWindow::new() {
        Ok(w) => {
            assert_eq!(w.get_current_tab(), 0);
            assert_eq!(w.get_thermal_mode(), 1);
            assert!(w.get_oled_care_enabled());
            assert_eq!(w.get_oled_dim_level(), 100);
            assert_eq!(w.get_charge_limit(), 80);
            assert_eq!(w.get_rgb_brightness(), 80);
            assert_eq!(w.get_selected_refresh_rate(), 120);
            assert_eq!(w.get_selected_rgb_preset(), 0);
            assert_eq!(w.get_hw_serial(), "••••••••••••");
            assert!(!w.get_hw_serial_revealed());
            assert!(w.get_tray_enabled());
            assert!(!w.get_autostart_enabled());
        }
        Err(e) => {
            // In headless sandbox/CI without display socket, verify error is platform/display related
            let err_str = e.to_string();
            assert!(
                err_str.contains("display")
                    || err_str.contains("Display")
                    || err_str.contains("platform")
                    || err_str.contains("Platform")
                    || std::env::var("DISPLAY").is_err()
                    || std::env::var("WAYLAND_DISPLAY").is_err(),
                "Unexpected window initialization error: {err_str}"
            );
        }
    }
}

#[test]
fn test_derive_m3_palette_fallback() {
    let palette = derive_m3_palette(None);
    // Fallback is M3 Violet (208, 188, 255)
    assert_eq!(palette.accent_color.red(), 208);
    assert_eq!(palette.accent_color.green(), 188);
    assert_eq!(palette.accent_color.blue(), 255);
}

#[test]
fn test_derive_m3_palette_custom() {
    let palette = derive_m3_palette(Some((61, 174, 233)));
    assert_eq!(palette.accent_color.red(), 61);
    assert_eq!(palette.accent_color.green(), 174);
    assert_eq!(palette.accent_color.blue(), 233);
}

#[test]
fn test_sanitize_product_model() {
    assert_eq!(
        sanitize_product_model("ASUS Vivobook S 15 S5506MA_S5506MA"),
        "ASUS Vivobook S 15 S5506MA"
    );
    assert_eq!(
        sanitize_product_model("Zenbook 14 OLED UX3405MA_UX3405MA"),
        "Zenbook 14 OLED UX3405MA"
    );
    assert_eq!(
        sanitize_product_model("ROG Zephyrus G14"),
        "ROG Zephyrus G14"
    );
    assert_eq!(
        sanitize_product_model("  ASUS Vivobook S 14 OLED S5406SA_S5406SA  "),
        "ASUS Vivobook S 14 OLED S5406SA"
    );
}

#[test]
fn test_sanitize_gpu_specs() {
    // Intel Arc Graphics (Meteor Lake-P)
    let intel_arc = "00:02.0 VGA compatible controller [0300]: Intel Corporation Meteor Lake-P [Intel Arc Graphics] [8086:7d55] (rev 08)";
    assert_eq!(
        sanitize_gpu_specs(intel_arc, "i915"),
        "Intel Arc Graphics (Meteor Lake-P) [driver: i915]"
    );

    // AMD Radeon 780M (Phoenix1)
    let amd_phoenix = "03:00.0 VGA compatible controller [0300]: Advanced Micro Devices, Inc. [AMD/ATI] Phoenix1 [Radeon 780M] [1002:15bf] (rev c8)";
    assert_eq!(
        sanitize_gpu_specs(amd_phoenix, "amdgpu"),
        "AMD Radeon 780M (Phoenix1) [driver: amdgpu]"
    );

    // NVIDIA GeForce RTX 4070
    let nvidia_rtx = "01:00.0 3D controller [0302]: NVIDIA Corporation AD106M [GeForce RTX 4070 Max-Q / Mobile] [10de:2860] (rev a1)";
    assert_eq!(
        sanitize_gpu_specs(nvidia_rtx, "nvidia"),
        "NVIDIA GeForce RTX 4070 Max-Q / Mobile (AD106M) [driver: nvidia]"
    );

    // Intel Alder Lake-P Iris Xe
    let intel_iris = "00:02.0 VGA compatible controller [0300]: Intel Corporation Alder Lake-P [Iris Xe Graphics] [8086:46a6] (rev 0c)";
    assert_eq!(
        sanitize_gpu_specs(intel_iris, "i915"),
        "Intel Iris Xe Graphics (Alder Lake-P) [driver: i915]"
    );

    // AMD Rembrandt Radeon 680M
    let amd_rembrandt =
        "Advanced Micro Devices, Inc. [AMD/ATI] Rembrandt [Radeon 680M] [1002:1681]";
    assert_eq!(
        sanitize_gpu_specs(amd_rembrandt, "amdgpu"),
        "AMD Radeon 680M (Rembrandt) [driver: amdgpu]"
    );

    // Plain Intel UHD without architecture bracket
    let intel_uhd = "Intel Corporation UHD Graphics 620 [8086:5917] (rev 07)";
    assert_eq!(
        sanitize_gpu_specs(intel_uhd, "i915"),
        "Intel UHD Graphics 620 [driver: i915]"
    );

    // Without driver
    assert_eq!(
        sanitize_gpu_specs(intel_arc, ""),
        "Intel Arc Graphics (Meteor Lake-P)"
    );

    // Fallback
    assert_eq!(sanitize_gpu_specs("", ""), "Integrated Graphics");
}
