// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Wlroots (Hyprland / Sway) display management integration.

use std::process::Command;

pub async fn apply_wlroots_panel_refresh(on_ac: bool) {
    tracing::info!(
        "Wlroots desktop detected (Hyprland/Sway). Panel refresh rate is managed via compositor config or wlr-randr (OnAC={on_ac})"
    );
}

pub async fn set_wlroots_refresh_rate(hz: f64) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let output = Command::new("wlr-randr").arg("--json").output();
    if let Ok(out) = output
        && out.status.success()
        && let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out.stdout)
        && let Some(arr) = json.as_array()
    {
        for head in arr {
            let name = head.get("name").and_then(|n| n.as_str()).unwrap_or("");
            if name.starts_with("eDP") {
                let status = Command::new("wlr-randr")
                    .args(["--output", name, "--mode", &format!("--custom-mode-rate={:.1}", hz)])
                    .status();
                if let Ok(s) = status
                    && s.success()
                {
                    return Ok(());
                }
            }
        }
    }
    Err("Failed to set refresh rate on wlroots compositor".into())
}
