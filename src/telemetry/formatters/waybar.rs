// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Waybar custom JSON module payload generator.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaybarPayload {
    pub text: String,
    pub alt: String,
    pub tooltip: String,
    pub class: String,
    pub percentage: u32,
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
}
