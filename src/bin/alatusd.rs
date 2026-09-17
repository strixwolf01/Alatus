// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    alatus::daemon::run_daemon().await
}
