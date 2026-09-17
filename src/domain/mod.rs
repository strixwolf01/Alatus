// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Domain models and self-validating types enforcing architectural boundaries.

pub mod battery;
pub mod error;
pub mod rgb;
pub mod thermal;

pub use battery::ChargeThreshold;
pub use error::DomainError;
pub use rgb::{BrightnessPercent, ColorRgb, RgbTimeoutPolicy, TimeoutDuration};
pub use thermal::ThermalMode;
