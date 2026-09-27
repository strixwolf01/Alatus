// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Desktop session integration CLI commands.

use clap::Subcommand;
use crate::services::daemon_client::DaemonClientError;
use super::display::parse_bool;

#[derive(Subcommand, Debug, Clone)]
pub enum SessionAction {
    /// Show active session agent status and configuration
    Status,

    /// Dynamically configure session subsystems on the running agent
    Set {
        /// Configure accent color sync (on/off, true/false)
        #[arg(long, value_parser = parse_bool)]
        accent: Option<bool>,

        /// Configure auto panel refresh rate switching (on/off, true/false)
        #[arg(long, value_parser = parse_bool)]
        refresh: Option<bool>,

        /// Configure OLED Care and idle dimming (on/off, true/false)
        #[arg(long = "oled-care", value_parser = parse_bool)]
        oled_care: Option<bool>,
    },

    /// Trigger an immediate OLED pixel refresh conditioning cycle
    #[command(alias = "refresh-pixels")]
    PixelRefresh,
}

pub async fn handle_session(
    action: Option<SessionAction>,
    sync_accent: bool,
    no_accent: bool,
    auto_refresh: bool,
    no_refresh: bool,
    oled_care: bool,
    no_oled_care: bool,
) -> Result<(), DaemonClientError> {
    match action {
        Some(SessionAction::Status) => {
            let status = crate::services::session::query_session_status().await;
            println!("Alatus Desktop Session Status");
            println!("─────────────────────────────");
            println!("Desktop Environment:      {:?}", status.desktop);
            println!(
                "Session Daemon:           {}",
                if status.running {
                    "Active (io.strixwolf.alatus.Session)"
                } else {
                    "Inactive (standalone / not running)"
                }
            );
            println!(
                "[Power Monitor]:          {}",
                if status.power_monitor_active || status.running {
                    "ACTIVE (Live uevent + D-Bus)"
                } else {
                    "STANDBY (Live uevent + D-Bus)"
                }
            );
            println!(
                "[Panel Refresh Switching]: {}",
                if status.auto_refresh {
                    "ACTIVE"
                } else {
                    "DISABLED"
                }
            );
            println!(
                "[Desktop Accent Sync]:    {}",
                if status.sync_accent {
                    "ACTIVE"
                } else {
                    "DISABLED"
                }
            );
            println!(
                "[OLED Care]:              {}",
                if status.oled_care {
                    "ACTIVE"
                } else {
                    "DISABLED"
                }
            );
            println!(
                "[Pixel Refresh]:          {:.1} hrs active ({} cycle{} completed)",
                status.pixel_refresh_hours,
                status.pixel_refresh_count,
                if status.pixel_refresh_count == 1 {
                    ""
                } else {
                    "s"
                }
            );
            Ok(())
        }
        Some(SessionAction::PixelRefresh) => {
            println!("Triggering OLED pixel refresh conditioning cycle...");
            crate::services::session::trigger_session_pixel_refresh()
                .await
                .map_err(|e| {
                    DaemonClientError::IoError(format!("Failed to trigger pixel refresh: {e}"))
                })?;
            println!("Pixel refresh cycle completed successfully.");
            Ok(())
        }
        Some(SessionAction::Set {
            accent,
            refresh,
            oled_care,
        }) => {
            if accent.is_none() && refresh.is_none() && oled_care.is_none() {
                println!(
                    "No changes specified. Use --accent <on|off>, --refresh <on|off>, or --oled-care <on|off>."
                );
                return Ok(());
            }

            if let Some(enable) = accent {
                crate::services::session::set_session_accent_sync(enable)
                    .await
                    .map_err(|e| {
                        DaemonClientError::IoError(format!("Failed to set accent sync: {e}"))
                    })?;
                println!(
                    "Desktop Accent Sync set to: {}",
                    if enable { "enabled" } else { "disabled" }
                );
            }

            if let Some(enable) = refresh {
                crate::services::session::set_session_auto_refresh(enable)
                    .await
                    .map_err(|e| {
                        DaemonClientError::IoError(format!("Failed to set auto refresh: {e}"))
                    })?;
                println!(
                    "Panel Refresh Switching set to: {}",
                    if enable { "enabled" } else { "disabled" }
                );
            }

            if let Some(enable) = oled_care {
                crate::services::session::set_session_oled_care(enable)
                    .await
                    .map_err(|e| {
                        DaemonClientError::IoError(format!("Failed to set OLED Care: {e}"))
                    })?;
                println!(
                    "OLED Care set to: {}",
                    if enable { "enabled" } else { "disabled" }
                );
            }

            Ok(())
        }
        None => {
            let _ = (sync_accent, auto_refresh, oled_care);
            let effective_sync_accent = !no_accent;
            let effective_auto_refresh = !no_refresh;
            let effective_oled_care = !no_oled_care;

            crate::services::session::run_desktop_session(
                crate::services::session::DesktopSessionOptions {
                    sync_accent: effective_sync_accent,
                    auto_refresh: effective_auto_refresh,
                    oled_care: effective_oled_care,
                },
            )
            .await
            .map_err(|e| DaemonClientError::IoError(e.to_string()))
        }
    }
}
