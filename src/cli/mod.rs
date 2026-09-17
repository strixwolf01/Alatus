// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Standalone native command-line interface for Alatus.
//!
//! Provides CLI controls for ASUS Linux hardware management including power/fan profiles,
//! battery charge limiting, deep sleep states, keyboard RGB lighting, and status diagnostics via D-Bus.

pub mod battery;
pub mod rgb;
pub mod session_cmd;
pub mod status;
pub mod thermal;

pub use battery::*;
pub use rgb::*;
pub use session_cmd::*;
pub use status::*;
pub use thermal::*;

use crate::services::daemon_client::{DaemonClient, DaemonClientError};
use clap::{Parser, Subcommand};
use std::process;

#[derive(Parser, Debug)]
#[command(name = "alatus")]
#[command(author = "Alatus Contributors")]
#[command(version)]
#[command(about = "ASUS Linux hardware control CLI and GUI", long_about = None)]
pub struct Cli {
    /// Launch the Slint graphical user interface
    #[arg(long, default_value_t = false)]
    pub gui: bool,

    /// Start GUI minimized to system tray
    #[arg(long, default_value_t = false)]
    pub tray: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Launch the Slint graphical user interface dashboard
    Gui {
        /// Start GUI minimized to system tray
        #[arg(short = 'm', long, visible_alias = "tray", default_value_t = false)]
        minimized: bool,
    },

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

pub async fn run() {
    let cli = Cli::parse();

    // Check for GUI launch via subcommand 'gui' or flag '--gui' or '--tray'
    if cli.gui || cli.tray || matches!(cli.command, Some(Commands::Gui { .. })) {
        #[cfg(feature = "gui")]
        {
            let minimized = cli.tray
                || match &cli.command {
                    Some(Commands::Gui { minimized, .. }) => *minimized,
                    _ => false,
                };
            if let Err(e) = crate::gui::run_gui(minimized).await {
                eprintln!("Error executing GUI: {e}");
                process::exit(1);
            }
            return;
        }
        #[cfg(not(feature = "gui"))]
        {
            eprintln!("Error: GUI feature was not enabled at compile time.");
            process::exit(1);
        }
    }

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
            if let Err(e) = crate::services::desktop_session::set_session_accent_sync(on).await {
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
        Commands::Gui { .. }
        | Commands::Oled { .. }
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
mod tests;
