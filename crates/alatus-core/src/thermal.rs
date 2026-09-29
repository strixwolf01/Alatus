use crate::capabilities::ThermalCapabilities;
use crate::error::AlatusError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThermalProfileMode {
    Quiet,
    Balanced,
    Performance,
    FullSpeed,
    Custom(u8),
}

impl std::fmt::Display for ThermalProfileMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Quiet => write!(f, "Quiet"),
            Self::Balanced => write!(f, "Balanced"),
            Self::Performance => write!(f, "Performance"),
            Self::FullSpeed => write!(f, "FullSpeed"),
            Self::Custom(id) => write!(f, "Custom({})", id),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FanStatus {
    pub label: String,
    pub current_rpm: u32,
    pub max_rpm: Option<u32>,
}

#[async_trait]
pub trait ThermalDriver: Send + Sync {
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> ThermalCapabilities;
    fn is_cpu_only(&self) -> bool {
        true
    }

    async fn available_profiles(&self) -> Result<Vec<ThermalProfileMode>, AlatusError>;
    async fn get_current_profile(&self) -> Result<ThermalProfileMode, AlatusError>;
    async fn set_profile(&self, mode: ThermalProfileMode) -> Result<(), AlatusError>;
    async fn get_fans(&self) -> Result<Vec<FanStatus>, AlatusError>;
}
