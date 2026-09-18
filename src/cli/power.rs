// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! CLI subcommands for platform power limits (PPT / Dynamic Boost) tuning.

use crate::services::daemon_client::{DaemonClientError, get_daemon_client};
use clap::Subcommand;

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum PowerAction {
    /// Platform power limits (PPT) and Dynamic Boost controls
    Ppt {
        #[command(subcommand)]
        action: Option<PptAction>,

        /// List available PPT attributes, current values, and hardware limits
        #[arg(long, default_value_t = false)]
        list: bool,
    },
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum PptAction {
    /// List available PPT attributes, current values, and hardware limits
    List,
    /// Set a platform power limit attribute (in Watts)
    Set {
        /// Attribute name (e.g. ppt_pl1_spl, ppt_pl2_sppt, ppt_fppt, nv_dynamic_boost)
        attribute: String,
        /// Target limit in Watts
        value: u32,
    },
}

pub async fn handle_power(action: PowerAction) -> Result<(), DaemonClientError> {
    let client = get_daemon_client().await?;

    match action {
        PowerAction::Ppt { action, list } => {
            match action {
                Some(PptAction::Set { attribute, value }) => {
                    client.set_ppt_limit(&attribute, value).await?;
                    match client.get_ppt_attribute(&attribute).await {
                        Ok(attr) => {
                            println!(
                                "✓ Set PPT limit for '{attribute}' to {}W (requested: {value}W, bounds: {}W..={}W, default: {}W)",
                                attr.current_value, attr.min_value, attr.max_value, attr.default_value
                            );
                        }
                        Err(_) => {
                            println!("✓ Successfully committed PPT limit for '{attribute}' to {value}W");
                        }
                    }
                }
                Some(PptAction::List) => {
                    display_ppt_attributes(&client).await?;
                }
                None => {
                    if list {
                        display_ppt_attributes(&client).await?;
                    } else {
                        // Default to displaying attributes when neither subcommand nor flag is given
                        display_ppt_attributes(&client).await?;
                    }
                }
            }
        }
    }

    Ok(())
}

async fn display_ppt_attributes(
    client: &crate::services::daemon_client::DaemonClient,
) -> Result<(), DaemonClientError> {
    let attrs = client.list_ppt_attributes().await?;
    if attrs.is_empty() {
        println!("No platform power limit (PPT) attributes detected on this device.");
        return Ok(());
    }

    println!("ASUS Platform Power Limits (PPT)");
    println!("────────────────────────────────────────────────────────────────────────");
    println!(
        "{:<20} {:<10} {:<20} {:<10} {:<10}",
        "Attribute", "Current", "Bounds (Min..=Max)", "Default", "Step"
    );
    println!("────────────────────────────────────────────────────────────────────────");

    for name in &attrs {
        if let Ok(attr) = client.get_ppt_attribute(name).await {
            let bounds_str = format!("{}W ..= {}W", attr.min_value, attr.max_value);
            println!(
                "{:<20} {:<10} {:<20} {:<10} {:<10}",
                attr.name,
                format!("{}W", attr.current_value),
                bounds_str,
                format!("{}W", attr.default_value),
                format!("{}W", attr.scalar_increment)
            );
        } else {
            println!("{:<20} {:<10}", name, "Unavailable");
        }
    }

    Ok(())
}

