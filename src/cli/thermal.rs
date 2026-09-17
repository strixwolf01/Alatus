// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::services::daemon_client::{DaemonClient, DaemonClientError};
use crate::services::firmware_mode::FirmwareMode;
use clap::{Subcommand, ValueEnum};

#[derive(Subcommand, Debug, PartialEq, Eq, Clone)]
pub enum ModeAction {
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
        #[arg(value_parser = super::session_cmd::parse_bool)]
        enable: Option<bool>,
    },
}

#[derive(Subcommand, Debug, PartialEq, Eq, Clone)]
pub enum FanAction {
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

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeChoice {
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

pub fn format_mode(mode: FirmwareMode) -> &'static str {
    match mode {
        FirmwareMode::Balanced => "Balanced",
        FirmwareMode::Quiet => "Quiet",
        FirmwareMode::High => "Performance",
        FirmwareMode::Full => "Full",
        FirmwareMode::Unknown(_) => "Unknown",
    }
}

pub async fn handle_mode(
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

pub async fn handle_fan(
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

pub async fn handle_cycle(client: &DaemonClient) -> Result<(), DaemonClientError> {
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
