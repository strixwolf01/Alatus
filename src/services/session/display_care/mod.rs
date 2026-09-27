// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Display care subsystem for user desktop sessions.
//!
//! Includes OLED burn-in prevention, compositor flicker-free dimming,
//! usage tracking metrics, and dynamic refresh rate coordination.

pub mod dimming;
pub mod oled;
pub mod refresh;

pub use dimming::apply_flicker_free_dimming;
pub use oled::{
    apply_oled_care_dimming, default_oled_dim_level, execute_pixel_refresh, get_epoch_seconds,
    get_metrics_path, get_primary_metrics_path, load_oled_metrics, load_oled_metrics_from,
    save_oled_metrics, save_oled_metrics_to, OledMetrics,
};
pub use refresh::{apply_panel_refresh_rate, set_panel_refresh_rate};
