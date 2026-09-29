pub mod battery;
pub mod capabilities;
pub mod error;
pub mod hotkey;
pub mod lighting;
pub mod thermal;

pub use battery::{BatteryDriver, BatteryInfo, BatteryStatus};
pub use capabilities::{BatteryCapabilities, LightingCapabilities, ThermalCapabilities};
pub use error::AlatusError;
pub use hotkey::{HotkeyAction, HotkeyDriver, HotkeyEvent};
pub use lighting::{LightingDriver, LightingEffect, LightingMode, RgbColor};
pub use thermal::{FanStatus, ThermalDriver, ThermalProfileMode};
