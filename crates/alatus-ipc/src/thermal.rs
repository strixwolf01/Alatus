//! D-Bus thermal interface and proxy definitions.

use alatus_core::thermal::FanStatus;
use serde::{Deserialize, Serialize};
use zvariant::Type;

pub const THERMAL_INTERFACE: &str = "org.alatus.Thermal";
pub const THERMAL_OBJECT_PATH: &str = "/org/alatus/Thermal";

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct FanStatusMsg {
    pub label: String,
    pub current_rpm: u32,
    pub max_rpm: Option<u32>,
}

impl From<FanStatus> for FanStatusMsg {
    fn from(f: FanStatus) -> Self {
        Self {
            label: f.label,
            current_rpm: f.current_rpm,
            max_rpm: f.max_rpm,
        }
    }
}

#[zbus::proxy(
    interface = "org.alatus.Thermal",
    default_service = "org.alatus.Daemon",
    default_path = "/org/alatus/Thermal"
)]
pub trait Thermal {
    async fn get_current_profile(&self) -> zbus::Result<String>;
    async fn set_profile(&self, mode: String) -> zbus::Result<()>;
    async fn list_profiles(&self) -> zbus::Result<Vec<String>>;
    async fn get_fans(&self) -> zbus::Result<Vec<FanStatusMsg>>;
    async fn is_cpu_only(&self) -> zbus::Result<bool>;

    #[zbus(signal)]
    async fn profile_changed(&self, new_profile: String) -> zbus::Result<()>;
}
