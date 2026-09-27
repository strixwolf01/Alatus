// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Daemon and session background service lifecycle management.

use clap::Subcommand;
use serde_json::json;

use crate::services::daemon_client::DaemonClientError;

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

pub async fn handle_daemon(
    target: DaemonTarget,
    json_output: bool,
) -> Result<(), DaemonClientError> {
    match target {
        DaemonTarget::Session { action } => {
            let act = action.unwrap_or(DaemonServiceAction::Status);
            if act == DaemonServiceAction::Run {
                tracing_subscriber::fmt::init();
                return crate::services::session::run_desktop_session(
                    crate::services::session::DesktopSessionOptions {
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
                        crate::services::session::is_session_daemon_running().await;
                    if json_output {
                        println!(
                            "{}",
                            json!({
                                "service": "alatus-session.service",
                                "scope": "user",
                                "state": if state.is_empty() { "inactive" } else { &state },
                                "dbus_active": dbus_active
                            })
                        );
                    } else {
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
                    }
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

            if json_output {
                println!("{}", json!({ "success": true, "action": cmd_arg, "service": "alatus-session.service" }));
            } else {
                println!(
                    "Successfully executed 'systemctl --user {} alatus-session.service'",
                    cmd_arg
                );
            }
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
                    if json_output {
                        println!(
                            "{}",
                            json!({
                                "service": "alatusd.service",
                                "scope": "system",
                                "state": if state.is_empty() { "inactive" } else { &state }
                            })
                        );
                    } else {
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
                    }
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

            if json_output {
                println!("{}", json!({ "success": true, "action": cmd_arg, "service": "alatusd.service" }));
            } else {
                println!(
                    "Successfully executed 'systemctl {} alatusd.service'",
                    cmd_arg
                );
            }
            Ok(())
        }
        DaemonTarget::Status => {
            let sess_output = std::process::Command::new("systemctl")
                .args(["--user", "is-active", "alatus-session.service"])
                .output()
                .ok();
            let sess_state = sess_output
                .as_ref()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|| "inactive".to_string());

            let sys_output = std::process::Command::new("systemctl")
                .args(["is-active", "alatusd.service"])
                .output()
                .ok();
            let sys_state = sys_output
                .as_ref()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|| "inactive".to_string());

            if json_output {
                println!(
                    "{}",
                    json!({
                        "session_agent": sess_state,
                        "system_daemon": sys_state
                    })
                );
            } else {
                println!("Alatus Background Daemons Status");
                println!("────────────────────────────────");
                println!("Desktop Session Agent:  {}", sess_state);
                println!("System Hardware Daemon: {}", sys_state);
            }
            Ok(())
        }
    }
}
