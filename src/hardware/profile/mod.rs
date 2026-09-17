// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Declarative device hardware profiles and DMI matching infrastructure.

pub mod matcher;
pub mod model;

pub use matcher::{DmiMatcher, MatcherError};
pub use model::{
    BatteryProfileConfig, DeviceMeta, DeviceProfile, DeviceProfileCapabilities,
    DisplayProfileConfig, RgbProfileConfig, ThermalProfileConfig,
};
