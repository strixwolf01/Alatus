// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::services::daemon_client::{DaemonClient, DaemonClientError};
use clap::Subcommand;

#[derive(Subcommand, Debug, Clone)]
pub enum RgbAction {
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

    /// Wake keyboard RGB backlight from inactivity sleep immediately
    Wake,

    /// Toggle desktop accent color synchronization with keyboard backlighting
    Sync {
        /// Enable or disable accent sync (on/off, true/false)
        #[arg(value_parser = super::session_cmd::parse_bool)]
        enable: Option<bool>,
    },

    /// Configure keyboard RGB inactivity timeout policy and delay duration
    #[command(alias = "idle")]
    Timeout {
        /// Optional timeout delay in seconds (shortcut for delay)
        #[arg(value_parser = clap::value_parser!(u32))]
        seconds: Option<u32>,

        /// Inactivity timeout policy (never, battery, always)
        #[arg(long, short = 'p')]
        policy: Option<String>,

        /// Inactivity timeout delay in seconds
        #[arg(long, short = 'd', value_parser = clap::value_parser!(u32))]
        delay: Option<u32>,

        #[command(subcommand)]
        subcommand: Option<RgbTimeoutSubcommand>,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum RgbTimeoutSubcommand {
    /// Set timeout policy (never, battery, always)
    Policy {
        /// Policy value: never, battery (or battery_only), always
        policy: String,
    },
    /// Set timeout delay duration in seconds
    Delay {
        /// Duration in seconds (e.g. 10, 30, 60, 120, 300)
        #[arg(value_parser = clap::value_parser!(u32))]
        seconds: u32,
    },
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

pub async fn handle_rgb(client: &DaemonClient, action: RgbAction) -> Result<(), DaemonClientError> {
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
            if let Ok(policy) = client.get_rgb_timeout_policy().await {
                println!("Timeout Policy: {policy}");
            }
            if let Ok(timeout) = client.get_rgb_timeout().await {
                if timeout == 0 {
                    println!("Timeout Delay:  Disabled (always on)");
                } else {
                    println!("Timeout Delay:  {}s", timeout);
                }
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
            let mut config = crate::services::config::load_config();
            config.rgb_brightness = brightness;
            let _ = crate::services::config::save_config_atomic(&config);
            println!("RGB brightness set to {}%", brightness);
        }
        RgbAction::Off => {
            client.set_rgb_brightness(0).await?;
            let mut config = crate::services::config::load_config();
            config.rgb_brightness = 0;
            let _ = crate::services::config::save_config_atomic(&config);
            println!("Keyboard RGB turned off");
        }
        RgbAction::On => {
            client.set_rgb_brightness(100).await?;
            let mut config = crate::services::config::load_config();
            config.rgb_brightness = 100;
            let _ = crate::services::config::save_config_atomic(&config);
            println!("Keyboard RGB turned on");
        }
        RgbAction::Wake => {
            client.wake_rgb().await?;
            println!("Keyboard RGB backlight awakened");
        }
        RgbAction::Sync { enable } => {
            let on = enable.unwrap_or(true);
            crate::services::desktop_session::set_session_accent_sync(on)
                .await
                .map_err(|e| {
                    DaemonClientError::IoError(format!("Failed to set accent sync: {e}"))
                })?;
            println!(
                "Desktop Accent Color Sync set to: {}",
                if on { "enabled" } else { "disabled" }
            );
        }
        RgbAction::Timeout {
            seconds,
            policy,
            delay,
            subcommand,
        } => {
            let mut target_policy: Option<String> = policy;
            let mut target_delay: Option<u32> = delay.or(seconds);

            if let Some(sub) = subcommand {
                match sub {
                    RgbTimeoutSubcommand::Policy { policy: p } => {
                        target_policy = Some(p);
                    }
                    RgbTimeoutSubcommand::Delay { seconds: s } => {
                        target_delay = Some(s);
                    }
                }
            }

            if target_policy.is_none() && target_delay.is_none() {
                let current_policy = client
                    .get_rgb_timeout_policy()
                    .await
                    .unwrap_or_else(|_| "unknown".to_string());
                let current_delay = client.get_rgb_timeout().await.unwrap_or(60);
                println!("Keyboard RGB Inactivity Timeout");
                println!("───────────────────────────────");
                println!("Policy:   {current_policy}");
                println!("Delay:    {current_delay}s");
                return Ok(());
            }

            let mut config = crate::services::config::load_config();

            if let Some(p_str) = target_policy {
                let p = p_str
                    .parse::<crate::services::config::RgbTimeoutPolicy>()
                    .map_err(DaemonClientError::InvalidArgument)?;
                client.set_rgb_timeout_policy(&p.to_string()).await?;
                config.rgb_timeout_policy = p;
                println!("Keyboard RGB timeout policy set to '{p}'");
            }

            if let Some(d) = target_delay {
                client.set_rgb_timeout(d).await?;
                config.rgb_timeout_seconds = d;
                if d == 0 {
                    println!("Keyboard RGB inactivity delay set to 0 (disabled)");
                } else {
                    println!("Keyboard RGB inactivity delay set to {d} seconds");
                }
            }

            let _ = crate::services::config::save_config_atomic(&config);
        }
    }
    Ok(())
}
