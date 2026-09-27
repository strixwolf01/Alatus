// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Shared atomic state container for the user desktop session integration agent.

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering};

use super::display_care::oled::{load_oled_metrics, OledMetrics};

#[derive(Debug)]
pub struct SessionState {
    pub sync_accent: AtomicBool,
    pub auto_refresh: AtomicBool,
    pub oled_care: AtomicBool,
    pub power_monitor_active: AtomicBool,
    pub is_dimmed: AtomicBool,
    pub cached_panel_brightness: AtomicI32,
    pub cached_rgb_brightness: AtomicU32,
    pub active_screen_seconds: AtomicU64,
    pub pixel_refresh_count: AtomicU32,
    pub pixel_refresh_in_progress: AtomicBool,
    pub oled_dim_level: AtomicU32,
}

impl SessionState {
    pub fn new(sync_accent: bool, auto_refresh: bool, oled_care: bool) -> Self {
        Self::with_metrics(sync_accent, auto_refresh, oled_care, load_oled_metrics())
    }

    pub fn with_metrics(
        sync_accent: bool,
        auto_refresh: bool,
        oled_care: bool,
        metrics: OledMetrics,
    ) -> Self {
        Self {
            sync_accent: AtomicBool::new(sync_accent),
            auto_refresh: AtomicBool::new(auto_refresh),
            oled_care: AtomicBool::new(oled_care),
            power_monitor_active: AtomicBool::new(false),
            is_dimmed: AtomicBool::new(false),
            cached_panel_brightness: AtomicI32::new(-1),
            cached_rgb_brightness: AtomicU32::new(100),
            active_screen_seconds: AtomicU64::new(metrics.active_screen_seconds),
            pixel_refresh_count: AtomicU32::new(metrics.refresh_count),
            pixel_refresh_in_progress: AtomicBool::new(false),
            oled_dim_level: AtomicU32::new(metrics.oled_dim_level),
        }
    }

    /// Attempts to enter the dimmed state atomically.
    /// Returns true if successfully transitioned from not-dimmed to dimmed,
    /// or false if already dimmed (preventing pre-dim cache clobbering).
    pub fn try_enter_dimmed(&self) -> bool {
        self.is_dimmed
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// Attempts to exit the dimmed state atomically.
    /// Returns true if successfully transitioned from dimmed to not-dimmed.
    pub fn try_exit_dimmed(&self) -> bool {
        self.is_dimmed
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }
}
