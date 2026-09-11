// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Standalone native command-line interface for Alatus.
//!
//! Provides CLI controls for ASUS Linux hardware management including power/fan profiles,
//! battery charge limiting, deep sleep states, keyboard RGB lighting, and status diagnostics via D-Bus.

use alatus::services::daemon_client::{DaemonClient, DaemonClientError};
use alatus::services::firmware_mode::FirmwareMode;
use alatus::services::telemetry::{
    RgbPayload, StatusPayload, format_waybar_payload, read_battery_telemetry,
    read_thermal_telemetry,
};
use clap::{Parser, Subcommand, ValueEnum};
use std::process;

#[derive(Parser, Debug)]
#[command(name = "alatus")]
#[command(author = "Alatus Contributors")]
#[command(version)]
#[command(about = "ASUS Linux hardware control CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Show system, firmware, battery, and lighting status
    Status {
        /// Output status formatted as JSON for scripting and Waybar integration
        #[arg(short = 'j', long, default_value_t = false)]
        json: bool,

        /// Stream live JSON status line-by-line on events / ticker (ideal for Waybar)
        #[arg(short = 'w', long, default_value_t = false)]
        watch: bool,
    },

    /// Stream live JSON status line-by-line (alias for 'alatus status -j -w')
    Watch,

    /// Live real-time hardware telemetry monitor
    Monitor {
        /// Polling interval in milliseconds
        #[arg(short, long, default_value_t = 1000)]
        interval_ms: u64,
    },

    /// Query or set firmware power/fan mode
    #[command(alias = "m")]
    Mode {
        #[command(subcommand)]
        action: Option<ModeAction>,
    },

    /// Query or set fan profile
    Fan {
        #[command(subcommand)]
        action: Option<FanAction>,
    },

    /// Cycle through thermal profiles: Quiet -> Balanced -> Performance -> Full -> Quiet
    Cycle,

    /// Shortcut: Switch directly to Quiet thermal profile
    #[command(alias = "silent")]
    Quiet,

    /// Shortcut: Switch directly to Balanced thermal profile
    #[command(alias = "bal")]
    Balanced,

    /// Shortcut: Switch directly to Performance thermal profile
    #[command(alias = "high", alias = "performance")]
    Perf,

    /// Shortcut: Switch directly to Full speed thermal profile
    #[command(alias = "max")]
    Full,

    /// OLED panel care, cumulative usage hours, idle dimming, and pixel refresh
    #[command(alias = "o")]
    Oled {
        #[command(subcommand)]
        action: Option<OledAction>,
    },

    /// Display panel resolution, refresh rate switching, and auto power-matching
    Display {
        #[command(subcommand)]
        action: Option<DisplayAction>,
    },

    /// Set battery charge limit threshold (0-100%)
    #[command(alias = "c", alias = "charge")]
    ChargeLimit {
        /// Battery charge threshold percentage (0-100)
        #[arg(value_parser = clap::value_parser!(u32))]
        percentage: u32,
    },

    /// Control keyboard RGB backlight and desktop accent color sync
    Rgb {
        #[command(subcommand)]
        action: RgbAction,
    },

    /// Manage system hardware daemon and user desktop session background services
    Daemon {
        #[command(subcommand)]
        target: DaemonTarget,
    },

    /// Generate shell auto-completion scripts for Bash, Zsh, or Fish
    Completions {
        /// Target shell
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },

    /// Run desktop session integration agent (hidden/legacy compatibility)
    #[command(hide = true)]
    Session {
        #[command(subcommand)]
        action: Option<SessionAction>,

        /// Enable XDG desktop accent color sync (default: true)
        #[arg(long, default_value_t = false)]
        sync_accent: bool,

        /// Disable XDG desktop accent color sync
        #[arg(long, default_value_t = false)]
        no_accent: bool,

        /// Enable auto display panel refresh rate switching (default: true)
        #[arg(long, default_value_t = false)]
        auto_refresh: bool,

        /// Disable auto display panel refresh rate switching
        #[arg(long, default_value_t = false)]
        no_refresh: bool,

        /// Enable OLED Care and idle dimming (default: true)
        #[arg(long, default_value_t = false)]
        oled_care: bool,

        /// Disable OLED Care and idle dimming
        #[arg(long, default_value_t = false)]
        no_oled_care: bool,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum OledAction {
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
enum DisplayAction {
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
enum DaemonTarget {
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
enum DaemonServiceAction {
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
enum SessionAction {
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

#[derive(Subcommand, Debug, PartialEq, Eq)]
enum ModeAction {
    /// Get current firmware mode
    Get,
    /// Set firmware mode (balanced, quiet, performance, full)
    Set {
        /// Mode profile name
        #[arg(value_enum)]
        name: ModeChoice,
    },
    /// Switch to Balanced profile
    #[command(alias = "bal")]
    Balanced,
    /// Switch to Quiet profile
    #[command(alias = "silent")]
    Quiet,
    /// Switch to Performance profile
    #[command(alias = "perf", alias = "high")]
    Performance,
    /// Switch to Full speed profile
    #[command(alias = "max")]
    Full,
    /// Cycle through profiles: Quiet -> Balanced -> Performance -> Full -> Quiet
    Cycle,
    /// Query or configure auto-switching thermal profile on power changes
    Auto {
        /// Enable or disable auto switching (on/off, true/false)
        #[arg(value_parser = parse_bool)]
        enable: Option<bool>,
    },
}

#[derive(Subcommand, Debug, PartialEq, Eq)]
enum FanAction {
    /// Get current fan profile
    Get,
    /// Set fan profile (balanced, quiet, performance, full)
    Set {
        /// Fan profile name
        #[arg(value_enum)]
        profile: ModeChoice,
    },
    /// Switch to Balanced profile
    #[command(alias = "bal")]
    Balanced,
    /// Switch to Quiet profile
    #[command(alias = "silent")]
    Quiet,
    /// Switch to Performance profile
    #[command(alias = "perf", alias = "high")]
    Performance,
    /// Switch to Full speed profile
    #[command(alias = "max")]
    Full,
    /// Cycle through fan profiles: Quiet -> Balanced -> Performance -> Full -> Quiet
    Cycle,
}

#[derive(Subcommand, Debug)]
enum RgbAction {
    /// Show current RGB status
    Status,

    /// Set static RGB color via HEX (#RRGGBB, RRGGBB) or RGB values (R,G,B)
    #[command(alias = "color")]
    SetColor {
        /// Color code, e.g. '#FF5500', 'FF5500', or '255,85,0'
        color: String,
    },

    /// Set RGB brightness level (0-100% or 0-3)
    #[command(alias = "brightness")]
    SetBrightness {
        /// Brightness level (0-100% or 0-3)
        #[arg(value_parser = clap::value_parser!(u32))]
        brightness: u32,
    },

    /// Turn off keyboard RGB backlight
    Off,

    /// Turn on keyboard RGB backlight
    On,

    /// Toggle desktop accent color synchronization with keyboard backlighting
    Sync {
        /// Enable or disable accent sync (on/off, true/false)
        #[arg(value_parser = parse_bool)]
        enable: Option<bool>,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum ModeChoice {
    /// Balanced standard performance
    Balanced,
    /// Quiet / silent operation
    Quiet,
    /// High performance boost
    #[value(alias = "high")]
    Performance,
    /// Maximum fan speed
    Full,
}

impl From<ModeChoice> for FirmwareMode {
    fn from(c: ModeChoice) -> Self {
        match c {
            ModeChoice::Balanced => FirmwareMode::Balanced,
            ModeChoice::Quiet => FirmwareMode::Quiet,
            ModeChoice::Performance => FirmwareMode::High,
            ModeChoice::Full => FirmwareMode::Full,
        }
    }
}

fn format_mode(mode: FirmwareMode) -> &'static str {
    match mode {
        FirmwareMode::Balanced => "Balanced",
        FirmwareMode::Quiet => "Quiet",
        FirmwareMode::High => "Performance",
        FirmwareMode::Full => "Full",
        FirmwareMode::Unknown(_) => "Unknown",
    }
}

pub fn parse_color(input: &str) -> Result<(u8, u8, u8), String> {
    let s = input.trim();
    let hex = s.strip_prefix('#').unwrap_or(s);
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        let r = u8::from_str_radix(&hex[0..2], 16).map_err(|e| e.to_string())?;
        let g = u8::from_str_radix(&hex[2..4], 16).map_err(|e| e.to_string())?;
        let b = u8::from_str_radix(&hex[4..6], 16).map_err(|e| e.to_string())?;
        return Ok((r, g, b));
    }

    let parts: Vec<&str> = s
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() == 3 {
        let r = parts[0]
            .parse::<u8>()
            .map_err(|_| "Invalid R component (must be 0-255)".to_string())?;
        let g = parts[1]
            .parse::<u8>()
            .map_err(|_| "Invalid G component (must be 0-255)".to_string())?;
        let b = parts[2]
            .parse::<u8>()
            .map_err(|_| "Invalid B component (must be 0-255)".to_string())?;
        return Ok((r, g, b));
    }

    Err("Invalid color format. Expected '#RRGGBB', 'RRGGBB', or 'R,G,B' (e.g. '#FF5500' or '255,85,0')".to_string())
}

fn parse_bool(s: &str) -> Result<bool, String> {
    match s.to_lowercase().as_str() {
        "1" | "true" | "on" | "yes" | "enable" | "enabled" => Ok(true),
        "0" | "false" | "off" | "no" | "disable" | "disabled" => Ok(false),
        _ => Err(format!(
            "Invalid boolean: '{s}'. Use 'on'/'off' or 'true'/'false'."
        )),
    }
}

async fn handle_session(
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
            let status = alatus::services::desktop_session::query_session_status().await;
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
            alatus::services::desktop_session::trigger_session_pixel_refresh()
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
                alatus::services::desktop_session::set_session_accent_sync(enable)
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
                alatus::services::desktop_session::set_session_auto_refresh(enable)
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
                alatus::services::desktop_session::set_session_oled_care(enable)
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

            alatus::services::desktop_session::run_desktop_session(
                alatus::services::desktop_session::DesktopSessionOptions {
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

async fn handle_oled(action: Option<OledAction>) -> Result<(), DaemonClientError> {
    match action.unwrap_or(OledAction::Status) {
        OledAction::Status => {
            let status = alatus::services::desktop_session::query_session_status().await;
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
                alatus::services::desktop_session::set_session_oled_dim_level(clamped)
                    .await
                    .map_err(|e| {
                        DaemonClientError::IoError(format!("Failed to set OLED dimming level: {e}"))
                    })?;
                println!("OLED flicker-free dimming level set to {clamped}%.");
                return Ok(());
            }
            let enable = state.or(dim).unwrap_or(true);
            alatus::services::desktop_session::set_session_oled_care(enable)
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
            alatus::services::desktop_session::trigger_session_pixel_refresh()
                .await
                .map_err(|e| {
                    DaemonClientError::IoError(format!("Failed to trigger pixel refresh: {e}"))
                })?;
            println!("Pixel refresh cycle completed successfully.");
            Ok(())
        }
    }
}

async fn handle_display(action: Option<DisplayAction>) -> Result<(), DaemonClientError> {
    match action.unwrap_or(DisplayAction::Status) {
        DisplayAction::Status => {
            let info = alatus::services::desktop_session::query_display_info().await;
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
                alatus::services::desktop_session::set_session_auto_refresh(on)
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

                let _ = alatus::services::desktop_session::set_session_auto_refresh(false).await;
                alatus::services::desktop_session::set_panel_refresh_rate(hz)
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
            alatus::services::desktop_session::set_session_oled_dim_level(clamped)
                .await
                .map_err(|e| {
                    DaemonClientError::IoError(format!("Failed to set display dimming: {e}"))
                })?;
            println!("Display flicker-free dimming set to {clamped}%.");
            Ok(())
        }
    }
}

async fn handle_daemon(target: DaemonTarget) -> Result<(), DaemonClientError> {
    match target {
        DaemonTarget::Session { action } => {
            let act = action.unwrap_or(DaemonServiceAction::Status);
            if act == DaemonServiceAction::Run {
                tracing_subscriber::fmt::init();
                return alatus::services::desktop_session::run_desktop_session(
                    alatus::services::desktop_session::DesktopSessionOptions {
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
                        alatus::services::desktop_session::is_session_daemon_running().await;
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

async fn build_status_payload(client: &DaemonClient) -> Result<StatusPayload, DaemonClientError> {
    let mode = client.get_firmware_mode().await?;
    let charge_limit = client.get_charge_limit().await?;
    let on_ac = client.get_on_ac().await.unwrap_or(true);
    let rgb_status = client.get_rgb_status().await.ok();

    let mode_str = format_mode(mode).to_string();
    let thermal = read_thermal_telemetry();
    let power_source = if on_ac { "AC" } else { "Battery" }.to_string();

    let (hex, brightness) = match &rgb_status {
        Some(rgb) if rgb.available => (
            format!("#{:02X}{:02X}{:02X}", rgb.red, rgb.green, rgb.blue),
            rgb.brightness,
        ),
        _ => ("#000000".to_string(), 0),
    };

    let waybar = format_waybar_payload(
        &mode_str,
        on_ac,
        charge_limit,
        thermal.temp_c,
        thermal.fan_rpm,
    );

    Ok(StatusPayload {
        text: waybar.text.clone(),
        alt: waybar.alt.clone(),
        tooltip: waybar.tooltip.clone(),
        class: waybar.class.clone(),
        percentage: waybar.percentage,
        firmware_mode: mode_str,
        charge_limit,
        power_source,
        on_ac,
        rgb: RgbPayload { hex, brightness },
        thermal,
        waybar,
    })
}

async fn emit_status_line(client: &DaemonClient) {
    use std::io::Write;
    if let Ok(payload) = build_status_payload(client).await {
        let json_res = serde_json::to_string(&payload);
        if let Ok(json) = json_res {
            println!("{}", json);
            let _ = std::io::stdout().flush();
        }
    }
}

async fn handle_status_watch(client: &DaemonClient) -> Result<(), DaemonClientError> {
    use futures_util::StreamExt;
    use tokio::time::{Duration, interval};
    use zbus::{MatchRule, MessageStream};

    // Emit immediately on startup
    emit_status_line(client).await;

    let conn = client.connection();

    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .path_namespace("/io/strixwolf/alatus")
        .map_err(|e| DaemonClientError::IoError(e.to_string()))?
        .build();

    let mut stream = MessageStream::for_match_rule(rule, conn, Some(16))
        .await
        .map_err(|e| DaemonClientError::IoError(e.to_string()))?;

    let mut ticker = interval(Duration::from_secs(3));
    ticker.tick().await;

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                break;
            }
            Some(_) = stream.next() => {
                emit_status_line(client).await;
            }
            _ = ticker.tick() => {
                emit_status_line(client).await;
            }
        }
    }

    Ok(())
}

async fn handle_status(
    client: &DaemonClient,
    json_mode: bool,
    watch: bool,
) -> Result<(), DaemonClientError> {
    if watch {
        return handle_status_watch(client).await;
    }

    let mode = client.get_firmware_mode().await?;
    let charge_limit = client.get_charge_limit().await?;
    let deep_sleep = client.get_deep_sleep_active().await?;
    let on_ac = client.get_on_ac().await.unwrap_or(true);
    let auto_th = client.get_auto_thermal_profile().await.unwrap_or(false);
    let rgb_status = client.get_rgb_status().await.ok();

    if json_mode {
        let payload = build_status_payload(client).await?;
        let json = serde_json::to_string_pretty(&payload)
            .map_err(|e| DaemonClientError::IoError(e.to_string()))?;
        println!("{}", json);
        return Ok(());
    }

    println!("Alatus System Status");
    println!("────────────────────");
    println!("Firmware Mode:  {}", format_mode(mode));
    println!("Charge Limit:   {}%", charge_limit);
    println!(
        "Deep Sleep:     {}",
        if deep_sleep { "Active" } else { "Inactive" }
    );
    println!(
        "Power Source:   {}",
        if on_ac { "AC Connected" } else { "Battery" }
    );
    println!(
        "Auto Thermal:   {}",
        if auto_th { "Enabled" } else { "Disabled" }
    );

    match rgb_status {
        Some(rgb) if rgb.available => {
            println!("RGB Device:     {} ({})", rgb.model, rgb.path);
            println!(
                "RGB Color:      #{:02X}{:02X}{:02X} @ {}%",
                rgb.red, rgb.green, rgb.blue, rgb.brightness
            );
        }
        _ => {
            println!("RGB Device:     None detected");
        }
    }

    Ok(())
}

async fn handle_monitor(client: &DaemonClient, interval_ms: u64) -> Result<(), DaemonClientError> {
    let interval = std::time::Duration::from_millis(interval_ms.max(100));

    // Hide cursor during monitoring
    print!("\x1B[?25l");
    let _ = std::io::Write::flush(&mut std::io::stdout());

    let res = run_monitor_loop(client, interval).await;

    // Restore cursor and finish with a newline
    println!("\x1B[?25h");
    let _ = std::io::Write::flush(&mut std::io::stdout());

    res
}

async fn run_monitor_loop(
    client: &DaemonClient,
    interval: std::time::Duration,
) -> Result<(), DaemonClientError> {
    loop {
        let mode = client
            .get_firmware_mode()
            .await
            .unwrap_or(FirmwareMode::Unknown(0));
        let charge_limit = client.get_charge_limit().await.unwrap_or(0);
        let deep_sleep = client.get_deep_sleep_active().await.unwrap_or(false);
        let on_ac = client.get_on_ac().await.unwrap_or(true);
        let auto_th = client.get_auto_thermal_profile().await.unwrap_or(false);
        let rgb_status = client.get_rgb_status().await.ok();

        let daemon_active = client.check_active().await.unwrap_or(false);

        let thermal = read_thermal_telemetry();
        let battery = read_battery_telemetry();

        // ANSI clear screen (\x1B[2J) and cursor home (\x1B[H)
        print!("\x1B[2J\x1B[H");

        println!("┌────────────────────────────────────────────────────────┐");
        println!("│               ALATUS HARDWARE MONITOR                  │");
        if !daemon_active {
            println!("│ [!] WARNING: alatusd service is NOT active             │");
        }
        println!("├────────────────────────────────────────────────────────┤");
        println!("│ Power & Profile:                                       │");
        println!("│   Firmware Mode:    {:<35}│", format_mode(mode));
        println!(
            "│   Auto-Thermal:     {:<35}│",
            if auto_th { "Enabled" } else { "Disabled" }
        );
        println!(
            "│   Power Source:     {:<35}│",
            if on_ac { "AC Connected" } else { "Battery" }
        );
        println!(
            "│   Deep Sleep:       {:<35}│",
            if deep_sleep { "Active" } else { "Inactive" }
        );
        println!("├────────────────────────────────────────────────────────┤");
        println!("│ Battery Telemetry:                                     │");
        println!(
            "│   Charge Limit:     {:<35}│",
            format!("{}%", charge_limit)
        );
        if let Some(bat) = battery {
            let bat_stat = format!("{} ({}%)", bat.status, bat.capacity);
            let rate_label = if bat.status.eq_ignore_ascii_case("charging") {
                "Charge Rate"
            } else {
                "Discharge Rate"
            };
            let bat_rate = format!(
                "{:.2} W ({:.2} V @ {:.2} A)",
                bat.rate_watts, bat.voltage_v, bat.current_a
            );
            println!("│   Battery Status:   {:<35}│", bat_stat);
            println!("│   {:<18}{:<35}│", format!("{}:", rate_label), bat_rate);
        } else {
            println!("│   Battery:          {:<35}│", "Not detected");
        }
        println!("├────────────────────────────────────────────────────────┤");
        println!("│ Thermal & Cooling:                                     │");
        println!(
            "│   CPU Temperature:  {:<35}│",
            format!("{}°C", thermal.temp_c)
        );
        println!(
            "│   Fan 1 RPM:        {:<35}│",
            format!("{} RPM", thermal.fan_rpm)
        );
        if let Some(f2) = thermal.fan2_rpm {
            println!("│   Fan 2 RPM:        {:<35}│", format!("{} RPM", f2));
        }
        println!("├────────────────────────────────────────────────────────┤");
        println!("│ Keyboard Lighting:                                     │");
        match rgb_status {
            Some(rgb) if rgb.available => {
                let rgb_str = format!(
                    "#{:02X}{:02X}{:02X} @ {}%",
                    rgb.red, rgb.green, rgb.blue, rgb.brightness
                );
                println!("│   RGB Backlight:    {:<35}│", rgb_str);
            }
            _ => {
                println!("│   RGB Backlight:    {:<35}│", "None detected");
            }
        }
        println!("└────────────────────────────────────────────────────────┘");
        println!(
            "  Press [Ctrl+C] to exit. Refresh: {}ms",
            interval.as_millis()
        );
        let _ = std::io::Write::flush(&mut std::io::stdout());

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                break;
            }
            _ = tokio::time::sleep(interval) => {}
        }
    }

    Ok(())
}

async fn handle_mode(
    client: &DaemonClient,
    action: Option<ModeAction>,
) -> Result<(), DaemonClientError> {
    match action.unwrap_or(ModeAction::Get) {
        ModeAction::Get => {
            let mode = client.get_firmware_mode().await?;
            println!("{}", format_mode(mode).to_lowercase());
        }
        ModeAction::Set { name } => {
            let target: FirmwareMode = name.into();
            client.set_firmware_mode(target).await?;
            println!("Firmware mode set to: {}", format_mode(target));
        }
        ModeAction::Balanced => {
            client.set_firmware_mode(FirmwareMode::Balanced).await?;
            println!("Firmware mode set to: Balanced");
        }
        ModeAction::Quiet => {
            client.set_firmware_mode(FirmwareMode::Quiet).await?;
            println!("Firmware mode set to: Quiet");
        }
        ModeAction::Performance => {
            client.set_firmware_mode(FirmwareMode::High).await?;
            println!("Firmware mode set to: Performance");
        }
        ModeAction::Full => {
            client.set_firmware_mode(FirmwareMode::Full).await?;
            println!("Firmware mode set to: Full");
        }
        ModeAction::Cycle => {
            let current = client.get_firmware_mode().await?;
            let next = match current {
                FirmwareMode::Quiet => FirmwareMode::Balanced,
                FirmwareMode::Balanced => FirmwareMode::High,
                FirmwareMode::High => FirmwareMode::Full,
                FirmwareMode::Full => FirmwareMode::Quiet,
                FirmwareMode::Unknown(_) => FirmwareMode::Balanced,
            };
            client.set_firmware_mode(next).await?;
            println!(
                "Switched profile: {} -> {}",
                format_mode(current),
                format_mode(next)
            );
        }
        ModeAction::Auto { enable } => match enable {
            Some(en) => {
                client.set_auto_thermal_profile(en).await?;
                println!(
                    "Auto-thermal profile switching: {}",
                    if en { "enabled" } else { "disabled" }
                );
            }
            None => {
                let en = client.get_auto_thermal_profile().await?;
                println!(
                    "Auto-thermal profile switching: {}",
                    if en { "enabled" } else { "disabled" }
                );
            }
        },
    }
    Ok(())
}

async fn handle_fan(
    client: &DaemonClient,
    action: Option<FanAction>,
) -> Result<(), DaemonClientError> {
    match action.unwrap_or(FanAction::Get) {
        FanAction::Get => {
            let mode = client.get_firmware_mode().await?;
            println!("{}", format_mode(mode).to_lowercase());
        }
        FanAction::Set { profile } => {
            let target: FirmwareMode = profile.into();
            client.set_firmware_mode(target).await?;
            println!("Fan profile set to: {}", format_mode(target));
        }
        FanAction::Balanced => {
            client.set_firmware_mode(FirmwareMode::Balanced).await?;
            println!("Fan profile set to: Balanced");
        }
        FanAction::Quiet => {
            client.set_firmware_mode(FirmwareMode::Quiet).await?;
            println!("Fan profile set to: Quiet");
        }
        FanAction::Performance => {
            client.set_firmware_mode(FirmwareMode::High).await?;
            println!("Fan profile set to: Performance");
        }
        FanAction::Full => {
            client.set_firmware_mode(FirmwareMode::Full).await?;
            println!("Fan profile set to: Full");
        }
        FanAction::Cycle => {
            let current = client.get_firmware_mode().await?;
            let next = match current {
                FirmwareMode::Quiet => FirmwareMode::Balanced,
                FirmwareMode::Balanced => FirmwareMode::High,
                FirmwareMode::High => FirmwareMode::Full,
                FirmwareMode::Full => FirmwareMode::Quiet,
                FirmwareMode::Unknown(_) => FirmwareMode::Balanced,
            };
            client.set_firmware_mode(next).await?;
            println!(
                "Switched profile: {} -> {}",
                format_mode(current),
                format_mode(next)
            );
        }
    }
    Ok(())
}

async fn handle_charge_limit(
    client: &DaemonClient,
    percentage: u32,
) -> Result<(), DaemonClientError> {
    if percentage > 100 {
        eprintln!(
            "Error: Charge limit must be between 0 and 100 (got: {}%)",
            percentage
        );
        process::exit(1);
    }

    client.set_charge_limit(percentage).await?;
    println!("Battery charge limit set to {}%", percentage);
    Ok(())
}

async fn handle_rgb(client: &DaemonClient, action: RgbAction) -> Result<(), DaemonClientError> {
    match action {
        RgbAction::Status => {
            let status = client.get_rgb_status().await?;
            println!("Keyboard RGB Status");
            println!("───────────────────");
            println!(
                "Hardware:   {}",
                if status.available {
                    "Connected"
                } else {
                    "Not detected"
                }
            );
            if status.available {
                println!("Model:      {}", status.model);
                println!("Device:     {}", status.path);
                println!(
                    "Color:      #{:02X}{:02X}{:02X} (RGB: {}, {}, {})",
                    status.red, status.green, status.blue, status.red, status.green, status.blue
                );
                println!("Brightness: {}%", status.brightness);
            }
        }
        RgbAction::SetColor { color } => {
            let (r, g, b) = parse_color(&color).map_err(DaemonClientError::InvalidArgument)?;
            client.set_rgb_color(r, g, b).await?;
            println!(
                "RGB color set to #{:02X}{:02X}{:02X} (RGB: {}, {}, {})",
                r, g, b, r, g, b
            );
        }
        RgbAction::SetBrightness { brightness } => {
            if brightness > 100 {
                return Err(DaemonClientError::InvalidArgument(
                    "Brightness must be between 0 and 100".to_string(),
                ));
            }
            client.set_rgb_brightness(brightness).await?;
            println!("RGB brightness set to {}%", brightness);
        }
        RgbAction::Off => {
            client.set_rgb_brightness(0).await?;
            println!("Keyboard RGB turned off");
        }
        RgbAction::On => {
            client.set_rgb_brightness(100).await?;
            println!("Keyboard RGB turned on");
        }
        RgbAction::Sync { enable } => {
            let on = enable.unwrap_or(true);
            alatus::services::desktop_session::set_session_accent_sync(on)
                .await
                .map_err(|e| {
                    DaemonClientError::IoError(format!("Failed to set accent sync: {e}"))
                })?;
            println!(
                "Desktop Accent Color Sync set to: {}",
                if on { "enabled" } else { "disabled" }
            );
        }
    }
    Ok(())
}

async fn handle_cycle(client: &DaemonClient) -> Result<(), DaemonClientError> {
    let current = client.get_firmware_mode().await?;
    let next = match current {
        FirmwareMode::Quiet => FirmwareMode::Balanced,
        FirmwareMode::Balanced => FirmwareMode::High,
        FirmwareMode::High => FirmwareMode::Full,
        FirmwareMode::Full => FirmwareMode::Quiet,
        FirmwareMode::Unknown(_) => FirmwareMode::Balanced,
    };
    client.set_firmware_mode(next).await?;
    println!(
        "Switched profile: {} -> {}",
        format_mode(current),
        format_mode(next)
    );
    Ok(())
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Some(Commands::Session {
        action,
        sync_accent,
        no_accent,
        auto_refresh,
        no_refresh,
        oled_care,
        no_oled_care,
    }) = cli.command
    {
        if action.is_none() {
            tracing_subscriber::fmt::init();
        }
        let result = handle_session(
            action,
            sync_accent,
            no_accent,
            auto_refresh,
            no_refresh,
            oled_care,
            no_oled_care,
        )
        .await;
        if let Err(e) = result {
            eprintln!("Error executing command: {}", e);
            process::exit(1);
        }
        return;
    }

    if let Some(Commands::Completions { shell }) = &cli.command {
        use clap::CommandFactory;
        let mut cmd = Cli::command();
        clap_complete::generate(*shell, &mut cmd, "alatus", &mut std::io::stdout());
        return;
    }

    match &cli.command {
        Some(Commands::Oled { action }) => {
            if let Err(e) = handle_oled(action.clone()).await {
                eprintln!("Error executing command: {}", e);
                eprintln!(
                    "Hint: Ensure 'alatus-session' is running: systemctl --user start alatus-session.service"
                );
                process::exit(1);
            }
            return;
        }
        Some(Commands::Display { action }) => {
            if let Err(e) = handle_display(action.clone()).await {
                eprintln!("Error executing command: {}", e);
                eprintln!(
                    "Hint: Ensure 'alatus-session' is running: systemctl --user start alatus-session.service"
                );
                process::exit(1);
            }
            return;
        }
        Some(Commands::Daemon { target }) => {
            if let Err(e) = handle_daemon(target.clone()).await {
                eprintln!("Error executing command: {}", e);
                process::exit(1);
            }
            return;
        }
        Some(Commands::Rgb {
            action: RgbAction::Sync { enable },
        }) => {
            let on = enable.unwrap_or(true);
            if let Err(e) = alatus::services::desktop_session::set_session_accent_sync(on).await {
                eprintln!("Error setting accent sync: {}", e);
                eprintln!(
                    "Hint: Ensure 'alatus-session' is running: systemctl --user start alatus-session.service"
                );
                process::exit(1);
            }
            println!(
                "Desktop Accent Color Sync set to: {}",
                if on { "enabled" } else { "disabled" }
            );
            return;
        }
        _ => {}
    }

    let client = match DaemonClient::connect().await {
        Ok(c) => c,
        Err(err) => {
            eprintln!("Error: Failed to connect to alatusd via D-Bus: {}", err);
            eprintln!("Remediation: Start it with: sudo systemctl start alatusd.service");
            process::exit(1);
        }
    };

    if let Err(err) = client.ping().await {
        eprintln!("Error: {}", err);
        eprintln!("Remediation: Start it with: sudo systemctl start alatusd.service");
        process::exit(1);
    }

    let result = match cli.command.unwrap_or(Commands::Status {
        json: false,
        watch: false,
    }) {
        Commands::Status { json, watch } => handle_status(&client, json, watch).await,
        Commands::Watch => handle_status(&client, true, true).await,
        Commands::Monitor { interval_ms } => handle_monitor(&client, interval_ms).await,
        Commands::Mode { action } => handle_mode(&client, action).await,
        Commands::Quiet => handle_mode(&client, Some(ModeAction::Quiet)).await,
        Commands::Balanced => handle_mode(&client, Some(ModeAction::Balanced)).await,
        Commands::Perf => handle_mode(&client, Some(ModeAction::Performance)).await,
        Commands::Full => handle_mode(&client, Some(ModeAction::Full)).await,
        Commands::Fan { action } => handle_fan(&client, action).await,
        Commands::Cycle => handle_cycle(&client).await,
        Commands::ChargeLimit { percentage } => handle_charge_limit(&client, percentage).await,
        Commands::Rgb { action } => handle_rgb(&client, action).await,
        Commands::Oled { .. }
        | Commands::Display { .. }
        | Commands::Daemon { .. }
        | Commands::Session { .. }
        | Commands::Completions { .. } => unreachable!(),
    };

    if let Err(e) = result {
        eprintln!("Error executing command: {}", e);
        if matches!(e, DaemonClientError::ServiceNotRunning(_)) {
            eprintln!("Remediation: Start it with: sudo systemctl start alatusd.service");
        }
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parse_status() {
        let cli = Cli::try_parse_from(["alatus", "status"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Status {
                json: false,
                watch: false
            })
        ));

        let cli_json = Cli::try_parse_from(["alatus", "status", "--json"]).unwrap();
        assert!(matches!(
            cli_json.command,
            Some(Commands::Status {
                json: true,
                watch: false
            })
        ));

        let cli_watch = Cli::try_parse_from(["alatus", "status", "--watch"]).unwrap();
        assert!(matches!(
            cli_watch.command,
            Some(Commands::Status {
                json: false,
                watch: true
            })
        ));

        let cli_json_watch =
            Cli::try_parse_from(["alatus", "status", "--json", "--watch"]).unwrap();
        assert!(matches!(
            cli_json_watch.command,
            Some(Commands::Status {
                json: true,
                watch: true
            })
        ));
    }

    #[test]
    fn test_cli_parse_monitor() {
        let cli = Cli::try_parse_from(["alatus", "monitor"]).unwrap();
        match cli.command {
            Some(Commands::Monitor { interval_ms }) => {
                assert_eq!(interval_ms, 1000);
            }
            _ => panic!("Expected monitor command"),
        }

        let cli_custom = Cli::try_parse_from(["alatus", "monitor", "-i", "500"]).unwrap();
        match cli_custom.command {
            Some(Commands::Monitor { interval_ms }) => {
                assert_eq!(interval_ms, 500);
            }
            _ => panic!("Expected monitor command with custom interval"),
        }
    }

    #[test]
    fn test_cli_parse_no_args_defaults_to_status() {
        let cli = Cli::try_parse_from(["alatus"]).unwrap();
        assert!(cli.command.is_none());
    }

    #[test]
    fn test_cli_parse_mode_get() {
        let cli = Cli::try_parse_from(["alatus", "mode", "get"]).unwrap();
        match cli.command {
            Some(Commands::Mode { action }) => {
                assert_eq!(action, Some(ModeAction::Get));
            }
            _ => panic!("Expected Commands::Mode"),
        }
    }

    #[test]
    fn test_cli_parse_mode_cycle() {
        let cli = Cli::try_parse_from(["alatus", "mode", "cycle"]).unwrap();
        match cli.command {
            Some(Commands::Mode { action }) => {
                assert_eq!(action, Some(ModeAction::Cycle));
            }
            _ => panic!("Expected Commands::Mode Cycle"),
        }
    }

    #[test]
    fn test_cli_parse_fan_cycle() {
        let cli = Cli::try_parse_from(["alatus", "fan", "cycle"]).unwrap();
        match cli.command {
            Some(Commands::Fan { action }) => {
                assert_eq!(action, Some(FanAction::Cycle));
            }
            _ => panic!("Expected Commands::Fan Cycle"),
        }
    }

    #[test]
    fn test_cli_parse_mode_set_performance_and_high_alias() {
        let cli1 = Cli::try_parse_from(["alatus", "mode", "set", "performance"]).unwrap();
        match cli1.command {
            Some(Commands::Mode {
                action: Some(ModeAction::Set { name }),
            }) => {
                assert_eq!(name, ModeChoice::Performance);
                assert_eq!(FirmwareMode::from(name), FirmwareMode::High);
            }
            _ => panic!("Expected mode set performance"),
        }

        let cli2 = Cli::try_parse_from(["alatus", "mode", "set", "high"]).unwrap();
        match cli2.command {
            Some(Commands::Mode {
                action: Some(ModeAction::Set { name }),
            }) => {
                assert_eq!(name, ModeChoice::Performance);
            }
            _ => panic!("Expected high alias to parse as Performance"),
        }

        let cli3 = Cli::try_parse_from(["alatus", "mode", "set", "full"]).unwrap();
        match cli3.command {
            Some(Commands::Mode {
                action: Some(ModeAction::Set { name }),
            }) => {
                assert_eq!(name, ModeChoice::Full);
                assert_eq!(FirmwareMode::from(name), FirmwareMode::Full);
            }
            _ => panic!("Expected mode set full"),
        }
    }

    #[test]
    fn test_cli_parse_fan_get_and_set() {
        let cli_get = Cli::try_parse_from(["alatus", "fan", "get"]).unwrap();
        match cli_get.command {
            Some(Commands::Fan {
                action: Some(FanAction::Get),
            }) => {}
            _ => panic!("Expected fan get"),
        }

        let cli_set = Cli::try_parse_from(["alatus", "fan", "set", "quiet"]).unwrap();
        match cli_set.command {
            Some(Commands::Fan {
                action: Some(FanAction::Set { profile }),
            }) => {
                assert_eq!(profile, ModeChoice::Quiet);
                assert_eq!(FirmwareMode::from(profile), FirmwareMode::Quiet);
            }
            _ => panic!("Expected fan set quiet"),
        }
    }

    #[test]
    fn test_cli_parse_charge_limit() {
        let cli = Cli::try_parse_from(["alatus", "charge-limit", "80"]).unwrap();
        match cli.command {
            Some(Commands::ChargeLimit { percentage }) => {
                assert_eq!(percentage, 80);
            }
            _ => panic!("Expected charge-limit 80"),
        }

        let err = Cli::try_parse_from(["alatus", "charge-limit", "invalid"]);
        assert!(err.is_err());
    }

    #[test]
    fn test_cli_parse_rgb_subcommands() {
        let cli_status = Cli::try_parse_from(["alatus", "rgb", "status"]).unwrap();
        assert!(matches!(
            cli_status.command,
            Some(Commands::Rgb {
                action: RgbAction::Status
            })
        ));

        let cli_color = Cli::try_parse_from(["alatus", "rgb", "set-color", "#FF00FF"]).unwrap();
        match cli_color.command {
            Some(Commands::Rgb {
                action: RgbAction::SetColor { color },
            }) => {
                assert_eq!(color, "#FF00FF");
            }
            _ => panic!("Expected rgb set-color"),
        }

        let cli_color_alias = Cli::try_parse_from(["alatus", "rgb", "color", "ff5500"]).unwrap();
        match cli_color_alias.command {
            Some(Commands::Rgb {
                action: RgbAction::SetColor { color },
            }) => {
                assert_eq!(color, "ff5500");
            }
            _ => panic!("Expected rgb color alias"),
        }

        let cli_bright = Cli::try_parse_from(["alatus", "rgb", "set-brightness", "75"]).unwrap();
        match cli_bright.command {
            Some(Commands::Rgb {
                action: RgbAction::SetBrightness { brightness },
            }) => {
                assert_eq!(brightness, 75);
            }
            _ => panic!("Expected rgb set-brightness"),
        }

        let cli_off = Cli::try_parse_from(["alatus", "rgb", "off"]).unwrap();
        assert!(matches!(
            cli_off.command,
            Some(Commands::Rgb {
                action: RgbAction::Off
            })
        ));

        let cli_on = Cli::try_parse_from(["alatus", "rgb", "on"]).unwrap();
        assert!(matches!(
            cli_on.command,
            Some(Commands::Rgb {
                action: RgbAction::On
            })
        ));
    }

    #[test]
    fn test_parse_color() {
        assert_eq!(parse_color("#FF5500").unwrap(), (255, 85, 0));
        assert_eq!(parse_color("00AAFF").unwrap(), (0, 170, 255));
        assert_eq!(parse_color("255,85,0").unwrap(), (255, 85, 0));
        assert_eq!(parse_color("255 85 0").unwrap(), (255, 85, 0));
        assert!(parse_color("invalid").is_err());
        assert!(parse_color("300,0,0").is_err());
    }

    #[test]
    fn test_cli_parse_session() {
        let cli = Cli::try_parse_from(["alatus", "session"]).unwrap();
        match cli.command {
            Some(Commands::Session {
                action,
                sync_accent,
                no_accent,
                auto_refresh,
                no_refresh,
                oled_care,
                no_oled_care,
            }) => {
                assert!(action.is_none());
                assert!(!sync_accent);
                assert!(!no_accent);
                assert!(!auto_refresh);
                assert!(!no_refresh);
                assert!(!oled_care);
                assert!(!no_oled_care);
            }
            _ => panic!("Expected session"),
        }

        let cli_flags = Cli::try_parse_from([
            "alatus",
            "session",
            "--no-accent",
            "--no-refresh",
            "--no-oled-care",
        ])
        .unwrap();
        match cli_flags.command {
            Some(Commands::Session {
                action,
                sync_accent: _,
                no_accent,
                auto_refresh: _,
                no_refresh,
                oled_care: _,
                no_oled_care,
            }) => {
                assert!(action.is_none());
                assert!(no_accent);
                assert!(no_refresh);
                assert!(no_oled_care);
            }
            _ => panic!("Expected session with flags"),
        }

        let cli_status = Cli::try_parse_from(["alatus", "session", "status"]).unwrap();
        match cli_status.command {
            Some(Commands::Session {
                action: Some(SessionAction::Status),
                ..
            }) => {}
            _ => panic!("Expected session status"),
        }

        let cli_set = Cli::try_parse_from([
            "alatus",
            "session",
            "set",
            "--accent",
            "off",
            "--refresh",
            "on",
            "--oled-care",
            "on",
        ])
        .unwrap();
        match cli_set.command {
            Some(Commands::Session {
                action:
                    Some(SessionAction::Set {
                        accent,
                        refresh,
                        oled_care,
                    }),
                ..
            }) => {
                assert_eq!(accent, Some(false));
                assert_eq!(refresh, Some(true));
                assert_eq!(oled_care, Some(true));
            }
            _ => panic!("Expected session set"),
        }

        let cli_refresh = Cli::try_parse_from(["alatus", "session", "pixel-refresh"]).unwrap();
        match cli_refresh.command {
            Some(Commands::Session {
                action: Some(SessionAction::PixelRefresh),
                ..
            }) => {}
            _ => panic!("Expected session pixel-refresh"),
        }

        let cli_refresh_alias =
            Cli::try_parse_from(["alatus", "session", "refresh-pixels"]).unwrap();
        match cli_refresh_alias.command {
            Some(Commands::Session {
                action: Some(SessionAction::PixelRefresh),
                ..
            }) => {}
            _ => panic!("Expected session refresh-pixels alias"),
        }
    }

    #[test]
    fn test_cli_parse_mode_auto() {
        let cli_auto = Cli::try_parse_from(["alatus", "mode", "auto"]).unwrap();
        match cli_auto.command {
            Some(Commands::Mode {
                action: Some(ModeAction::Auto { enable }),
            }) => {
                assert!(enable.is_none());
            }
            _ => panic!("Expected mode auto"),
        }

        let cli_auto_on = Cli::try_parse_from(["alatus", "mode", "auto", "on"]).unwrap();
        match cli_auto_on.command {
            Some(Commands::Mode {
                action: Some(ModeAction::Auto { enable }),
            }) => {
                assert_eq!(enable, Some(true));
            }
            _ => panic!("Expected mode auto on"),
        }
    }

    #[test]
    fn test_mode_format_and_mapping() {
        assert_eq!(format_mode(FirmwareMode::Balanced), "Balanced");
        assert_eq!(format_mode(FirmwareMode::Quiet), "Quiet");
        assert_eq!(format_mode(FirmwareMode::High), "Performance");
        assert_eq!(format_mode(FirmwareMode::Full), "Full");
        assert_eq!(format_mode(FirmwareMode::Unknown(42)), "Unknown");
    }

    #[test]
    fn test_cli_parse_oled() {
        let cli_oled = Cli::try_parse_from(["alatus", "oled"]).unwrap();
        assert!(matches!(
            cli_oled.command,
            Some(Commands::Oled { action: None })
        ));

        let cli_status = Cli::try_parse_from(["alatus", "oled", "status"]).unwrap();
        assert!(matches!(
            cli_status.command,
            Some(Commands::Oled {
                action: Some(OledAction::Status)
            })
        ));

        let cli_dim_on = Cli::try_parse_from(["alatus", "oled", "dim", "on"]).unwrap();
        match cli_dim_on.command {
            Some(Commands::Oled {
                action: Some(OledAction::Dim { state, dim, level }),
            }) => {
                assert_eq!(state, Some(true));
                assert_eq!(dim, None);
                assert_eq!(level, None);
            }
            _ => panic!("Expected oled dim on"),
        }

        let cli_dim_off = Cli::try_parse_from(["alatus", "oled", "dim", "off"]).unwrap();
        match cli_dim_off.command {
            Some(Commands::Oled {
                action: Some(OledAction::Dim { state, dim, level }),
            }) => {
                assert_eq!(state, Some(false));
                assert_eq!(dim, None);
                assert_eq!(level, None);
            }
            _ => panic!("Expected oled dim off"),
        }

        let cli_dim_level =
            Cli::try_parse_from(["alatus", "oled", "dim", "--level", "60"]).unwrap();
        match cli_dim_level.command {
            Some(Commands::Oled {
                action: Some(OledAction::Dim { state, dim, level }),
            }) => {
                assert_eq!(state, None);
                assert_eq!(dim, None);
                assert_eq!(level, Some(60));
            }
            _ => panic!("Expected oled dim --level 60"),
        }

        let cli_disp_dim = Cli::try_parse_from(["alatus", "display", "dim", "75"]).unwrap();
        match cli_disp_dim.command {
            Some(Commands::Display {
                action: Some(DisplayAction::Dim { level }),
            }) => {
                assert_eq!(level, 75);
            }
            _ => panic!("Expected display dim 75"),
        }

        let cli_refresh = Cli::try_parse_from(["alatus", "oled", "refresh"]).unwrap();
        assert!(matches!(
            cli_refresh.command,
            Some(Commands::Oled {
                action: Some(OledAction::Refresh)
            })
        ));

        let cli_clean = Cli::try_parse_from(["alatus", "oled", "clean"]).unwrap();
        assert!(matches!(
            cli_clean.command,
            Some(Commands::Oled {
                action: Some(OledAction::Refresh)
            })
        ));
    }

    #[test]
    fn test_cli_parse_display() {
        let cli_disp = Cli::try_parse_from(["alatus", "display"]).unwrap();
        assert!(matches!(
            cli_disp.command,
            Some(Commands::Display { action: None })
        ));

        let cli_status = Cli::try_parse_from(["alatus", "display", "status"]).unwrap();
        assert!(matches!(
            cli_status.command,
            Some(Commands::Display {
                action: Some(DisplayAction::Status)
            })
        ));

        let cli_rate_num = Cli::try_parse_from(["alatus", "display", "rate", "120"]).unwrap();
        match cli_rate_num.command {
            Some(Commands::Display {
                action: Some(DisplayAction::Rate { target, enable }),
            }) => {
                assert_eq!(target, "120");
                assert_eq!(enable, None);
            }
            _ => panic!("Expected display rate 120"),
        }

        let cli_rate_auto = Cli::try_parse_from(["alatus", "display", "rate", "auto"]).unwrap();
        match cli_rate_auto.command {
            Some(Commands::Display {
                action: Some(DisplayAction::Rate { target, enable }),
            }) => {
                assert_eq!(target, "auto");
                assert_eq!(enable, None);
            }
            _ => panic!("Expected display rate auto"),
        }

        let cli_rate_auto_off =
            Cli::try_parse_from(["alatus", "display", "rate", "auto", "off"]).unwrap();
        match cli_rate_auto_off.command {
            Some(Commands::Display {
                action: Some(DisplayAction::Rate { target, enable }),
            }) => {
                assert_eq!(target, "auto");
                assert_eq!(enable, Some(false));
            }
            _ => panic!("Expected display rate auto off"),
        }
    }

    #[test]
    fn test_cli_parse_daemon() {
        let cli_sess_start = Cli::try_parse_from(["alatus", "daemon", "session", "start"]).unwrap();
        assert!(matches!(
            cli_sess_start.command,
            Some(Commands::Daemon {
                target: DaemonTarget::Session {
                    action: Some(DaemonServiceAction::Start)
                }
            })
        ));

        let cli_sess_status =
            Cli::try_parse_from(["alatus", "daemon", "session", "status"]).unwrap();
        assert!(matches!(
            cli_sess_status.command,
            Some(Commands::Daemon {
                target: DaemonTarget::Session {
                    action: Some(DaemonServiceAction::Status)
                }
            })
        ));

        let cli_sys_restart =
            Cli::try_parse_from(["alatus", "daemon", "system", "restart"]).unwrap();
        assert!(matches!(
            cli_sys_restart.command,
            Some(Commands::Daemon {
                target: DaemonTarget::System {
                    action: Some(DaemonServiceAction::Restart)
                }
            })
        ));

        let cli_daemon_status = Cli::try_parse_from(["alatus", "daemon", "status"]).unwrap();
        assert!(matches!(
            cli_daemon_status.command,
            Some(Commands::Daemon {
                target: DaemonTarget::Status
            })
        ));
    }

    #[test]
    fn test_cli_parse_rgb_sync() {
        let cli_sync_on = Cli::try_parse_from(["alatus", "rgb", "sync", "on"]).unwrap();
        assert!(matches!(
            cli_sync_on.command,
            Some(Commands::Rgb {
                action: RgbAction::Sync { enable: Some(true) }
            })
        ));

        let cli_sync_off = Cli::try_parse_from(["alatus", "rgb", "sync", "off"]).unwrap();
        assert!(matches!(
            cli_sync_off.command,
            Some(Commands::Rgb {
                action: RgbAction::Sync {
                    enable: Some(false)
                }
            })
        ));
    }

    #[test]
    fn test_cli_parse_shortcuts_and_aliases() {
        // Direct mode switches
        let cli_bal = Cli::try_parse_from(["alatus", "mode", "balanced"]).unwrap();
        assert!(matches!(
            cli_bal.command,
            Some(Commands::Mode {
                action: Some(ModeAction::Balanced)
            })
        ));

        let cli_bal_alias = Cli::try_parse_from(["alatus", "mode", "bal"]).unwrap();
        assert!(matches!(
            cli_bal_alias.command,
            Some(Commands::Mode {
                action: Some(ModeAction::Balanced)
            })
        ));

        let cli_quiet = Cli::try_parse_from(["alatus", "mode", "quiet"]).unwrap();
        assert!(matches!(
            cli_quiet.command,
            Some(Commands::Mode {
                action: Some(ModeAction::Quiet)
            })
        ));

        let cli_perf = Cli::try_parse_from(["alatus", "mode", "perf"]).unwrap();
        assert!(matches!(
            cli_perf.command,
            Some(Commands::Mode {
                action: Some(ModeAction::Performance)
            })
        ));

        let cli_full = Cli::try_parse_from(["alatus", "mode", "full"]).unwrap();
        assert!(matches!(
            cli_full.command,
            Some(Commands::Mode {
                action: Some(ModeAction::Full)
            })
        ));

        // Direct fan switches
        let cli_fan_bal = Cli::try_parse_from(["alatus", "fan", "bal"]).unwrap();
        assert!(matches!(
            cli_fan_bal.command,
            Some(Commands::Fan {
                action: Some(FanAction::Balanced)
            })
        ));

        let cli_fan_perf = Cli::try_parse_from(["alatus", "fan", "perf"]).unwrap();
        assert!(matches!(
            cli_fan_perf.command,
            Some(Commands::Fan {
                action: Some(FanAction::Performance)
            })
        ));

        // Top level cycle & watch
        let cli_cycle = Cli::try_parse_from(["alatus", "cycle"]).unwrap();
        assert!(matches!(cli_cycle.command, Some(Commands::Cycle)));

        let cli_watch = Cli::try_parse_from(["alatus", "watch"]).unwrap();
        assert!(matches!(cli_watch.command, Some(Commands::Watch)));

        let cli_status_w = Cli::try_parse_from(["alatus", "status", "-w"]).unwrap();
        assert!(matches!(
            cli_status_w.command,
            Some(Commands::Status {
                json: false,
                watch: true
            })
        ));

        let cli_status_jw = Cli::try_parse_from(["alatus", "status", "-j", "-w"]).unwrap();
        assert!(matches!(
            cli_status_jw.command,
            Some(Commands::Status {
                json: true,
                watch: true
            })
        ));
    }
}
