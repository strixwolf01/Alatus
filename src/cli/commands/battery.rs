// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Battery charge threshold control command.

use crate::client::AlatusClient;
use crate::services::daemon_client::DaemonClientError;
use serde_json::json;
use std::process;

pub async fn handle_charge_limit(
    client: &AlatusClient,
    percentage: u32,
    json: bool,
) -> Result<(), DaemonClientError> {
    if percentage > 100 {
        if json {
            println!(
                "{}",
                json!({
                    "success": false,
                    "error": format!("Charge limit must be between 0 and 100 (got: {}%)", percentage)
                })
            );
        } else {
            eprintln!(
                "Error: Charge limit must be between 0 and 100 (got: {}%)",
                percentage
            );
        }
        process::exit(1);
    }

    client.set_charge_limit(percentage).await?;

    if json {
        println!(
            "{}",
            json!({
                "success": true,
                "charge_limit": percentage
            })
        );
    } else {
        println!("Battery charge limit set to {}%", percentage);
    }
    Ok(())
}
