// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Battery sensor telemetry collection from Linux power_supply sysfs interfaces.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatteryTelemetry {
    pub status: String,
    pub capacity: u32,
    pub voltage_v: f64,
    pub current_a: f64,
    pub rate_watts: f64,
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
