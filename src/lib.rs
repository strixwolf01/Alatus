// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

pub mod cli;
pub mod daemon;
pub mod domain;
#[cfg(feature = "gui")]
pub mod gui;
pub mod hardware;
pub mod services;
pub mod telemetry;
