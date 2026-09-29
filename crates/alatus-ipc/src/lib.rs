//! D-Bus interfaces and IPC protocols for Alatus.

pub mod battery;
pub mod lighting;
pub mod thermal;

pub use battery::{BatteryInfoMsg, BatteryProxy, BATTERY_INTERFACE, BATTERY_OBJECT_PATH};
pub use lighting::{LightingProxy, LightingStateMsg, LIGHTING_INTERFACE, LIGHTING_OBJECT_PATH};
pub use thermal::{FanStatusMsg, ThermalProxy, THERMAL_INTERFACE, THERMAL_OBJECT_PATH};

pub const SYSTEM_BUS_NAME: &str = "org.alatus.Daemon";
pub const SYSTEM_OBJECT_PATH: &str = "/org/alatus/Daemon";
