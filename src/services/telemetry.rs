// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Hardware sensor telemetry and presentation payload formatting.
//!
//! Provides read-only sysfs queries for CPU temperature, fan RPMs, battery charge rates,
//! and structured JSON / Waybar formatting.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThermalTelemetry {
    pub fan_rpm: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fan2_rpm: Option<u32>,
    pub temp_c: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatteryTelemetry {
    pub status: String,
    pub capacity: u32,
    pub voltage_v: f64,
    pub current_a: f64,
    pub rate_watts: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RgbPayload {
    pub hex: String,
    pub brightness: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaybarPayload {
    pub text: String,
    pub alt: String,
    pub tooltip: String,
    pub class: String,
    pub percentage: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatusPayload {
    pub text: String,
    pub alt: String,
    pub tooltip: String,
    pub class: String,
    pub percentage: u32,
    pub firmware_mode: String,
    pub charge_limit: u32,
    pub power_source: String,
    pub on_ac: bool,
    pub rgb: RgbPayload,
    pub thermal: ThermalTelemetry,
    pub waybar: WaybarPayload,
}

/// Safely inspects `/sys/class/hwmon` for CPU temperature and fan RPMs.
pub fn read_thermal_telemetry() -> ThermalTelemetry {
    read_thermal_telemetry_from(Path::new("/sys/class/hwmon"))
}

/// Safely inspects the given hwmon directory for CPU temperature and fan RPMs.
pub fn read_thermal_telemetry_from(hwmon_dir: &Path) -> ThermalTelemetry {
    let mut fan1: Option<u32> = None;
    let mut fan2: Option<u32> = None;
    let mut temp: Option<i32> = None;
    let mut preferred_temp: Option<i32> = None;

    if let Ok(entries) = fs::read_dir(hwmon_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = fs::read_to_string(path.join("name"))
                .unwrap_or_default()
                .trim()
                .to_string();

            // Check for fan inputs
            if fan1.is_none() || name == "asus" {
                if let Some(rpm) = fs::read_to_string(path.join("fan1_input"))
                    .ok()
                    .and_then(|s| s.trim().parse::<u32>().ok())
                {
                    fan1 = Some(rpm);
                }
                if let Some(rpm) = fs::read_to_string(path.join("fan2_input"))
                    .ok()
                    .and_then(|s| s.trim().parse::<u32>().ok())
                {
                    fan2 = Some(rpm);
                }
            }

            // Check for temperature inputs
            if let Some(millicelsius) = fs::read_to_string(path.join("temp1_input"))
                .ok()
                .and_then(|s| s.trim().parse::<i32>().ok())
            {
                let deg = millicelsius / 1000;
                if ["coretemp", "k10temp", "zenpower", "cpu_thermal", "asus"]
                    .contains(&name.as_str())
                {
                    preferred_temp = Some(deg);
                } else if temp.is_none() || name == "acpitz" {
                    temp = Some(deg);
                }
            }
        }
    }

    ThermalTelemetry {
        fan_rpm: fan1.unwrap_or(0),
        fan2_rpm: fan2,
        temp_c: preferred_temp.or(temp).unwrap_or(0),
    }
}

/// Safely inspects `/sys/class/power_supply` for battery discharge/charge rate.
pub fn read_battery_telemetry() -> Option<BatteryTelemetry> {
    read_battery_telemetry_from(Path::new("/sys/class/power_supply"))
}

/// Safely inspects the given directory for battery discharge/charge rate.
pub fn read_battery_telemetry_from(base: &Path) -> Option<BatteryTelemetry> {
    let entries = fs::read_dir(base).ok()?;

    for entry in entries.flatten() {
        let path = entry.path();
        let supply_type = fs::read_to_string(path.join("type")).unwrap_or_default();
        if !supply_type.trim().eq_ignore_ascii_case("Battery") {
            continue;
        }

        let status = fs::read_to_string(path.join("status"))
            .unwrap_or_else(|_| "Unknown".to_string())
            .trim()
            .to_string();

        let capacity = fs::read_to_string(path.join("capacity"))
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(0);

        let voltage_uv = fs::read_to_string(path.join("voltage_now"))
            .ok()
            .and_then(|s| s.trim().parse::<f64>().ok())
            .unwrap_or(0.0);

        let current_ua = fs::read_to_string(path.join("current_now"))
            .ok()
            .and_then(|s| s.trim().parse::<f64>().ok())
            .map(|c| c.abs())
            .unwrap_or(0.0);

        let voltage_v = voltage_uv / 1_000_000.0;
        let current_a = current_ua / 1_000_000.0;

        let rate_watts = if let Ok(s) = fs::read_to_string(path.join("power_now")) {
            s.trim()
                .parse::<f64>()
                .map(|p| p / 1_000_000.0)
                .unwrap_or(voltage_v * current_a)
        } else {
            voltage_v * current_a
        };

        return Some(BatteryTelemetry {
            status,
            capacity,
            voltage_v,
            current_a,
            rate_watts,
        });
    }

    None
}

/// Formats a Waybar module payload adhering to the Waybar custom JSON specification.
pub fn format_waybar_payload(
    mode: &str,
    on_ac: bool,
    charge_limit: u32,
    temp_c: i32,
    fan_rpm: u32,
) -> WaybarPayload {
    let mode_lower = mode.to_lowercase();
    let (text, alt, class) = match mode_lower.as_str() {
        "quiet" => (
            "🍃 Quiet".to_string(),
            "quiet".to_string(),
            "quiet".to_string(),
        ),
        "balanced" => (
            "⚖️ Bal".to_string(),
            "balanced".to_string(),
            "balanced".to_string(),
        ),
        "performance" | "high" => (
            "🚀 Perf".to_string(),
            "performance".to_string(),
            "performance".to_string(),
        ),
        "full" => (
            "🌪️ Full".to_string(),
            "full".to_string(),
            "full".to_string(),
        ),
        _ => (format!("⚙️ {mode}"), mode_lower.clone(), mode_lower.clone()),
    };

    let power_str = if on_ac { "AC" } else { "Battery" };
    let profile_desc = if mode_lower == "full" {
        "Full (Maximum Cooling)"
    } else {
        mode
    };
    let tooltip = format!(
        "Profile: {profile_desc}\nPower: {power_str}\nTemp: {temp_c}°C\nFan: {fan_rpm} RPM\nCharge Limit: {charge_limit}%"
    );

    WaybarPayload {
        text,
        alt,
        tooltip,
        class,
        percentage: charge_limit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_waybar_payload() {
        let payload = format_waybar_payload("Performance", true, 80, 55, 3200);
        assert_eq!(payload.text, "🚀 Perf");
        assert_eq!(payload.alt, "performance");
        assert_eq!(payload.class, "performance");
        assert_eq!(payload.percentage, 80);
        assert!(payload.tooltip.contains("Profile: Performance"));
        assert!(payload.tooltip.contains("Power: AC"));
        assert!(payload.tooltip.contains("Temp: 55°C"));
        assert!(payload.tooltip.contains("Fan: 3200 RPM"));
        assert!(payload.tooltip.contains("Charge Limit: 80%"));

        let payload_full = format_waybar_payload("Full", true, 80, 70, 5600);
        assert_eq!(payload_full.text, "🌪️ Full");
        assert_eq!(payload_full.alt, "full");
        assert_eq!(payload_full.class, "full");
        assert!(
            payload_full
                .tooltip
                .contains("Profile: Full (Maximum Cooling)")
        );

        let payload_quiet = format_waybar_payload("Quiet", false, 80, 42, 0);
        assert_eq!(payload_quiet.text, "🍃 Quiet");
        assert_eq!(payload_quiet.alt, "quiet");
        assert_eq!(payload_quiet.class, "quiet");

        let payload_bal = format_waybar_payload("Balanced", true, 80, 48, 2000);
        assert_eq!(payload_bal.text, "⚖️ Bal");
        assert_eq!(payload_bal.alt, "balanced");
        assert_eq!(payload_bal.class, "balanced");
    }

    #[test]
    fn test_status_payload_serialization() {
        let waybar = format_waybar_payload("Balanced", true, 80, 48, 2100);
        let payload = StatusPayload {
            text: waybar.text.clone(),
            alt: waybar.alt.clone(),
            tooltip: waybar.tooltip.clone(),
            class: waybar.class.clone(),
            percentage: waybar.percentage,
            firmware_mode: "Balanced".to_string(),
            charge_limit: 80,
            power_source: "AC".to_string(),
            on_ac: true,
            rgb: RgbPayload {
                hex: "#3DAEE9".to_string(),
                brightness: 100,
            },
            thermal: ThermalTelemetry {
                fan_rpm: 2100,
                fan2_rpm: Some(2100),
                temp_c: 48,
            },
            waybar,
        };

        let json = serde_json::to_string_pretty(&payload).expect("serialization succeeds");
        assert!(json.contains("\"text\": \"⚖️ Bal\""));
        assert!(json.contains("\"class\": \"balanced\""));
        assert!(json.contains("\"firmware_mode\": \"Balanced\""));
        assert!(json.contains("\"charge_limit\": 80"));
        assert!(json.contains("\"power_source\": \"AC\""));
        assert!(json.contains("\"on_ac\": true"));
        assert!(json.contains("\"hex\": \"#3DAEE9\""));
        assert!(json.contains("\"temp_c\": 48"));
        assert!(json.contains("\"waybar\":"));
    }
}
