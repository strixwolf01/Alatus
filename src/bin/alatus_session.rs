// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use alatus::services::desktop_session::{DesktopSessionOptions, run_desktop_session};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting alatus-session desktop agent");

    run_desktop_session(DesktopSessionOptions {
        sync_accent: true,
        auto_refresh: true,
        oled_care: true,
    })
    .await?;

    Ok(())
}
