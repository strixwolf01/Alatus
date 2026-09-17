// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::services::daemon_client::DaemonClientError;
use clap::Subcommand;

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

#[derive(Subcommand, Debug, Clone)]
pub enum DaemonTarget {
    /// Manage the desktop session background agent (alatus-session.service)
    Session {
        #[command(subcommand)]
        action: Option<DaemonServiceAction>,
    },
    /// Manage the system hardware daemon (alatusd.service)
    System {
        #[command(subcommand)]
        action: Option<DaemonServiceAction>,
    },
    /// Show status of both background daemons
    Status,
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum DaemonServiceAction {
    /// Start the service
    Start,
    /// Stop the service
    Stop,
    /// Restart the service
    Restart,
    /// Query service status
    Status,
    /// Run the daemon in foreground (used by systemd user unit)
    #[command(hide = true)]
    Run,
}

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

pub fn parse_bool(s: &str) -> Result<bool, String> {
    match s.to_lowercase().as_str() {
        "1" | "true" | "on" | "yes" | "enable" | "enabled" => Ok(true),
        "0" | "false" | "off" | "no" | "disable" | "disabled" => Ok(false),
        _ => Err(format!(
            "Invalid boolean: '{s}'. Use 'on'/'off' or 'true'/'false'."
        )),
    }
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
            let status = crate::services::desktop_session::query_session_status().await;
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
            crate::services::desktop_session::trigger_session_pixel_refresh()
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
                crate::services::desktop_session::set_session_accent_sync(enable)
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
                crate::services::desktop_session::set_session_auto_refresh(enable)
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
                crate::services::desktop_session::set_session_oled_care(enable)
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

            crate::services::desktop_session::run_desktop_session(
                crate::services::desktop_session::DesktopSessionOptions {
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

pub async fn handle_oled(action: Option<OledAction>) -> Result<(), DaemonClientError> {
    match action.unwrap_or(OledAction::Status) {
        OledAction::Status => {
            let status = crate::services::desktop_session::query_session_status().await;
            println!("ASUS OLED Display Care Status");
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
            Ok(())
        }
        OledAction::Dim { state, dim, level } => {
            if let Some(lvl) = level {
                let clamped = lvl.clamp(10, 100);
                crate::services::desktop_session::set_session_oled_dim_level(clamped)
                    .await
                    .map_err(|e| {
                        DaemonClientError::IoError(format!("Failed to set OLED dimming level: {e}"))
                    })?;
                println!("OLED flicker-free dimming level set to {clamped}%.");
                return Ok(());
            }
            let enable = state.or(dim).unwrap_or(true);
            crate::services::desktop_session::set_session_oled_care(enable)
                .await
                .map_err(|e| {
                    DaemonClientError::IoError(format!("Failed to set OLED dimming: {e}"))
                })?;
            println!(
                "OLED idle dimming set to: {}",
                if enable { "enabled" } else { "disabled" }
            );
            Ok(())
        }
        OledAction::Refresh => {
            println!("Triggering OLED pixel refresh conditioning cycle...");
            crate::services::desktop_session::trigger_session_pixel_refresh()
                .await
                .map_err(|e| {
                    DaemonClientError::IoError(format!("Failed to trigger pixel refresh: {e}"))
                })?;
            println!("Pixel refresh cycle completed successfully.");
            Ok(())
        }
    }
}

pub async fn handle_display(action: Option<DisplayAction>) -> Result<(), DaemonClientError> {
    match action.unwrap_or(DisplayAction::Status) {
        DisplayAction::Status => {
            let info = crate::services::desktop_session::query_display_info().await;
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
            Ok(())
        }
        DisplayAction::Rate { target, enable } => {
            if target.eq_ignore_ascii_case("auto") {
                let on = enable.unwrap_or(true);
                crate::services::desktop_session::set_session_auto_refresh(on)
                    .await
                    .map_err(|e| {
                        DaemonClientError::IoError(format!("Failed to set auto refresh: {e}"))
                    })?;
                println!(
                    "Panel refresh rate auto-switching set to: {}",
                    if on {
                        "enabled (Dynamic AC/Battery matching)"
                    } else {
                        "disabled"
                    }
                );
            } else {
                let hz: f64 = target.parse().map_err(|_| {
                    DaemonClientError::InvalidArgument(format!(
                        "Invalid refresh rate '{target}'. Expected a number (e.g. 60, 120) or 'auto'."
                    ))
                })?;

                let _ = crate::services::desktop_session::set_session_auto_refresh(false).await;
                crate::services::desktop_session::set_panel_refresh_rate(hz)
                    .await
                    .map_err(|e| {
                        DaemonClientError::IoError(format!("Failed to switch refresh rate: {e}"))
                    })?;
                println!(
                    "Display panel refresh rate set to: {:.1} Hz (auto-switching disabled)",
                    hz
                );
            }
            Ok(())
        }
        DisplayAction::Dim { level } => {
            let clamped = level.clamp(10, 100);
            crate::services::desktop_session::set_session_oled_dim_level(clamped)
                .await
                .map_err(|e| {
                    DaemonClientError::IoError(format!("Failed to set display dimming: {e}"))
                })?;
            println!("Display flicker-free dimming set to {clamped}%.");
            Ok(())
        }
    }
}

pub async fn handle_daemon(target: DaemonTarget) -> Result<(), DaemonClientError> {
    match target {
        DaemonTarget::Session { action } => {
            let act = action.unwrap_or(DaemonServiceAction::Status);
            if act == DaemonServiceAction::Run {
                tracing_subscriber::fmt::init();
                return crate::services::desktop_session::run_desktop_session(
                    crate::services::desktop_session::DesktopSessionOptions {
                        sync_accent: true,
                        auto_refresh: true,
                        oled_care: true,
                    },
                )
                .await
                .map_err(|e| DaemonClientError::IoError(e.to_string()));
            }

            let cmd_arg = match act {
                DaemonServiceAction::Start => "start",
                DaemonServiceAction::Stop => "stop",
                DaemonServiceAction::Restart => "restart",
                DaemonServiceAction::Status => {
                    let output = std::process::Command::new("systemctl")
                        .args(["--user", "is-active", "alatus-session.service"])
                        .output()
                        .map_err(|e| {
                            DaemonClientError::IoError(format!("Failed to execute systemctl: {e}"))
                        })?;
                    let state = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    let dbus_active =
                        crate::services::desktop_session::is_session_daemon_running().await;
                    println!("Alatus Desktop Session Service");
                    println!("──────────────────────────────");
                    println!("Systemd Unit:       alatus-session.service (user)");
                    println!(
                        "State:              {}",
                        if state.is_empty() {
                            "unknown / inactive"
                        } else {
                            &state
                        }
                    );
                    println!(
                        "D-Bus Name:         io.strixwolf.alatus.Session ({})",
                        if dbus_active { "Active" } else { "Inactive" }
                    );
                    return Ok(());
                }
                DaemonServiceAction::Run => unreachable!(),
            };

            let status = std::process::Command::new("systemctl")
                .args(["--user", cmd_arg, "alatus-session.service"])
                .status()
                .map_err(|e| {
                    DaemonClientError::IoError(format!(
                        "Failed to execute systemctl {cmd_arg}: {e}"
                    ))
                })?;

            if !status.success() {
                return Err(DaemonClientError::IoError(format!(
                    "systemctl --user {cmd_arg} alatus-session.service returned non-zero exit status: {status}"
                )));
            }

            println!(
                "Successfully executed 'systemctl --user {} alatus-session.service'",
                cmd_arg
            );
            Ok(())
        }
        DaemonTarget::System { action } => {
            let act = action.unwrap_or(DaemonServiceAction::Status);
            let cmd_arg = match act {
                DaemonServiceAction::Start => "start",
                DaemonServiceAction::Stop => "stop",
                DaemonServiceAction::Restart => "restart",
                DaemonServiceAction::Status => {
                    let output = std::process::Command::new("systemctl")
                        .args(["is-active", "alatusd.service"])
                        .output()
                        .map_err(|e| {
                            DaemonClientError::IoError(format!("Failed to execute systemctl: {e}"))
                        })?;
                    let state = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    println!("Alatus System Hardware Daemon");
                    println!("─────────────────────────────");
                    println!("Systemd Unit:       alatusd.service (system)");
                    println!(
                        "State:              {}",
                        if state.is_empty() {
                            "unknown / inactive"
                        } else {
                            &state
                        }
                    );
                    println!("D-Bus Interface:    io.strixwolf.alatus.Daemon");
                    return Ok(());
                }
                DaemonServiceAction::Run => unreachable!(),
            };

            let status = std::process::Command::new("sudo")
                .args(["systemctl", cmd_arg, "alatusd.service"])
                .status()
                .map_err(|e| {
                    DaemonClientError::IoError(format!(
                        "Failed to execute systemctl {cmd_arg}: {e}"
                    ))
                })?;

            if !status.success() {
                return Err(DaemonClientError::IoError(format!(
                    "systemctl {cmd_arg} alatusd.service returned non-zero exit status: {status}"
                )));
            }

            println!(
                "Successfully executed 'systemctl {} alatusd.service'",
                cmd_arg
            );
            Ok(())
        }
        DaemonTarget::Status => {
            println!("Alatus Background Daemons Status");
            println!("────────────────────────────────");
            let sess_output = std::process::Command::new("systemctl")
                .args(["--user", "is-active", "alatus-session.service"])
                .output()
                .ok();
            let sess_state = sess_output
                .as_ref()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|| "inactive".to_string());
            println!("Desktop Session Agent:  {}", sess_state);

            let sys_output = std::process::Command::new("systemctl")
                .args(["is-active", "alatusd.service"])
                .output()
                .ok();
            let sys_state = sys_output
                .as_ref()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|| "inactive".to_string());
            println!("System Hardware Daemon: {}", sys_state);
            Ok(())
        }
    }
}
