// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Real hardware drivers interfacing with Linux kernel sysfs, debugfs, and HID interfaces.

pub mod battery;
pub mod display;
pub mod rgb;
pub mod thermal;

pub use battery::SysfsBatteryDriver;
pub use display::OledDisplayDriver;
pub use rgb::Ite5570Driver;
pub use thermal::AsusWmiDriver;
