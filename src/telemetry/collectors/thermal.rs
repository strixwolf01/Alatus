// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Thermal sensor telemetry collection from Linux hwmon sysfs interfaces.

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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_telemetry_cache_and_consecutive_error_recovery() {
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

