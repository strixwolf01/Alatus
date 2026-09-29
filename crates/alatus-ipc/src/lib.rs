//! D-Bus interfaces and IPC protocols for Alatus.

pub mod battery;

pub use battery::{BatteryInfoMsg, BatteryProxy, BATTERY_INTERFACE, BATTERY_OBJECT_PATH};

pub const SYSTEM_BUS_NAME: &str = "org.alatus.Daemon";
pub const SYSTEM_OBJECT_PATH: &str = "/org/alatus/Daemon";
