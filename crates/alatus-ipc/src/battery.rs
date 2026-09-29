//! D-Bus battery interface and proxy definitions.

use alatus_core::battery::{BatteryInfo, BatteryStatus};
use serde::{Deserialize, Serialize};
use zvariant::Type;

pub const BATTERY_INTERFACE: &str = "org.alatus.Battery";
pub const BATTERY_OBJECT_PATH: &str = "/org/alatus/Battery";

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct BatteryInfoMsg {
    pub percentage: u8,
    pub status: String,
    pub charge_limit: Option<u8>,
    pub health_percentage: Option<u8>,
    pub power_now_microwatts: Option<u64>,
}

impl From<BatteryInfo> for BatteryInfoMsg {
    fn from(info: BatteryInfo) -> Self {
        let status_str = match info.status {
            BatteryStatus::Charging => "Charging",
            BatteryStatus::Discharging => "Discharging",
            BatteryStatus::NotCharging => "Not charging",
            BatteryStatus::Full => "Full",
            BatteryStatus::Unknown => "Unknown",
        };

        Self {
            percentage: info.percentage,
            status: status_str.to_string(),
            charge_limit: info.charge_limit,
            health_percentage: info.health_percentage,
            power_now_microwatts: info.power_now_microwatts,
        }
    }
}

#[zbus::proxy(
    interface = "org.alatus.Battery",
    default_service = "org.alatus.Daemon",
    default_path = "/org/alatus/Battery"
)]
pub trait Battery {
    async fn get_info(&self) -> zbus::Result<BatteryInfoMsg>;
    async fn get_charge_limit(&self) -> zbus::Result<u8>;
    async fn set_charge_limit(&self, limit: u8) -> zbus::Result<()>;
    async fn get_health_percentage(&self) -> zbus::Result<u8>;
    async fn is_charging(&self) -> zbus::Result<bool>;

    #[zbus(signal)]
    async fn limit_changed(&self, new_limit: u8) -> zbus::Result<()>;
}
