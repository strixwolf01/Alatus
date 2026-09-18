// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

pub mod asus_wmi;
pub mod rog_wmi;

pub use asus_wmi::AsusWmiDriver;
pub use rog_wmi::{FanCurve, FanCurvePoint, RogWmiThermalDriver};
