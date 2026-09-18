// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Real hardware drivers interfacing with Linux kernel sysfs, debugfs, and HID interfaces.

pub mod battery;
pub mod display;
pub mod platform;
pub mod proxy;
pub mod rgb;
pub mod thermal;

pub use battery::SysfsBatteryDriver;
pub use display::OledDisplayDriver;
pub use platform::{ArmouryAttribute, ArmouryPlatformDriver};
pub use proxy::AsusctlProxyDriver;
pub use rgb::{AuraHidDriver, Ite5570Driver, TufSysfsRgbDriver};
pub use thermal::{AsusWmiDriver, FanCurve, FanCurvePoint, RogWmiThermalDriver};
