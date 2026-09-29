//! D-Bus lighting interface and proxy definitions.

use serde::{Deserialize, Serialize};
use zvariant::Type;

pub const LIGHTING_INTERFACE: &str = "org.alatus.Lighting";
pub const LIGHTING_OBJECT_PATH: &str = "/org/alatus/Lighting";

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct LightingStateMsg {
    pub mode: String,
    pub brightness: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub speed: u8,
}

#[zbus::proxy(
    interface = "org.alatus.Lighting",
    default_service = "org.alatus.Daemon",
    default_path = "/org/alatus/Lighting"
)]
pub trait Lighting {
    async fn get_brightness(&self) -> zbus::Result<u8>;
    async fn set_brightness(&self, level: u8) -> zbus::Result<()>;
    async fn set_color(&self, r: u8, g: u8, b: u8) -> zbus::Result<()>;
    async fn set_mode(&self, mode: String, speed: u8) -> zbus::Result<()>;
    async fn get_state(&self) -> zbus::Result<LightingStateMsg>;

    #[zbus(signal)]
    async fn state_changed(&self, state: LightingStateMsg) -> zbus::Result<()>;
}
