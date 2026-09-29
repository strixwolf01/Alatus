use crate::capabilities::BatteryCapabilities;
use crate::error::AlatusError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatteryStatus {
    Charging,
    Discharging,
    NotCharging,
    Full,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryInfo {
    pub percentage: u8,
    pub status: BatteryStatus,
    pub charge_limit: Option<u8>,
    pub health_percentage: Option<u8>,
    pub power_now_microwatts: Option<u64>,
}

#[async_trait]
pub trait BatteryDriver: Send + Sync {
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> BatteryCapabilities;

    async fn get_info(&self) -> Result<BatteryInfo, AlatusError>;
    async fn get_charge_limit(&self) -> Result<u8, AlatusError>;
    async fn set_charge_limit(&self, limit: u8) -> Result<(), AlatusError>;
    async fn get_health_percentage(&self) -> Result<u8, AlatusError>;
    async fn is_charging(&self) -> Result<bool, AlatusError>;
}
