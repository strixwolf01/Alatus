//! D-Bus client proxy and message definitions for org.alatus.System.

use serde::{Deserialize, Serialize};
use zbus::proxy;
use zbus::zvariant::Type;

pub const SYSTEM_INTERFACE: &str = "org.alatus.System";
pub const SYSTEM_SYSTEM_OBJECT_PATH: &str = "/org/alatus/System";

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct SystemInfoMsg {
    pub serial_number: String,
    pub product_name: String,
    pub board_name: String,
    pub bios_version: String,
}

#[proxy(
    interface = "org.alatus.System",
    default_service = "org.alatus.Daemon",
    default_path = "/org/alatus/System"
)]
pub trait System {
    async fn get_serial_number(&self) -> zbus::Result<String>;
    async fn get_system_info(&self) -> zbus::Result<SystemInfoMsg>;
}
