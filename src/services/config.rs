// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Unified atomic configuration persistence for Alatus.
//!
//! Stores user preferences and hardware states across reboots at:
//! `$XDG_CONFIG_HOME/alatus/config.json` (or `~/.config/alatus/config.json`).

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RgbTimeoutPolicy {
    Never,
    BatteryOnly,
    #[default]
    Always,
}

impl std::fmt::Display for RgbTimeoutPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Never => write!(f, "never"),
            Self::BatteryOnly => write!(f, "battery"),
            Self::Always => write!(f, "always"),
        }
    }
}

impl std::str::FromStr for RgbTimeoutPolicy {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().trim() {
            "never" | "off" | "none" => Ok(Self::Never),
            "battery" | "battery_only" | "batteryonly" | "bat" => Ok(Self::BatteryOnly),
            "always" | "all" | "on" => Ok(Self::Always),
            other => Err(format!(
                "Invalid RGB timeout policy '{other}' (expected 'never', 'battery', or 'always')"
            )),
        }
    }
}

fn default_rgb_timeout_seconds() -> u32 {
    60
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlatusConfig {
    pub thermal_mode: u32,
    pub charge_limit: u32,
    pub tray_enabled: bool,
    pub autostart_enabled: bool,
    pub rgb_preset: i32,
    pub rgb_brightness: u32,
    pub custom_rgb: (u8, u8, u8),
    pub oled_care_enabled: bool,
    pub oled_dim_level: u32,
    pub refresh_rate: u32,
    #[serde(default = "default_rgb_timeout_seconds")]
    pub rgb_timeout_seconds: u32,
    #[serde(default)]
    pub rgb_timeout_policy: RgbTimeoutPolicy,
}

impl Default for AlatusConfig {
    fn default() -> Self {
        Self {
            thermal_mode: 1, // Balanced
            charge_limit: 80,
            tray_enabled: true,
            autostart_enabled: false,
            rgb_preset: 0, // Auto-Sync
            rgb_brightness: 80,
            custom_rgb: (208, 188, 255),
            oled_care_enabled: true,
            oled_dim_level: 100,
            refresh_rate: 120,
            rgb_timeout_seconds: 60,
            rgb_timeout_policy: RgbTimeoutPolicy::Always,
        }
    }
}

pub fn get_config_path() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            PathBuf::from(home).join(".config")
        });
    base.join("alatus").join("config.json")
}

pub fn load_config() -> AlatusConfig {
    let path = get_config_path();
    if let Ok(data) = std::fs::read_to_string(&path)
        && let Ok(cfg) = serde_json::from_str::<AlatusConfig>(&data)
    {
        return cfg;
    }

    // Fallback search across /home/* for system daemon running as root
    if let Ok(entries) = std::fs::read_dir("/home") {
        for entry in entries.flatten() {
            let candidate = entry
                .path()
                .join(".config")
                .join("alatus")
                .join("config.json");
            if candidate.exists()
                && let Ok(data) = std::fs::read_to_string(&candidate)
                && let Ok(cfg) = serde_json::from_str::<AlatusConfig>(&data)
            {
                return cfg;
            }
        }
    }

    // Migration fallback for legacy ~/.config/ascend/config.json
    let legacy_ascend = path
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("ascend").join("config.json"));
    if let Some(legacy_path) = legacy_ascend
        && let Ok(data) = std::fs::read_to_string(&legacy_path)
        && let Ok(cfg) = serde_json::from_str::<AlatusConfig>(&data)
    {
        return cfg;
    }

    // Migration fallback for legacy gui_config.json
    let legacy_path = path.with_file_name("gui_config.json");
    if let Ok(data) = std::fs::read_to_string(&legacy_path)
        && let Ok(legacy) = serde_json::from_str::<serde_json::Value>(&data)
    {
        let mut cfg = AlatusConfig::default();
        if let Some(t) = legacy.get("tray_enabled").and_then(|v| v.as_bool()) {
            cfg.tray_enabled = t;
        }
        return cfg;
    }

    AlatusConfig::default()
}

pub fn save_config_atomic(cfg: &AlatusConfig) -> std::io::Result<()> {
    let path = get_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let tmp_path = path.with_extension("tmp");
    let json_bytes = serde_json::to_vec_pretty(cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    {
        let mut file = std::fs::File::create(&tmp_path)?;
        file.write_all(&json_bytes)?;
        file.sync_all()?;
    }

    std::fs::rename(&tmp_path, &path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = AlatusConfig::default();
        assert_eq!(cfg.thermal_mode, 1);
        assert_eq!(cfg.charge_limit, 80);
        assert!(cfg.tray_enabled);
        assert!(!cfg.autostart_enabled);
        assert_eq!(cfg.rgb_preset, 0);
        assert_eq!(cfg.rgb_brightness, 80);
        assert!(cfg.oled_care_enabled);
        assert_eq!(cfg.oled_dim_level, 100);
        assert_eq!(cfg.refresh_rate, 120);
        assert_eq!(cfg.rgb_timeout_seconds, 60);
        assert_eq!(cfg.rgb_timeout_policy, RgbTimeoutPolicy::Always);
    }

    #[test]
    fn test_config_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_file = temp_dir.path().join("config.json");
        let tmp_file = temp_dir.path().join("config.tmp");

        let cfg = AlatusConfig {
            thermal_mode: 2,
            charge_limit: 60,
            tray_enabled: false,
            autostart_enabled: true,
            rgb_preset: 2,
            rgb_brightness: 50,
            custom_rgb: (255, 0, 128),
            oled_care_enabled: false,
            oled_dim_level: 80,
            refresh_rate: 60,
            rgb_timeout_seconds: 45,
            rgb_timeout_policy: RgbTimeoutPolicy::BatteryOnly,
        };

        let json_bytes = serde_json::to_vec_pretty(&cfg).unwrap();
        std::fs::write(&tmp_file, &json_bytes).unwrap();
        std::fs::rename(&tmp_file, &config_file).unwrap();

        let read_data = std::fs::read_to_string(&config_file).unwrap();
        let loaded: AlatusConfig = serde_json::from_str(&read_data).unwrap();
        assert_eq!(cfg, loaded);
    }

    #[test]
    fn test_rgb_timeout_policy_conversions() {
        use std::str::FromStr;

        assert_eq!(
            RgbTimeoutPolicy::from_str("never").unwrap(),
            RgbTimeoutPolicy::Never
        );
        assert_eq!(
            RgbTimeoutPolicy::from_str("battery").unwrap(),
            RgbTimeoutPolicy::BatteryOnly
        );
        assert_eq!(
            RgbTimeoutPolicy::from_str("always").unwrap(),
            RgbTimeoutPolicy::Always
        );

        assert_eq!(RgbTimeoutPolicy::Never.to_string(), "never");
        assert_eq!(RgbTimeoutPolicy::BatteryOnly.to_string(), "battery");
        assert_eq!(RgbTimeoutPolicy::Always.to_string(), "always");
    }
}
