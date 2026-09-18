// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Hardware sensor telemetry and presentation payload formatting.
//!
//! Provides read-only sysfs queries for CPU temperature, fan RPMs, battery charge rates,
//! and structured JSON / Waybar formatting.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

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

#[derive(Debug, Clone, Default)]
struct CachedHwmonSensors {
    hwmon_dir: PathBuf,
    fan1_input: Option<PathBuf>,
    fan2_input: Option<PathBuf>,
    temp1_input: Option<PathBuf>,
    consecutive_errors: u32,
    last_known: Option<ThermalTelemetry>,
}

static TELEMETRY_CACHE: Mutex<Option<CachedHwmonSensors>> = Mutex::new(None);

/// Resets cached sensor paths and telemetry state (e.g. on resume from sleep or hardware rebind).
pub fn reset_telemetry_cache() {
    if let Ok(mut guard) = TELEMETRY_CACHE.lock() {
        *guard = None;
    }
    tracing::info!("Thermal telemetry sensor cache reset");
}

fn scan_hwmon_paths(hwmon_dir: &Path) -> (Option<PathBuf>, Option<PathBuf>, Option<PathBuf>) {
    let mut fan1: Option<PathBuf> = None;
    let mut fan2: Option<PathBuf> = None;
    let mut temp: Option<PathBuf> = None;
    let mut preferred_temp: Option<PathBuf> = None;

    let Ok(entries) = fs::read_dir(hwmon_dir) else {
        return (None, None, None);
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = fs::read_to_string(path.join("name"))
            .unwrap_or_default()
            .trim()
            .to_string();

        let p_fan1 = path.join("fan1_input");
        let p_fan2 = path.join("fan2_input");

        // Verify fan1_input can actually be read without ENODEV
        if (fan1.is_none() || name == "asus")
            && p_fan1.exists()
            && let Ok(content) = fs::read_to_string(&p_fan1)
            && content.trim().parse::<u32>().is_ok()
        {
            fan1 = Some(p_fan1);
        }

        // Verify fan2_input can actually be read
        if (fan2.is_none() || name == "asus")
            && p_fan2.exists()
            && let Ok(content) = fs::read_to_string(&p_fan2)
            && content.trim().parse::<u32>().is_ok()
        {
            fan2 = Some(p_fan2);
        }

        // Check for temperature inputs
        let temp_candidates = [
            path.join("temp1_input"),
            path.join("temp2_input"),
            path.join("temp3_input"),
        ];

        for p_temp in temp_candidates {
            if p_temp.exists()
                && let Ok(content) = fs::read_to_string(&p_temp)
                && content.trim().parse::<i32>().is_ok()
            {
                if ["coretemp", "k10temp", "zenpower", "cpu_thermal", "asus"]
                    .contains(&name.as_str())
                {
                    preferred_temp = Some(p_temp);
                } else if temp.is_none() || name == "acpitz" {
                    temp = Some(p_temp);
                }
                break;
            }
        }
    }

    (fan1, fan2, preferred_temp.or(temp))
}

/// Safely inspects `/sys/class/hwmon` for CPU temperature and fan RPMs.
pub fn read_thermal_telemetry() -> ThermalTelemetry {
    read_thermal_telemetry_from(Path::new("/sys/class/hwmon"))
}

/// Safely inspects the given hwmon directory for CPU temperature and fan RPMs with caching and error resilience.
pub fn read_thermal_telemetry_from(hwmon_dir: &Path) -> ThermalTelemetry {
    let mut guard = match TELEMETRY_CACHE.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };

    let should_scan = match guard.as_ref() {
        Some(cached) => {
            cached.hwmon_dir != hwmon_dir
                || cached.consecutive_errors >= 3
                || (cached.fan1_input.is_none() || cached.temp1_input.is_none())
        }
        None => true,
    };

    if should_scan {
        let (fan1_p, fan2_p, temp_p) = scan_hwmon_paths(hwmon_dir);
        let last_known = guard.as_ref().and_then(|c| c.last_known.clone());
        *guard = Some(CachedHwmonSensors {
            hwmon_dir: hwmon_dir.to_path_buf(),
            fan1_input: fan1_p,
            fan2_input: fan2_p,
            temp1_input: temp_p,
            consecutive_errors: 0,
            last_known,
        });
    }

    let Some(cached) = guard.as_mut() else {
        return ThermalTelemetry {
            fan_rpm: 0,
            fan2_rpm: None,
            temp_c: 0,
        };
    };

    let mut fan1: Option<u32> = None;
    let mut fan2: Option<u32> = None;
    let mut temp: Option<i32> = None;
    let mut had_error = false;

    if let Some(ref p) = cached.fan1_input {
        match fs::read_to_string(p) {
            Ok(s) => match s.trim().parse::<u32>() {
                Ok(rpm) => fan1 = Some(rpm),
                Err(e) => {
                    tracing::warn!("Failed parsing fan1 RPM from {}: {e}", p.display());
                    had_error = true;
                }
            },
            Err(e) => {
                tracing::warn!("Failed reading fan1 from {}: {e}", p.display());
                had_error = true;
            }
        }
    }

    if let Some(ref p) = cached.fan2_input {
        match fs::read_to_string(p) {
            Ok(s) => match s.trim().parse::<u32>() {
                Ok(rpm) => fan2 = Some(rpm),
                Err(e) => {
                    tracing::warn!("Failed parsing fan2 RPM from {}: {e}", p.display());
                    had_error = true;
                }
            },
            Err(e) => {
                tracing::warn!("Failed reading fan2 from {}: {e}", p.display());
                had_error = true;
            }
        }
    }

    if let Some(ref p) = cached.temp1_input {
        match fs::read_to_string(p) {
            Ok(s) => match s.trim().parse::<i32>() {
                Ok(mc) => temp = Some(mc / 1000),
                Err(e) => {
                    tracing::warn!("Failed parsing temp from {}: {e}", p.display());
                    had_error = true;
                }
            },
            Err(e) => {
                tracing::warn!("Failed reading temp from {}: {e}", p.display());
                had_error = true;
            }
        }
    }

    if had_error {
        cached.consecutive_errors = cached.consecutive_errors.saturating_add(1);
        tracing::warn!(
            "Thermal telemetry read error encountered (consecutive: {})",
            cached.consecutive_errors
        );
        if cached.consecutive_errors >= 3 {
            tracing::warn!(
                "Consecutive thermal read errors reached threshold (3). Invalidating cached hwmon paths."
            );
            cached.fan1_input = None;
            cached.fan2_input = None;
            cached.temp1_input = None;
        }
    } else if fan1.is_some() || temp.is_some() {
        cached.consecutive_errors = 0;
    }

    let result = if had_error && let Some(ref last) = cached.last_known {
        ThermalTelemetry {
            fan_rpm: fan1.unwrap_or(last.fan_rpm),
            fan2_rpm: fan2.or(last.fan2_rpm),
            temp_c: temp.unwrap_or(last.temp_c),
        }
    } else {
        ThermalTelemetry {
            fan_rpm: fan1.unwrap_or(0),
            fan2_rpm: fan2,
            temp_c: temp.unwrap_or(0),
        }
    };

    if !had_error && (result.fan_rpm > 0 || result.temp_c > 0) {
        cached.last_known = Some(result.clone());
    }

    result
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

    #[test]
    fn test_telemetry_cache_and_consecutive_error_recovery() {
        use tempfile::tempdir;

        let tmp = tempdir().expect("tempdir");
        let hwmon0 = tmp.path().join("hwmon0");
        fs::create_dir_all(&hwmon0).unwrap();
        fs::write(hwmon0.join("name"), "asus\n").unwrap();
        fs::write(hwmon0.join("fan1_input"), "3200\n").unwrap();
        fs::write(hwmon0.join("temp1_input"), "52000\n").unwrap();

        // Fresh read
        reset_telemetry_cache();
        let t1 = read_thermal_telemetry_from(tmp.path());
        assert_eq!(t1.fan_rpm, 3200);
        assert_eq!(t1.temp_c, 52);

        // Delete the fan file to simulate broken file / ENODEV
        fs::remove_file(hwmon0.join("fan1_input")).unwrap();

        // 1st error: should fall back to last_known fan_rpm
        let t2 = read_thermal_telemetry_from(tmp.path());
        assert_eq!(t2.fan_rpm, 3200);
        assert_eq!(t2.temp_c, 52);

        // 2nd error: still falls back to last_known
        let t3 = read_thermal_telemetry_from(tmp.path());
        assert_eq!(t3.fan_rpm, 3200);

        // 3rd error: threshold reached, cache invalidated
        let t4 = read_thermal_telemetry_from(tmp.path());
        assert_eq!(t4.fan_rpm, 3200);

        // Recreate fan with new RPM to prove dynamic recovery on re-scan
        fs::write(hwmon0.join("fan1_input"), "4100\n").unwrap();
        let t5 = read_thermal_telemetry_from(tmp.path());
        assert_eq!(t5.fan_rpm, 4100);
        assert_eq!(t5.temp_c, 52);

        // Explicit cache reset works
        reset_telemetry_cache();
        let t6 = read_thermal_telemetry_from(tmp.path());
        assert_eq!(t6.fan_rpm, 4100);
    }
}
