// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Event listener streams for desktop sessions.

pub mod portal;
pub mod power;

pub use portal::create_portal_stream;
pub use power::{create_power_stream, create_screensaver_streams, create_thermal_osd_stream};
