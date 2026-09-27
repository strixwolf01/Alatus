// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Display panel and OLED Care commands.

use clap::Subcommand;
use serde_json::json;

use crate::client::AlatusClient;
use crate::services::daemon_client::DaemonClientError;

#[derive(Subcommand, Debug, Clone)]
pub enum OledAction {
    /// Show OLED care status, cumulative panel hours, cycle counts, and idle dimming state
    Status,

    /// Toggle OLED idle dimming or specify target dimming percentage level
    Dim {
        /// Enable or disable idle dimming (on/off, true/false)
        #[arg(value_parser = parse_bool)]
        state: Option<bool>,

        /// Alternative flag to enable or disable idle dimming
        #[arg(long, value_parser = parse_bool)]
        dim: Option<bool>,

        /// Target dimming percentage level (10-100)
        #[arg(long, value_parser = clap::value_parser!(u32).range(10..=100))]
        level: Option<u32>,
    },

    /// Trigger an immediate OLED pixel refresh conditioning cycle
    #[command(alias = "clean")]
    Refresh,
}

#[derive(Subcommand, Debug, Clone)]
pub enum DisplayAction {
    /// Show active panel connector, current refresh rate, resolution, and auto-refresh state
    Status,

    /// Switch refresh rate in Hz (e.g. 60, 120) or toggle auto power-matching (auto [on|off])
    Rate {
        /// Target rate in Hz (e.g. 60, 120) or 'auto'
        target: String,

        /// When target is 'auto', optional state: 'on' or 'off' (default: on)
        #[arg(value_parser = parse_bool)]
        enable: Option<bool>,
    },

    /// Scale flicker-free luminance level at compositor level (10-100%)
    Dim {
        /// Luminance percentage (10 to 100)
        #[arg(value_parser = clap::value_parser!(u32).range(10..=100))]
        level: u32,
    },
}

pub fn parse_bool(s: &str) -> Result<bool, String> {
    match s.to_lowercase().as_str() {
        "1" | "true" | "on" | "enable" | "yes" => Ok(true),
        "0" | "false" | "off" | "disable" | "no" => Ok(false),
        other => Err(format!(
            "Invalid boolean value: '{other}'. Expected true/false, on/off, 1/0."
        )),
    }
}

pub async fn handle_oled(
    client: &AlatusClient,
    action: Option<OledAction>,
    json_output: bool,
) -> Result<(), DaemonClientError> {
    match action.unwrap_or(OledAction::Status) {
        OledAction::Status => {
            let status = client.query_session_status().await;
            if json_output {
                println!(
                    "{}",
                    json!({
                        "session_running": status.running,
                        "oled_care_active": status.oled_care,
                        "pixel_refresh_hours": status.pixel_refresh_hours,
                        "pixel_refresh_count": status.pixel_refresh_count,
                        "oled_dim_level": status.oled_dim_level,
                    })
                );
            } else {
                println!("OLED Display Care Status");
                println!("─────────────────────────────");
                println!(
                    "Session Daemon:     {}",
                    if status.running {
                        "Active (io.strixwolf.alatus.Session)"
                    } else {
                        "Inactive (session daemon not running)"
                    }
                );
                println!(
                    "Idle Dimming:       {}",
                    if status.oled_care {
                        "ACTIVE"
                    } else {
                        "DISABLED"
                    }
                );
                println!(
                    "Active Screen Time: {:.1} hours",
                    status.pixel_refresh_hours
                );
                println!(
                    "Refresh Cycles:     {} completed",
                    status.pixel_refresh_count
                );
            }
            Ok(())
        }
        OledAction::Dim { state, dim, level } => {
            if let Some(lvl) = level {
                let clamped = lvl.clamp(10, 100);
                client
                    .set_oled_dim_level(clamped)
                    .await
                    .map_err(|e| {
                        DaemonClientError::IoError(format!("Failed to set OLED dimming level: {e}"))
                    })?;
                if json_output {
                    println!("{}", json!({ "success": true, "dim_level": clamped }));
                } else {
                    println!("OLED flicker-free dimming level set to {clamped}%.");
                }
                return Ok(());
            }
            let enable = state.or(dim).unwrap_or(true);
            client
                .set_oled_care(enable)
                .await
                .map_err(|e| {
                    DaemonClientError::IoError(format!("Failed to set OLED dimming: {e}"))
                })?;
            if json_output {
                println!("{}", json!({ "success": true, "idle_dimming": enable }));
            } else {
                println!(
                    "OLED idle dimming set to: {}",
                    if enable { "enabled" } else { "disabled" }
                );
            }
            Ok(())
        }
        OledAction::Refresh => {
            if !json_output {
                println!("Triggering OLED pixel refresh conditioning cycle...");
            }
            let res = client
                .trigger_pixel_refresh()
                .await
                .map_err(|e| {
                    DaemonClientError::IoError(format!("Failed to trigger pixel refresh: {e}"))
                })?;
            if json_output {
                println!("{}", json!({ "success": res }));
            } else {
                println!("Pixel refresh cycle completed successfully.");
            }
            Ok(())
        }
    }
}

pub async fn handle_display(
    client: &AlatusClient,
    action: Option<DisplayAction>,
    json_output: bool,
) -> Result<(), DaemonClientError> {
    match action.unwrap_or(DisplayAction::Status) {
        DisplayAction::Status => {
            let info = client.query_display_info().await;
            if json_output {
                println!(
                    "{}",
                    json!({
                        "connector": info.connector,
                        "width": info.width,
                        "height": info.height,
                        "refresh_rate": info.refresh_rate,
                        "auto_refresh": info.auto_refresh,
                    })
                );
            } else {
                println!("Display Panel Status");
                println!("────────────────────");
                println!("Connector:          {}", info.connector);
                println!("Resolution:         {}x{}", info.width, info.height);
                println!("Refresh Rate:       {:.1} Hz", info.refresh_rate);
                println!(
                    "Auto Refresh Rate:  {}",
                    if info.auto_refresh {
                        "ACTIVE (Dynamic AC/Battery matching)"
                    } else {
                        "DISABLED"
                    }
                );
            }
            Ok(())
        }
        DisplayAction::Rate { target, enable } => {
            if target.eq_ignore_ascii_case("auto") {
                let on = enable.unwrap_or(true);
                client
                    .set_auto_refresh(on)
                    .await
                    .map_err(|e| {
                        DaemonClientError::IoError(format!("Failed to set auto refresh: {e}"))
                    })?;
                if json_output {
                    println!("{}", json!({ "success": true, "auto_refresh": on }));
                } else {
                    println!(
                        "Panel refresh rate auto-switching set to: {}",
                        if on {
                            "enabled (Dynamic AC/Battery matching)"
                        } else {
                            "disabled"
                        }
                    );
                }
            } else {
                let hz: f64 = target.parse().map_err(|_| {
                    DaemonClientError::InvalidArgument(format!(
                        "Invalid refresh rate '{target}'. Expected a number (e.g. 60, 120) or 'auto'."
                    ))
                })?;

                let _ = client.set_auto_refresh(false).await;
                client
                    .set_refresh_rate(hz)
                    .await
                    .map_err(|e| {
                        DaemonClientError::IoError(format!("Failed to switch refresh rate: {e}"))
                    })?;
                if json_output {
                    println!("{}", json!({ "success": true, "refresh_rate": hz, "auto_refresh": false }));
                } else {
                    println!(
                        "Display panel refresh rate set to: {:.1} Hz (auto-switching disabled)",
                        hz
                    );
                }
            }
            Ok(())
        }
        DisplayAction::Dim { level } => {
            let clamped = level.clamp(10, 100);
            client
                .set_oled_dim_level(clamped)
                .await
                .map_err(|e| {
                    DaemonClientError::IoError(format!("Failed to set display dimming: {e}"))
                })?;
            if json_output {
                println!("{}", json!({ "success": true, "dim_level": clamped }));
            } else {
                println!("Display flicker-free dimming set to {clamped}%.");
            }
            Ok(())
        }
    }
}
