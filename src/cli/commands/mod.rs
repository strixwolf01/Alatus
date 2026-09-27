// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Modular CLI command handlers.

pub mod battery;
pub mod daemon;
pub mod display;
pub mod monitor;
pub mod profile;
pub mod rgb;
pub mod session;
pub mod status;

pub use battery::*;
pub use daemon::*;
pub use display::*;
pub use monitor::*;
pub use profile::*;
pub use rgb::*;
pub use session::*;
pub use status::*;
