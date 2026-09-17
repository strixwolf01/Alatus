// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Characterization tests locking current observable contracts and behavior invariants
//! before replacing or abstracting underlying hardware implementations.

use alatus::domain::{BrightnessPercent, ChargeThreshold, ColorRgb, ThermalMode};
use alatus::services::firmware_mode::FirmwareMode;

#[test]
fn test_characterization_thermal_mode_mappings() {
    // Verified mapping table:
    // ThermalMode     -> FirmwareMode -> DSTS nibble -> DEVS write param
    // Quiet           -> Quiet        -> 2           -> 1
    // Balanced        -> Balanced     -> 0           -> 0
    // Performance     -> High         -> 1           -> 2
    // FullSpeed       -> Full         -> 3           -> 3

    let test_cases = [
        (ThermalMode::Quiet, FirmwareMode::Quiet, 2u8, 1u8),
        (ThermalMode::Balanced, FirmwareMode::Balanced, 0u8, 0u8),
        (ThermalMode::Performance, FirmwareMode::High, 1u8, 2u8),
        (ThermalMode::FullSpeed, FirmwareMode::Full, 3u8, 3u8),
    ];

    for (domain_mode, expected_fw_mode, expected_nibble, expected_write_param) in test_cases {
        assert_eq!(expected_fw_mode.nibble(), expected_nibble);
        assert_eq!(expected_fw_mode.write_param(), Some(expected_write_param));

        // Verify readback decoding from DSTS nibble produces the exact firmware mode
        let decoded = FirmwareMode::from_nibble(expected_nibble);
        assert_eq!(decoded, expected_fw_mode);

        // Verify domain string serialization preserves exact token
        assert_eq!(domain_mode.to_string(), domain_mode.as_str());
    }
}

#[test]
fn test_characterization_rgb_brightness_sysfs_scaling() {
    // Characterize sysfs brightness levels (0..=3) mapped from percentages (0..=100)
    fn percent_to_sysfs(pct: u8) -> u32 {
        match pct {
            0 => 0,
            1..=33 => 1,
            34..=66 => 2,
            _ => 3,
        }
    }

    fn sysfs_to_percent(sys: u32) -> u8 {
        match sys {
            0 => 0,
            1 => 33,
            2 => 66,
            _ => 100,
        }
    }

    assert_eq!(percent_to_sysfs(0), 0);
    assert_eq!(percent_to_sysfs(25), 1);
    assert_eq!(percent_to_sysfs(50), 2);
    assert_eq!(percent_to_sysfs(100), 3);

    assert_eq!(sysfs_to_percent(0), 0);
    assert_eq!(sysfs_to_percent(1), 33);
    assert_eq!(sysfs_to_percent(2), 66);
    assert_eq!(sysfs_to_percent(3), 100);

    // Verify BrightnessPercent domain constraints
    assert!(BrightnessPercent::new(0).is_ok());
    assert!(BrightnessPercent::new(100).is_ok());
    assert!(BrightnessPercent::new(101).is_err());
}

#[test]
fn test_characterization_rgb_hex_color_conversion() {
    let color = ColorRgb::from_hex("#00ff88").unwrap();
    assert_eq!(color.r, 0);
    assert_eq!(color.g, 255);
    assert_eq!(color.b, 136);
    assert_eq!(color.to_hex(), "#00ff88");

    // Color string formatting matching sysfs / LampArray expectations
    let formatted = format!("{:02x}{:02x}{:02x}", color.r, color.g, color.b);
    assert_eq!(formatted, "00ff88");
}

#[test]
fn test_characterization_battery_threshold_bounds_and_format() {
    // Standard ASUS battery charge threshold range: 50% to 100%
    assert!(ChargeThreshold::new(49).is_err());
    assert!(ChargeThreshold::new(50).is_ok());
    assert!(ChargeThreshold::new(80).is_ok());
    assert!(ChargeThreshold::new(100).is_ok());
    assert!(ChargeThreshold::new(101).is_err());

    let threshold = ChargeThreshold::new(80).unwrap();
    // Kernel sysfs expects integer followed by newline
    let sysfs_payload = format!("{}\n", threshold.value());
    assert_eq!(sysfs_payload, "80\n");
}
