// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Backward-compatibility bridge for `crate::services::session`.
//!
//! All session management, desktop integration, OLED care, and display handling
//! have moved to [`crate::services::session`].

pub use crate::services::session::*;
