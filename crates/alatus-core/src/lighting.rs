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

impl LightingMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            LightingMode::Static => "Static",
            LightingMode::Breathing => "Breathing",
            LightingMode::Strobe => "Strobe",
            LightingMode::Rainbow => "Rainbow",
            LightingMode::Off => "Off",
        }
    }
}

impl std::fmt::Display for LightingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for LightingMode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "static" => Ok(LightingMode::Static),
            "breathing" => Ok(LightingMode::Breathing),
            "strobe" => Ok(LightingMode::Strobe),
            "rainbow" => Ok(LightingMode::Rainbow),
            "off" => Ok(LightingMode::Off),
            _ => Err(()),
        }
    }
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
    fn supported_modes(&self) -> Vec<LightingMode> {
        vec![LightingMode::Static]
    }

    async fn set_brightness(&self, level: u8) -> Result<(), AlatusError>;
    async fn get_brightness(&self) -> Result<u8, AlatusError>;
    async fn apply_effect(&self, effect: &LightingEffect) -> Result<(), AlatusError>;
}
