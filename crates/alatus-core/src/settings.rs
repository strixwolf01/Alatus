//! Alatus User Settings persistence module.
//!
//! Stores user-level desktop preferences and hardware states across sessions,
//! application closures, and system reboots.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn default_true() -> bool {
    true
}
fn default_battery_limit() -> u8 {
    80
}
fn default_thermal_profile() -> String {
    "Balanced".to_string()
}
fn default_brightness() -> u8 {
    3
}
fn default_lighting_mode() -> String {
    "Static".to_string()
}
fn default_rgb_color() -> (u8, u8, u8) {
    (0, 240, 255)
}
fn default_kbd_idle_mode() -> String {
    "always_on".to_string()
}
fn default_kbd_idle_timeout() -> u32 {
    1
}
fn default_refresh_rate() -> u32 {
    120
}
fn default_dimming() -> u32 {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlatusSettings {
    #[serde(default = "default_battery_limit")]
    pub battery_limit: u8,
    #[serde(default = "default_thermal_profile")]
    pub thermal_profile: String,
    #[serde(default = "default_brightness")]
    pub lighting_brightness: u8,
    #[serde(default = "default_lighting_mode")]
    pub lighting_mode: String,
    #[serde(default = "default_rgb_color")]
    pub lighting_color: (u8, u8, u8),
    #[serde(default)]
    pub sync_accent_color: bool,
    #[serde(default = "default_kbd_idle_mode")]
    pub kbd_idle_mode: String,
    #[serde(default = "default_kbd_idle_timeout")]
    pub kbd_idle_timeout_min: u32,
    #[serde(default = "default_kbd_idle_timeout")]
    pub kbd_idle_timeout_ac_min: u32,
    #[serde(default = "default_kbd_idle_timeout")]
    pub kbd_idle_timeout_bat_min: u32,

    #[serde(default = "default_true")]
    pub auto_refresh: bool,
    #[serde(default = "default_refresh_rate")]
    pub display_refresh_rate: u32,
    #[serde(default = "default_dimming")]
    pub oled_dimming_level: u32,
    #[serde(default)]
    pub target_mode_active: bool,
    #[serde(default)]
    pub panel_autohide_active: bool,
    #[serde(default)]
    pub panel_transparency_active: bool,
    #[serde(default)]
    pub dpms_pixel_refresh_active: bool,
    #[serde(default = "default_true")]
    pub stay_in_tray: bool,
    #[serde(default = "default_true")]
    pub start_on_boot: bool,
    #[serde(default = "default_true")]
    pub touchpad_gestures_active: bool,
}

impl Default for AlatusSettings {
    fn default() -> Self {
        Self {
            battery_limit: default_battery_limit(),
            thermal_profile: default_thermal_profile(),
            lighting_brightness: default_brightness(),
            lighting_mode: default_lighting_mode(),
            lighting_color: default_rgb_color(),
            sync_accent_color: false,
            kbd_idle_mode: default_kbd_idle_mode(),
            kbd_idle_timeout_min: default_kbd_idle_timeout(),
            kbd_idle_timeout_ac_min: default_kbd_idle_timeout(),
            kbd_idle_timeout_bat_min: default_kbd_idle_timeout(),
            auto_refresh: true,
            display_refresh_rate: default_refresh_rate(),
            oled_dimming_level: default_dimming(),
            target_mode_active: false,
            panel_autohide_active: false,
            panel_transparency_active: false,
            dpms_pixel_refresh_active: false,
            stay_in_tray: true,
            start_on_boot: true,
            touchpad_gestures_active: true,
        }
    }
}

impl AlatusSettings {
    pub fn config_path() -> Option<PathBuf> {
        let home = std::env::var("HOME").ok()?;
        Some(PathBuf::from(home).join(".config/alatus/settings.json"))
    }

    pub fn load() -> Self {
        if let Some(path) = Self::config_path() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(settings) = serde_json::from_str::<Self>(&content) {
                    return settings;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(path) = Self::config_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let data = serde_json::to_string_pretty(self)?;
            std::fs::write(path, data)?;
        }
        Ok(())
    }
}
