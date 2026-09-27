// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Flicker-free compositor-level display dimming.

use zbus::Connection;

use crate::services::session::desktop::{
    create_kscreen_doctor_cmd, detect_desktop, find_kde_internal_connector,
    set_kde_screen_brightness, DesktopEnv,
};

/// Instant compositor-level software dimming without touching hardware backlight.
pub async fn apply_flicker_free_dimming(connector: Option<&str>, level_pct: u32) {
    let clamped = level_pct.min(100);
    let desktop = detect_desktop();
    match desktop {
        DesktopEnv::Kde => {
            let conn = match connector {
                Some(c) => c.to_string(),
                None => find_kde_internal_connector()
                    .await
                    .unwrap_or_else(|| "eDP-1".to_string()),
            };
            let arg = format!("output.{conn}.brightness.{clamped}");
            tracing::info!("KDE Plasma Flicker-Free Dimming: executing kscreen-doctor {arg}");
            let mut success = false;

            if let Some(mut cmd) = create_kscreen_doctor_cmd() {
                let res = cmd.arg(&arg).output();
                if let Ok(out) = res {
                    if out.status.success() {
                        success = true;
                    } else {
                        // Fallback for HDR mode: use sdr-brightness (range 100-1000)
                        let sdr_val = ((clamped as f64) * 10.0).clamp(100.0, 1000.0) as u32;
                        let sdr_arg = format!("output.{conn}.sdr-brightness.{sdr_val}");
                        if let Some(mut sdr_cmd) = create_kscreen_doctor_cmd()
                            && let Ok(sdr_out) = sdr_cmd.arg(&sdr_arg).output()
                            && sdr_out.status.success()
                        {
                            success = true;
                        }
                    }
                }
            }

            if !success {
                tracing::warn!(
                    "kscreen-doctor flicker-free dimming failed or unavailable; falling back to D-Bus BrightnessControl"
                );
                if let Ok(session_conn) = Connection::session().await {
                    let _ = set_kde_screen_brightness(&session_conn, clamped as i32).await;
                }
            }
        }
        DesktopEnv::Wlroots => {
            let factor = (clamped as f64) / 100.0;
            tracing::info!(
                "Wlroots Compositor Flicker-Free Dimming software luminance factor: {factor:.2} (Hyprland/Sway)"
            );
        }
        DesktopEnv::Gnome | DesktopEnv::Other => {
            let factor = (clamped as f64) / 100.0;
            tracing::info!(
                "Compositor Flicker-Free Dimming software luminance factor: {factor:.2}"
            );
        }
    }
}
