// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Platform hardware resolution, DMI discovery, and driver instantiation factory.

pub mod device;
pub mod factory;
pub mod sysfs_probe;

pub use device::{builtin_profiles, fallback_profile, resolve_profile_from_dmi};
pub use factory::DriverFactory;
pub use sysfs_probe::*;
