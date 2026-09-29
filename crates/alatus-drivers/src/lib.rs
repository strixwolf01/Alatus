//! Concrete hardware drivers for ASUS laptops.

pub mod battery;
pub mod hotkey;
pub mod lighting;
pub mod sysfs;
pub mod thermal;

pub use battery::AsusSysfsBatteryDriver;
pub use hotkey::AsusWmiHotkeyDriver;
pub use lighting::AsusIte5570LightingDriver;
pub use sysfs::SysfsRoot;
pub use thermal::AsusHybridThermalDriver;
