// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Display panel refresh rate coordination.

use crate::services::session::desktop::{
    apply_gnome_panel_refresh, apply_kde_panel_refresh, apply_wlroots_panel_refresh, detect_desktop,
    set_gnome_refresh_rate, set_kscreen_refresh_rate, set_wlroots_refresh_rate, DesktopEnv,
};

/// Automatically adjusts display panel refresh rate for internal eDP panels based on power state.
pub async fn apply_panel_refresh_rate(on_ac: bool) {
    let desktop = detect_desktop();
    match desktop {
        DesktopEnv::Kde => {
            apply_kde_panel_refresh(on_ac).await;
        }
        DesktopEnv::Gnome => {
            apply_gnome_panel_refresh(on_ac).await;
        }
        DesktopEnv::Wlroots => {
            apply_wlroots_panel_refresh(on_ac).await;
        }
        DesktopEnv::Other => {
            tracing::info!(
                "Non-KDE/GNOME desktop detected. Skipping auto-panel refresh (OnAC={on_ac})"
            );
        }
    }
}

/// Explicitly sets the refresh rate for the internal display panel.
pub async fn set_panel_refresh_rate(target_hz: f64) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let desktop = detect_desktop();
    match desktop {
        DesktopEnv::Kde => {
            set_kscreen_refresh_rate(target_hz).await?;
        }
        DesktopEnv::Gnome => {
            set_gnome_refresh_rate(target_hz).await?;
        }
        DesktopEnv::Wlroots => {
            set_wlroots_refresh_rate(target_hz).await?;
        }
        DesktopEnv::Other => {
            tracing::warn!("Manual refresh rate switching not supported on desktop {desktop:?}");
        }
    }
    Ok(())
}
