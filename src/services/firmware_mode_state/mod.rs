// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! State/synchronisation layer for the "Fan Profile" UI.
//!
//! Sits between the raw backend ([`crate::services::firmware_mode`]) and the
//! future UI page. It owns the *policy* for when the requested mode may be
//! written back after something else (asusd / the kernel / the user) has silently changed the EC mode.

pub mod events;
pub mod reconcile;
pub mod types;

pub use events::*;
pub use reconcile::*;
pub use types::*;

#[cfg(test)]
mod tests;
