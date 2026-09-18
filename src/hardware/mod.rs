// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Pure hardware driver abstractions and platform capability models.

pub mod capabilities;
pub mod context;
pub mod drivers;
pub mod error;
pub mod mock;
pub mod profile;
pub mod traits;

pub use capabilities::{
    BatteryCapabilityDetails, CapabilityState, DisplayCapabilityDetails, RgbCapabilityDetails,
    SystemCapabilities, ThermalCapabilityDetails, UnavailableReason,
};
pub use context::DeviceContext;
pub use drivers::{
    AsusWmiDriver, AsusctlProxyDriver, AuraHidDriver, FanCurve, FanCurvePoint, Ite5570Driver,
    OledDisplayDriver, RogWmiThermalDriver, SysfsBatteryDriver, TufSysfsRgbDriver,
};
pub use error::DriverError;
pub use mock::{MockBatteryDriver, MockDisplayDriver, MockRgbDriver, MockThermalDriver};
pub use profile::{DeviceProfile, DmiMatcher, MatcherError};
pub use traits::{BatteryDriver, DisplayDriver, RgbDriver, ThermalDriver};
