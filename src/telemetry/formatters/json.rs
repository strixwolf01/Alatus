// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Structured machine-readable JSON status payload models.

use super::waybar::WaybarPayload;
use crate::telemetry::collectors::ThermalTelemetry;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RgbPayload {
    pub hex: String,
    pub brightness: u32,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::formatters::waybar::format_waybar_payload;

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

