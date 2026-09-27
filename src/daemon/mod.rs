// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Backward-compatibility bridge for `crate::services::daemon`.
//!
//! Privileged daemon operations, D-Bus interfaces, and policy engine
//! have moved to [`crate::services::daemon`].

pub use crate::services::daemon::*;
pub use crate::services::daemon::dbus as dbus_interface;
