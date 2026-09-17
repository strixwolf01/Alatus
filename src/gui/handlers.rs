// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use super::AppWindow;
use super::state_sync::*;
use super::tray::is_autostart_enabled;
use crate::services::config::{AlatusConfig, RgbTimeoutPolicy};
use slint::ComponentHandle;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;

pub enum GuiAction {
    SetThermalMode(i32),
    ToggleOledCare(bool),
    TriggerPixelRefresh,
    SetOledDimLevel(i32),
    SetRefreshRate(i32),
    SetRgbBrightness(i32),
    SetRgbTimeout(i32),
    SetRgbTimeoutPolicy(i32),
    SetRgbPreset(i32),
    ApplyCustomRgb(u8, u8, u8),
    SetChargeLimit(u32),
    RevealSerial,
    CopySerial,
    RunDiagnostics,
    ToggleTray(bool),
    ToggleAutostart(bool),
}

thread_local! {
    pub static APP_WINDOW: std::cell::RefCell<Option<AppWindow>> = const { std::cell::RefCell::new(None) };
}

pub fn with_app_window<F: FnOnce(&AppWindow)>(f: F) {
    APP_WINDOW.with(|cell| {
        if let Some(ref w) = *cell.borrow() {
            f(w);
        }
    });
}

pub fn setup_window(
    w: &AppWindow,
    tx: &mpsc::UnboundedSender<GuiAction>,
    tray_ref: Arc<AtomicBool>,
    cfg: &AlatusConfig,
    initial_palette: &M3Palette,
) {
    w.set_version_text(format!("v{}", env!("CARGO_PKG_VERSION")).into());
    w.set_tray_enabled(cfg.tray_enabled);
    w.set_autostart_enabled(is_autostart_enabled());

    w.set_thermal_mode(cfg.thermal_mode as i32);
    w.set_charge_limit(cfg.charge_limit as i32);
    w.set_rgb_brightness(cfg.rgb_brightness as i32);
    w.set_rgb_timeout(cfg.rgb_timeout_seconds as i32);
    let policy_int = match cfg.rgb_timeout_policy {
        RgbTimeoutPolicy::Never => 0,
        RgbTimeoutPolicy::BatteryOnly => 1,
        RgbTimeoutPolicy::Always => 2,
    };
    w.set_rgb_timeout_policy(policy_int);
    w.set_selected_rgb_preset(cfg.rgb_preset);
    w.set_custom_red(cfg.custom_rgb.0 as i32);
    w.set_custom_green(cfg.custom_rgb.1 as i32);
    w.set_custom_blue(cfg.custom_rgb.2 as i32);
    w.set_oled_care_enabled(cfg.oled_care_enabled);
    w.set_oled_dim_level(cfg.oled_dim_level as i32);
    w.set_selected_refresh_rate(cfg.refresh_rate as i32);

    // Initial Hardware & Desktop theming setup
    w.set_hw_model(get_product_model().into());
    w.set_hw_bios(get_bios_version().into());
    w.set_hw_kernel(get_kernel_version().into());
    w.set_hw_compositor(get_compositor_name().into());
    w.set_hw_battery_id(get_battery_model().into());
    w.set_hw_bios_kernel(get_bios_kernel_string().into());

    // Exhaustive System Specifications
    w.set_hw_cpu(get_cpu_specs().into());
    w.set_hw_ram(get_memory_specs().into());
    w.set_hw_motherboard(get_motherboard_specs().into());
    w.set_hw_bios_detailed(get_bios_detailed().into());
    w.set_hw_gpu(get_gpu_specs().into());

    // Dedicated Battery Health & Telemetry
    let bat = get_battery_health_details();
    w.set_hw_battery_status(bat.status.into());
    w.set_hw_battery_pct(bat.pct.into());
    w.set_hw_battery_health(bat.health.into());
    w.set_hw_battery_cycles(bat.cycles.into());
    w.set_hw_battery_manufacturer(bat.manufacturer.into());

    w.set_accent_color(initial_palette.accent_color);
    w.set_accent_container(initial_palette.accent_container);
    w.set_on_accent_container(initial_palette.on_accent_container);
    w.set_on_accent(initial_palette.on_accent);

    // Wire Slint callbacks to dispatch commands to Tokio
    {
        let tx = tx.clone();
        w.on_set_thermal_mode(move |mode| {
            let _ = tx.send(GuiAction::SetThermalMode(mode));
        });
    }
    {
        let tx = tx.clone();
        w.on_toggle_oled_care(move |enable| {
            let _ = tx.send(GuiAction::ToggleOledCare(enable));
        });
    }
    {
        let tx = tx.clone();
        w.on_trigger_pixel_refresh(move || {
            let _ = tx.send(GuiAction::TriggerPixelRefresh);
        });
    }
    {
        let tx = tx.clone();
        w.on_set_oled_dim_level(move |val| {
            let _ = tx.send(GuiAction::SetOledDimLevel(val));
        });
    }
    {
        let tx = tx.clone();
        w.on_set_refresh_rate(move |rate| {
            let _ = tx.send(GuiAction::SetRefreshRate(rate));
        });
    }
    {
        let tx = tx.clone();
        w.on_set_rgb_brightness(move |brightness| {
            let _ = tx.send(GuiAction::SetRgbBrightness(brightness));
        });
    }
    {
        let tx = tx.clone();
        w.on_set_rgb_timeout(move |seconds| {
            let _ = tx.send(GuiAction::SetRgbTimeout(seconds));
        });
    }
    {
        let tx = tx.clone();
        w.on_set_rgb_timeout_policy(move |policy| {
            let _ = tx.send(GuiAction::SetRgbTimeoutPolicy(policy));
        });
    }
    {
        let tx = tx.clone();
        w.on_set_rgb_preset(move |preset| {
            let _ = tx.send(GuiAction::SetRgbPreset(preset));
        });
    }
    {
        let tx = tx.clone();
        w.on_apply_custom_rgb(move |r, g, b| {
            let _ = tx.send(GuiAction::ApplyCustomRgb(
                r.clamp(0, 255) as u8,
                g.clamp(0, 255) as u8,
                b.clamp(0, 255) as u8,
            ));
        });
    }
    {
        let tx = tx.clone();
        w.on_set_charge_limit(move |limit| {
            let _ = tx.send(GuiAction::SetChargeLimit(limit.clamp(50, 100) as u32));
        });
    }
    {
        let tx = tx.clone();
        w.on_reveal_serial(move || {
            let _ = tx.send(GuiAction::RevealSerial);
        });
    }
    {
        let tx = tx.clone();
        w.on_copy_serial(move || {
            let _ = tx.send(GuiAction::CopySerial);
        });
    }
    {
        let tx = tx.clone();
        w.on_run_diagnostics(move || {
            let _ = tx.send(GuiAction::RunDiagnostics);
        });
    }
    {
        let tx = tx.clone();
        w.on_toggle_tray(move |val| {
            let _ = tx.send(GuiAction::ToggleTray(val));
        });
    }
    {
        let tx = tx.clone();
        w.on_toggle_autostart(move |val| {
            let _ = tx.send(GuiAction::ToggleAutostart(val));
        });
    }

    // Intercept window close request: hide to tray if enabled, or terminate app cleanly
    {
        let weak_close = w.as_weak();
        w.window().on_close_requested(move || {
            if tray_ref.load(Ordering::Relaxed) {
                if let Some(win) = weak_close.upgrade() {
                    let _ = win.hide();
                }
                slint::CloseRequestResponse::KeepWindowShown
            } else {
                let _ = slint::quit_event_loop();
                slint::CloseRequestResponse::HideWindow
            }
        });
    }
}
