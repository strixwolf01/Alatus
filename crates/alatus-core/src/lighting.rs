use crate::capabilities::LightingCapabilities;
use crate::error::AlatusError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LightingMode {
    Static,
    Breathing,
    Strobe,
    Rainbow,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbColor {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightingEffect {
    pub mode: LightingMode,
    pub primary_color: RgbColor,
    pub secondary_color: Option<RgbColor>,
    pub speed: u8,
    pub brightness: u8,
}

#[async_trait]
pub trait LightingDriver: Send + Sync {
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> LightingCapabilities;

    async fn set_brightness(&self, level: u8) -> Result<(), AlatusError>;
    async fn get_brightness(&self) -> Result<u8, AlatusError>;
    async fn apply_effect(&self, effect: &LightingEffect) -> Result<(), AlatusError>;
}
