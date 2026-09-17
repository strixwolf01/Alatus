// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::services::daemon_client::{DaemonClient, DaemonClientError};
use std::process;

pub async fn handle_charge_limit(
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
