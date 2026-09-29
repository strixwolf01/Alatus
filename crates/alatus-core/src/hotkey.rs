use crate::error::AlatusError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HotkeyAction {
    FanModeToggle,
    AuraModeToggle,
    BrightnessUp,
    BrightnessDown,
    MicMuteToggle,
    TouchpadToggle,
    Sleep,
    Custom(u32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyEvent {
    pub action: HotkeyAction,
    pub raw_code: u32,
}

#[async_trait]
pub trait HotkeyDriver: Send + Sync {
    fn name(&self) -> &'static str;
    async fn next_event(&mut self) -> Result<HotkeyEvent, AlatusError>;
}
