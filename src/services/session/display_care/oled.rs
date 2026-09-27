// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! OLED metrics persistence and cumulative panel operating hours tracking.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub fn default_oled_dim_level() -> u32 {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OledMetrics {
    pub active_screen_seconds: u64,
    pub refresh_count: u32,
    pub last_refresh_timestamp: u64,
    #[serde(default = "default_oled_dim_level")]
    pub oled_dim_level: u32,
}

impl Default for OledMetrics {
    fn default() -> Self {
        Self {
            active_screen_seconds: 0,
            refresh_count: 0,
            last_refresh_timestamp: 0,
            oled_dim_level: default_oled_dim_level(),
        }
    }
}

pub fn get_metrics_path() -> Option<PathBuf> {
    let base = if let Ok(dir) = std::env::var("XDG_STATE_HOME") {
        PathBuf::from(dir).join("alatus")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home)
            .join(".local")
            .join("state")
            .join("alatus")
    } else {
        return None;
    };

    let metrics_file = base.join("metrics.json");
    let legacy_file = base.join("session_metrics.json");
    if metrics_file.exists() {
        Some(metrics_file)
    } else if legacy_file.exists() {
        Some(legacy_file)
    } else {
        let legacy_ascend = base.parent().map(|p| p.join("ascend").join("metrics.json"));
        if let Some(ref l) = legacy_ascend
            && l.exists()
        {
            legacy_ascend
        } else {
            Some(metrics_file)
        }
    }
}

pub fn get_primary_metrics_path() -> Option<PathBuf> {
    let base = if let Ok(dir) = std::env::var("XDG_STATE_HOME") {
        PathBuf::from(dir).join("alatus")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home)
            .join(".local")
            .join("state")
            .join("alatus")
    } else {
        return None;
    };
    Some(base.join("metrics.json"))
}

pub fn load_oled_metrics() -> OledMetrics {
    load_oled_metrics_from(get_metrics_path().as_deref())
}

pub fn load_oled_metrics_from(path: Option<&Path>) -> OledMetrics {
    let Some(path) = path else {
        return OledMetrics::default();
    };
    if let Ok(content) = fs::read_to_string(path)
        && let Ok(metrics) = serde_json::from_str::<OledMetrics>(&content)
    {
        return metrics;
    }
    // Fallback: if path was metrics.json, check session_metrics.json in parent dir
    if let Some(parent) = path.parent() {
        let legacy = parent.join("session_metrics.json");
        if legacy != path
            && legacy.exists()
            && let Ok(content) = fs::read_to_string(&legacy)
            && let Ok(metrics) = serde_json::from_str::<OledMetrics>(&content)
        {
            return metrics;
        }
    }
    OledMetrics::default()
}

pub fn save_oled_metrics(metrics: &OledMetrics) {
    save_oled_metrics_to(get_primary_metrics_path().as_deref(), metrics);
}

pub fn save_oled_metrics_to(path: Option<&Path>, metrics: &OledMetrics) {
    let Some(path) = path else { return };
    if let Some(parent) = path.parent() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            builder.mode(0o700);
            let _ = builder.create(parent);
        }
        #[cfg(not(unix))]
        {
            let _ = fs::create_dir_all(parent);
        }
    }
    if let Ok(json) = serde_json::to_string_pretty(metrics) {
        let _ = fs::write(path, json);
    }
}

pub fn get_epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub async fn execute_pixel_refresh(
    session_conn: &zbus::Connection,
    state: std::sync::Arc<crate::services::session::state::SessionState>,
    _desktop: crate::services::session::desktop::DesktopEnv,
    internal_connector: Option<String>,
) -> bool {
    use std::sync::atomic::Ordering;

    // Atomic test-and-set: prevent concurrent conditioning executions
    if state
        .pixel_refresh_in_progress
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        tracing::warn!("Pixel refresh already in progress. Ignoring duplicate trigger.");
        return false;
    }

    let _ = crate::services::session::notifications::send_desktop_notification(
        session_conn,
        "OLED Care: Pixel Refresh Started",
        "Conditioning OLED panel to relieve subpixel stress...",
    )
    .await;

    // Reset usage counter and increment cycle count
    state.active_screen_seconds.store(0, Ordering::Relaxed);
    let count = state.pixel_refresh_count.fetch_add(1, Ordering::Relaxed) + 1;
    save_oled_metrics(&OledMetrics {
        active_screen_seconds: 0,
        refresh_count: count,
        last_refresh_timestamp: get_epoch_seconds(),
        oled_dim_level: state.oled_dim_level.load(Ordering::Relaxed),
    });

    let conn_clone = session_conn.clone();
    let state_clone = std::sync::Arc::clone(&state);
    tokio::spawn(async move {
        struct RefreshGuard(std::sync::Arc<crate::services::session::state::SessionState>);
        impl Drop for RefreshGuard {
            fn drop(&mut self) {
                self.0
                    .pixel_refresh_in_progress
                    .store(false, Ordering::SeqCst);
            }
        }
        let _guard = RefreshGuard(std::sync::Arc::clone(&state_clone));

        let resolved_connector = match internal_connector {
            Some(c) => Some(c),
            None => crate::services::session::desktop::find_kde_internal_connector().await,
        };

        // Deep conditioning blackout pulse at compositor level (0% total black, never touch /sys/class/backlight)
        crate::services::session::display_care::dimming::apply_flicker_free_dimming(
            resolved_connector.as_deref(),
            0,
        )
        .await;
        tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
        let restored_level = state_clone
            .oled_dim_level
            .load(Ordering::Relaxed)
            .clamp(10, 100);
        crate::services::session::display_care::dimming::apply_flicker_free_dimming(
            resolved_connector.as_deref(),
            restored_level,
        )
        .await;

        let _ = crate::services::session::notifications::send_desktop_notification(
            &conn_clone,
            "OLED Care: Pixel Refresh Completed",
            "Panel conditioning cycle finished successfully.",
        )
        .await;
    });

    true
}

pub async fn apply_oled_care_dimming(
    _session_conn: &zbus::Connection,
    daemon_client: &crate::services::daemon_client::DaemonClient,
    state: &crate::services::session::state::SessionState,
    _desktop: crate::services::session::desktop::DesktopEnv,
    internal_connector: Option<&str>,
    dim: bool,
) {
    use std::sync::atomic::Ordering;

    if !state.oled_care.load(Ordering::Relaxed) {
        return;
    }

    if dim {
        // Prevent cache clobbering: never overwrite cached values if already dimmed
        if !state.try_enter_dimmed() {
            return;
        }

        tracing::info!("OLED Care: User idle detected. Applying software compositor dimming.");

        // Cache pre-dim keyboard RGB brightness
        if let Ok(rgb_status) = daemon_client.get_rgb_status().await
            && rgb_status.available
        {
            state
                .cached_rgb_brightness
                .store(rgb_status.brightness, Ordering::Relaxed);
        }

        // Apply software dimming at compositor level (never touches hardware backlight)
        crate::services::session::display_care::dimming::apply_flicker_free_dimming(
            internal_connector,
            20,
        )
        .await;

        // Dim keyboard RGB to 0%
        let _ = daemon_client.set_rgb_brightness(0).await;
    } else {
        // Wake-up event
        if !state.try_exit_dimmed() {
            return;
        }

        tracing::info!("OLED Care: User return detected. Restoring software luminance.");
        let restored_level = state.oled_dim_level.load(Ordering::Relaxed).clamp(10, 100);
        crate::services::session::display_care::dimming::apply_flicker_free_dimming(
            internal_connector,
            restored_level,
        )
        .await;

        // Restore keyboard RGB brightness
        let cached_rgb = state.cached_rgb_brightness.load(Ordering::Relaxed);
        let _ = daemon_client.set_rgb_brightness(cached_rgb).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_save_oled_metrics_fallback() {
        let temp_dir = std::env::temp_dir().join(format!("alatus_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);

        let metrics_file = temp_dir.join("metrics.json");
        let legacy_file = temp_dir.join("session_metrics.json");

        // When legacy exists and metrics does not:
        let initial_metrics = OledMetrics {
            active_screen_seconds: 3600,
            refresh_count: 5,
            last_refresh_timestamp: 123456,
            oled_dim_level: 50,
        };
        save_oled_metrics_to(Some(&legacy_file), &initial_metrics);

        // Loading from metrics_file should fall back to legacy_file
        let loaded = load_oled_metrics_from(Some(&metrics_file));
        assert_eq!(loaded.active_screen_seconds, 3600);
        assert_eq!(loaded.refresh_count, 5);
        assert_eq!(loaded.oled_dim_level, 50);

        // Now save to metrics_file
        let updated_metrics = OledMetrics {
            active_screen_seconds: 7200,
            refresh_count: 6,
            last_refresh_timestamp: 123457,
            oled_dim_level: 70,
        };
        save_oled_metrics_to(Some(&metrics_file), &updated_metrics);

        // Loading should now load updated_metrics from metrics_file directly
        let loaded_updated = load_oled_metrics_from(Some(&metrics_file));
        assert_eq!(loaded_updated.active_screen_seconds, 7200);
        assert_eq!(loaded_updated.refresh_count, 6);
        assert_eq!(loaded_updated.oled_dim_level, 70);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}


