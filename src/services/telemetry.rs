// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Bridge module re-exporting telemetry collectors and formatters.
//!
//! Note: Telemetry logic has been relocated to `crate::telemetry`. This module
//! is maintained for backwards compatibility during subsystem reorganization.

pub use crate::telemetry::collectors::*;
pub use crate::telemetry::formatters::*;
