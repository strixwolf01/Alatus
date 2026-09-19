// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::domain::{ColorRgb, ThermalMode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

pub const DEFAULT_STATE_DIR: &str = "/var/lib/alatus";
pub const DEFAULT_STATE_FILE: &str = "/var/lib/alatus/state.json";

/// Authoritative system-level hardware state persisted across daemon runs,
/// system reboots, and sleep/resume cycles.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DaemonPersistentState {
    /// Battery charge stop threshold percentage (e.g., 60, 80, 100).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub charge_limit: Option<u32>,

    /// Last selected firmware/thermal profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thermal_mode: Option<ThermalMode>,

    /// True when the user explicitly chose a thermal profile, preventing
    /// auto-profile policy reconciliation from overriding it on sleep resume.
    #[serde(default)]
    pub manual_thermal_override: bool,

    /// Armoury platform power limits (SPL, SPPT, FPPT, Dynamic Boost, etc.).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub ppt_limits: HashMap<String, u32>,

    /// Hardware GPU MUX switch state (0 = Discrete, 1 = Dynamic/Hybrid).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_mux_mode: Option<u8>,

    /// Display panel response overdrive switch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panel_overdrive: Option<bool>,

    /// Active RGB backlight static color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rgb_color: Option<ColorRgb>,

    /// Active RGB backlight brightness (0..=100 or 0..=3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rgb_brightness: Option<u8>,

    /// Keyboard RGB inactivity timeout in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rgb_timeout_seconds: Option<u32>,

    /// Inactivity timeout policy ("never", "battery_only", "always").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rgb_timeout_policy: Option<String>,
}

impl DaemonPersistentState {
    /// Loads the authoritative state from the default system path `/var/lib/alatus/state.json`.
    /// Falls back gracefully to default state if the file does not exist or is corrupted.
    pub fn load() -> Self {
        Self::load_from_path(Path::new(DEFAULT_STATE_FILE))
    }

    /// Loads the authoritative state from a specified path.
    pub fn load_from_path(path: &Path) -> Self {
        if !path.exists() {
            tracing::info!(
                "No persistent state file found at {}. Using defaults.",
                path.display()
            );
            return Self::default();
        }

        match File::open(path) {
            Ok(mut file) => {
                let mut contents = String::new();
                if let Err(e) = file.read_to_string(&mut contents) {
                    tracing::warn!(
                        "Failed to read state file at {}: {e}. Falling back to defaults.",
                        path.display()
                    );
                    return Self::default();
                }

                match serde_json::from_str::<Self>(&contents) {
                    Ok(state) => {
                        tracing::info!("Loaded persistent state from {}", path.display());
                        state
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Corrupted state file at {}: {e}. Falling back to defaults.",
                            path.display()
                        );
                        Self::default()
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to open state file at {}: {e}. Falling back to defaults.",
                    path.display()
                );
                Self::default()
            }
        }
    }

    /// Atomically persists the state to `/var/lib/alatus/state.json`.
    ///
    /// Guarantees power-loss durability:
    /// 1. Ensures parent directory exists with `0755` permissions.
    /// 2. Writes to a temporary file in the exact same directory.
    /// 3. Calls `sync_all()` on the file handle.
    /// 4. Sets permissions `0644`.
    /// 5. Atomically renames the temporary file over the destination file.
    pub fn save_atomic(&self) -> Result<(), std::io::Error> {
        Self::save_atomic_to_path(self, Path::new(DEFAULT_STATE_FILE))
    }

    /// Atomically persists the state to a specified file path.
    pub fn save_atomic_to_path(&self, path: &Path) -> Result<(), std::io::Error> {
        let parent_dir = path
            .parent()
            .unwrap_or_else(|| Path::new(DEFAULT_STATE_DIR));
        if !parent_dir.exists() {
            fs::create_dir_all(parent_dir)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(parent_dir, fs::Permissions::from_mode(0o755));
            }
        }

        let json_bytes = serde_json::to_vec_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let unique_id = std::process::id();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let tmp_file_name = format!(
            ".{}.tmp.{}.{}",
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("state.json"),
            unique_id,
            timestamp
        );
        let tmp_path = parent_dir.join(tmp_file_name);

        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)?;

        file.write_all(&json_bytes)?;
        file.sync_all()?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = file.set_permissions(fs::Permissions::from_mode(0o644));
        }
        drop(file);

        if let Err(e) = fs::rename(&tmp_path, path) {
            let _ = fs::remove_file(&tmp_path);
            return Err(e);
        }

        // Best effort: sync parent directory to ensure rename durability
        if let Ok(dir) = File::open(parent_dir) {
            let _ = dir.sync_all();
        }

        tracing::debug!("Atomically saved persistent state to {}", path.display());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_state_save_and_load() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("state.json");

        let mut ppt_limits = HashMap::new();
        ppt_limits.insert("ppt_pl1_spl".to_string(), 65);
        ppt_limits.insert("ppt_pl2_sppt".to_string(), 80);

        let state = DaemonPersistentState {
            charge_limit: Some(80),
            thermal_mode: Some(ThermalMode::Performance),
            manual_thermal_override: true,
            ppt_limits,
            gpu_mux_mode: Some(0),
            panel_overdrive: Some(true),
            rgb_color: Some(ColorRgb::new(255, 0, 128)),
            rgb_brightness: Some(60),
            rgb_timeout_seconds: Some(120),
            rgb_timeout_policy: Some("battery_only".to_string()),
        };

        assert!(DaemonPersistentState::save_atomic_to_path(&state, &file_path).is_ok());

        let loaded = DaemonPersistentState::load_from_path(&file_path);
        assert_eq!(state, loaded);
    }

    #[test]
    fn test_atomic_overwrite_preserves_content() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("state.json");

        let mut state = DaemonPersistentState {
            charge_limit: Some(60),
            ..Default::default()
        };
        assert!(DaemonPersistentState::save_atomic_to_path(&state, &file_path).is_ok());

        // Update and save again
        state.charge_limit = Some(90);
        state.manual_thermal_override = true;
        state.thermal_mode = Some(ThermalMode::Quiet);
        assert!(DaemonPersistentState::save_atomic_to_path(&state, &file_path).is_ok());

        let loaded = DaemonPersistentState::load_from_path(&file_path);
        assert_eq!(loaded.charge_limit, Some(90));
        assert!(loaded.manual_thermal_override);
        assert_eq!(loaded.thermal_mode, Some(ThermalMode::Quiet));
    }

    #[test]
    fn test_corrupt_file_recovery() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("state.json");

        fs::write(&file_path, "{ broken json: 123 ").unwrap();

        let loaded = DaemonPersistentState::load_from_path(&file_path);
        assert_eq!(loaded, DaemonPersistentState::default());
    }

    #[test]
    fn test_missing_file_recovery() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("nonexistent_state.json");

        let loaded = DaemonPersistentState::load_from_path(&file_path);
        assert_eq!(loaded, DaemonPersistentState::default());
    }
}
