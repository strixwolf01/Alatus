// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

slint::include_modules!();

pub mod handlers;
pub mod state_sync;
pub mod tray;

pub use handlers::*;
pub use state_sync::*;
pub use tray::*;

use crate::services::config::{RgbTimeoutPolicy, load_config, save_config_atomic};
use crate::services::daemon_client::{DaemonClient, get_daemon_client};
use crate::services::desktop_session::{
    apply_flicker_free_dimming, is_session_daemon_running, load_oled_metrics, query_display_info,
    query_session_status, read_portal_accent_color, save_oled_metrics, send_desktop_notification,
    set_panel_refresh_rate, set_session_accent_sync, set_session_auto_refresh,
    set_session_oled_care, set_session_oled_dim_level, trigger_session_pixel_refresh,
};
use crate::services::firmware_mode::FirmwareMode;
use crate::services::telemetry::read_thermal_telemetry;
use crate::services::tray::{TrayEvent, start_tray_service};
use slint::ComponentHandle;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

pub async fn run_gui(minimized: bool) -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    info!("Starting Alatus Hardware Control Center GUI...");

    let args: Vec<String> = std::env::args().collect();
    let start_minimized = minimized
        || args
            .iter()
            .any(|a| a == "--minimized" || a == "-m" || a == "--tray");

    // Single-Instance File Lock: Guard against concurrent execution at kernel level
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            let tmp = std::env::temp_dir();
            let uid = unsafe { libc::getuid() };
            tmp.join(format!("alatus-runtime-{uid}"))
        });
    let _ = std::fs::create_dir_all(&runtime_dir);
    let lock_path = runtime_dir.join("alatus-gui.lock");

    let lock_file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path);

    let _lock_guard = match lock_file {
        Ok(file) => {
            use std::os::unix::io::AsRawFd;
            let res = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
            if res == 0 { Some(file) } else { None }
        }
        Err(_) => None,
    };

    if _lock_guard.is_none() {
        if start_minimized {
            info!("Another Alatus GUI instance is already running; exiting tray launch.");
            std::process::exit(0);
        } else {
            info!("Alatus GUI is already running. Raising active window.");
            if let Ok(conn) = zbus::Connection::session().await
                && let Ok(proxy) = zbus::Proxy::new(
                    &conn,
                    "io.strixwolf.alatus.Gui",
                    "/io/strixwolf/alatus/Gui",
                    "io.strixwolf.alatus.Gui",
                )
                .await
            {
                let _ = proxy.call::<_, _, ()>("ShowWindow", &()).await;
            }
            std::process::exit(0);
        }
    }

    // Associate Wayland window with desktop entry and scalable SVG icon
    let _ = slint::set_xdg_app_id("io.strixwolf.alatus");

    // D-Bus Single-Instance Guard: Prevent multiple instances from spawning duplicate tray icons
    let (tray_tx, mut tray_rx) = mpsc::unbounded_channel::<TrayEvent>();
    let session_conn = match zbus::Connection::session().await {
        Ok(conn) => {
            let already_running = if let Ok(dbus) = zbus::fdo::DBusProxy::new(&conn).await
                && let Ok(name) = zbus::names::WellKnownName::try_from("io.strixwolf.alatus.Gui")
            {
                dbus.name_has_owner(name.into()).await.unwrap_or(false)
            } else {
                false
            };

            if already_running {
                info!("Alatus GUI is already running. Raising active window.");
                if let Ok(proxy) = zbus::Proxy::new(
                    &conn,
                    "io.strixwolf.alatus.Gui",
                    "/io/strixwolf/alatus/Gui",
                    "io.strixwolf.alatus.Gui",
                )
                .await
                {
                    let _ = proxy.call::<_, _, ()>("ShowWindow", &()).await;
                }
                std::process::exit(0);
            }

            // Register IPC server before requesting well-known name
            let ipc_server = GuiIpcServer {
                tray_tx: tray_tx.clone(),
            };
            if let Err(e) = conn
                .object_server()
                .at("/io/strixwolf/alatus/Gui", ipc_server)
                .await
            {
                warn!("Failed to register GUI IPC server on D-Bus: {e}");
            } else {
                info!("GUI IPC server registered at /io/strixwolf/alatus/Gui");
            }

            match conn
                .request_name_with_flags(
                    "io.strixwolf.alatus.Gui",
                    zbus::fdo::RequestNameFlags::DoNotQueue.into(),
                )
                .await
            {
                Ok(zbus::fdo::RequestNameReply::PrimaryOwner)
                | Ok(zbus::fdo::RequestNameReply::AlreadyOwner) => {
                    info!("Acquired primary D-Bus name io.strixwolf.alatus.Gui");
                    Some(conn)
                }
                Ok(zbus::fdo::RequestNameReply::Exists)
                | Ok(zbus::fdo::RequestNameReply::InQueue) => {
                    info!("Alatus GUI is already running. Raising active window.");
                    if let Ok(proxy) = zbus::Proxy::new(
                        &conn,
                        "io.strixwolf.alatus.Gui",
                        "/io/strixwolf/alatus/Gui",
                        "io.strixwolf.alatus.Gui",
                    )
                    .await
                    {
                        let _ = proxy.call::<_, _, ()>("ShowWindow", &()).await;
                    }
                    std::process::exit(0);
                }
                Err(e) => {
                    warn!("Failed to request D-Bus name io.strixwolf.alatus.Gui: {e}");
                    Some(conn)
                }
            }
        }
        Err(e) => {
            warn!("Failed to connect to D-Bus session bus: {e}");
            None
        }
    };

    // Unified configuration persistence
    let config = load_config();
    let config_arc = Arc::new(std::sync::Mutex::new(config.clone()));
    let tray_enabled_atomic = Arc::new(AtomicBool::new(config.tray_enabled));
    let (tx, mut rx) = mpsc::unbounded_channel::<GuiAction>();
    let last_user_action = Arc::new(AtomicU64::new(0));

    // Restore persisted hardware configuration on startup
    {
        let startup_cfg = config.clone();
        tokio::spawn(async move {
            if let Ok(client) = get_daemon_client().await {
                let _ = client.set_charge_limit(startup_cfg.charge_limit).await;
                let mode = match startup_cfg.thermal_mode {
                    0 => FirmwareMode::Quiet,
                    1 => FirmwareMode::Balanced,
                    2 => FirmwareMode::High,
                    3 => FirmwareMode::Full,
                    _ => FirmwareMode::Balanced,
                };
                let _ = client.set_firmware_mode(mode).await;
                let _ = client.set_rgb_brightness(startup_cfg.rgb_brightness).await;
                if startup_cfg.rgb_preset == 0 {
                    let _ = set_session_accent_sync(true).await;
                } else if startup_cfg.rgb_preset == -1 {
                    let _ = set_session_accent_sync(false).await;
                    let (r, g, b) = startup_cfg.custom_rgb;
                    let _ = client.set_rgb_color(r, g, b).await;
                }
            }
            let _ = set_session_oled_care(startup_cfg.oled_care_enabled).await;
            let _ = set_session_oled_dim_level(startup_cfg.oled_dim_level).await;
        });
    }

    let initial_rgb = if let Some(ref conn) = session_conn {
        read_portal_accent_color(conn).await.unwrap_or(None)
    } else {
        None
    };
    let initial_palette = derive_m3_palette(initial_rgb);

    let revealed_serial: Arc<std::sync::Mutex<String>> =
        Arc::new(std::sync::Mutex::new(String::new()));

    // Universal FreeDesktop StatusNotifierItem & DBusMenu system tray service
    if let Some(ref conn) = session_conn {
        if let Err(e) = start_tray_service(conn, tray_tx.clone()).await {
            warn!("Failed to initialize system tray service: {e}");
        } else {
            info!("System tray service registered successfully on session bus.");
        }
    } else {
        warn!("Session D-Bus connection unavailable; system tray registration skipped.");
    }

    let tray_action_tx = tx.clone();
    let tray_enabled_tray = Arc::clone(&tray_enabled_atomic);
    let config_tray = Arc::clone(&config_arc);
    let initial_palette_tray = initial_palette;
    let tx_tray = tx.clone();
    tokio::spawn(async move {
        while let Some(evt) = tray_rx.recv().await {
            match evt {
                TrayEvent::ToggleWindow => {
                    let tx = tx_tray.clone();
                    let tray_ref = Arc::clone(&tray_enabled_tray);
                    let cfg_arc = Arc::clone(&config_tray);
                    let palette = initial_palette_tray;
                    let _ = slint::invoke_from_event_loop(move || {
                        APP_WINDOW.with(|cell| {
                            let mut b = cell.borrow_mut();
                            if b.is_none() {
                                match AppWindow::new() {
                                    Ok(w) => {
                                        let cfg =
                                            cfg_arc.lock().map(|c| c.clone()).unwrap_or_default();
                                        setup_window(&w, &tx, tray_ref, &cfg, &palette);
                                        let _ = w.show();
                                        *b = Some(w);
                                    }
                                    Err(e) => error!("Failed to create AppWindow: {e}"),
                                }
                            } else if let Some(ref w) = *b {
                                if w.window().is_visible() {
                                    let _ = w.hide();
                                } else {
                                    w.window().set_minimized(false);
                                    let _ = w.show();
                                }
                            }
                        });
                    });
                }
                TrayEvent::ShowWindow => {
                    let tx = tx_tray.clone();
                    let tray_ref = Arc::clone(&tray_enabled_tray);
                    let cfg_arc = Arc::clone(&config_tray);
                    let palette = initial_palette_tray;
                    let _ = slint::invoke_from_event_loop(move || {
                        APP_WINDOW.with(|cell| {
                            let mut b = cell.borrow_mut();
                            if b.is_none() {
                                match AppWindow::new() {
                                    Ok(w) => {
                                        let cfg =
                                            cfg_arc.lock().map(|c| c.clone()).unwrap_or_default();
                                        setup_window(&w, &tx, tray_ref, &cfg, &palette);
                                        let _ = w.show();
                                        *b = Some(w);
                                    }
                                    Err(e) => error!("Failed to create AppWindow: {e}"),
                                }
                            } else if let Some(ref w) = *b {
                                w.window().set_minimized(false);
                                w.show().unwrap();
                            }
                        });
                    });
                }
                TrayEvent::SetThermalMode(mode) => {
                    let _ = tray_action_tx.send(GuiAction::SetThermalMode(mode as i32));
                }
                TrayEvent::TriggerPixelRefresh => {
                    let _ = tray_action_tx.send(GuiAction::TriggerPixelRefresh);
                }
                TrayEvent::Quit => {
                    let _ = slint::quit_event_loop();
                }
            }
        }
    });

    if !start_minimized {
        let w = AppWindow::new()?;
        let cfg = config_arc.lock().map(|c| c.clone()).unwrap_or_default();
        setup_window(
            &w,
            &tx,
            Arc::clone(&tray_enabled_atomic),
            &cfg,
            &initial_palette,
        );
        w.show()?;
        APP_WINDOW.with(|cell| {
            *cell.borrow_mut() = Some(w);
        });
    } else {
        info!("Alatus GUI running minimized in system tray (no window mapped).");
    }

    // Trigger initial hardware compatibility diagnostics
    let _ = tx.send(GuiAction::RunDiagnostics);

    let last_user_action_processor = last_user_action.clone();
    let tray_enabled_action = tray_enabled_atomic.clone();
    let config_action = Arc::clone(&config_arc);
    let revealed_serial_action = Arc::clone(&revealed_serial);

    // Background task to process GUI action events asynchronously
    tokio::spawn(async move {
        while let Some(action) = rx.recv().await {
            let now_secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            last_user_action_processor.store(now_secs, Ordering::Relaxed);

            match action {
                GuiAction::SetThermalMode(mode_idx) => {
                    let mode = match mode_idx {
                        0 => FirmwareMode::Quiet,
                        1 => FirmwareMode::Balanced,
                        2 => FirmwareMode::High,
                        3 => FirmwareMode::Full,
                        _ => FirmwareMode::Balanced,
                    };
                    info!("GUI Action: Set thermal mode to {mode:?}");
                    let _ = slint::invoke_from_event_loop(move || {
                        with_app_window(|w| {
                            w.set_thermal_mode(mode_idx);
                        });
                    });
                    if let Ok(mut cfg_guard) = config_action.lock() {
                        cfg_guard.thermal_mode = mode_idx as u32;
                        let _ = save_config_atomic(&cfg_guard);
                    }
                    match get_daemon_client().await {
                        Ok(client) => {
                            if let Err(e) = client.set_firmware_mode(mode).await {
                                error!("Failed to set firmware mode on daemon: {e}");
                            } else {
                                info!("Firmware mode updated to {mode:?} successfully.");
                            }
                        }
                        Err(e) => error!("Daemon unavailable for thermal mode: {e}"),
                    }
                }
                GuiAction::ToggleOledCare(enable) => {
                    info!("GUI Action: Toggle OLED care to {enable}");
                    if let Ok(mut cfg_guard) = config_action.lock() {
                        cfg_guard.oled_care_enabled = enable;
                        let _ = save_config_atomic(&cfg_guard);
                    }
                    if let Err(e) = set_session_oled_care(enable).await {
                        warn!("Session daemon OLED care toggle failed: {e}");
                    } else {
                        info!("OLED Care set to {enable} successfully.");
                    }
                }
                GuiAction::TriggerPixelRefresh => {
                    info!("GUI Action: Trigger pixel refresh conditioning cycle");
                    let refresh_res = trigger_session_pixel_refresh()
                        .await
                        .map_err(|e| e.to_string());
                    let success = match refresh_res {
                        Ok(res) => {
                            info!("Session daemon pixel refresh responded: {res}");
                            res
                        }
                        Err(e) => {
                            warn!(
                                "Session daemon D-Bus call failed: {e}. Executing local fallback conditioning cycle."
                            );
                            let mut metrics = load_oled_metrics();
                            metrics.refresh_count += 1;
                            metrics.active_screen_seconds = 0;
                            metrics.last_refresh_timestamp = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            save_oled_metrics(&metrics);
                            if let Ok(conn) = zbus::Connection::session().await {
                                let _ = send_desktop_notification(
                                    &conn,
                                    "ASUS OLED Care: Pixel Refresh Started",
                                    "Conditioning OLED panel to relieve subpixel stress...",
                                )
                                .await;
                            }
                            tokio::spawn(async move {
                                apply_flicker_free_dimming(None, 50).await;
                                tokio::time::sleep(Duration::from_millis(1500)).await;
                                let metrics = load_oled_metrics();
                                apply_flicker_free_dimming(None, metrics.oled_dim_level).await;
                                if let Ok(conn) = zbus::Connection::session().await {
                                    let _ = send_desktop_notification(
                                        &conn,
                                        "ASUS OLED Care: Pixel Refresh Completed",
                                        "Panel conditioning cycle finished successfully.",
                                    )
                                    .await;
                                }
                            });
                            true
                        }
                    };
                    let msg = if success { "Cycle Active" } else { "Failed" };
                    let _ = slint::invoke_from_event_loop(move || {
                        with_app_window(|w| {
                            w.set_refresh_status_text(msg.into());
                        });
                    });
                    tokio::spawn(async move {
                        tokio::time::sleep(Duration::from_secs(4)).await;
                        let _ = slint::invoke_from_event_loop(move || {
                            with_app_window(|w| {
                                w.set_refresh_status_text("".into());
                            });
                        });
                    });
                }
                GuiAction::SetOledDimLevel(level) => {
                    let target = (level as u32).clamp(10, 100);
                    info!("GUI Action: Set Flicker-Free OLED dim level to {target}%");
                    if let Ok(mut cfg_guard) = config_action.lock() {
                        cfg_guard.oled_dim_level = target;
                        let _ = save_config_atomic(&cfg_guard);
                    }
                    let mut metrics = load_oled_metrics();
                    metrics.oled_dim_level = target;
                    save_oled_metrics(&metrics);
                    let set_res = set_session_oled_dim_level(target)
                        .await
                        .map_err(|e| e.to_string());
                    if let Err(e) = set_res {
                        warn!("Session daemon set OLED dim level failed: {e}");
                        apply_flicker_free_dimming(None, target).await;
                    } else {
                        info!("Flicker-Free OLED dim level set to {target}% successfully.");
                    }
                }
                GuiAction::SetRefreshRate(hz) => {
                    info!("GUI Action: Set display refresh rate to {hz} Hz");
                    if let Ok(mut cfg_guard) = config_action.lock() {
                        cfg_guard.refresh_rate = hz as u32;
                        let _ = save_config_atomic(&cfg_guard);
                    }
                    if hz == 0 {
                        if let Err(e) = set_session_auto_refresh(true).await {
                            warn!("Session auto refresh toggle failed: {e}");
                        }
                        if let Ok(client) = get_daemon_client().await
                            && let Ok(on_ac) = client.get_on_ac().await
                        {
                            let target_hz = if on_ac { 120.0 } else { 60.0 };
                            if let Err(e) = set_panel_refresh_rate(target_hz).await {
                                error!("Failed to set panel refresh rate: {e}");
                            } else {
                                info!("Auto refresh switched panel to {target_hz:.1} Hz.");
                            }
                        }
                    } else {
                        if let Err(e) = set_session_auto_refresh(false).await {
                            warn!("Session disable auto refresh failed: {e}");
                        }
                        if let Err(e) = set_panel_refresh_rate(hz as f64).await {
                            error!("Failed to set panel refresh rate to {hz} Hz: {e}");
                        } else {
                            info!("Panel refresh rate set to {hz} Hz successfully.");
                        }
                    }
                }
                GuiAction::SetRgbBrightness(percent) => {
                    let val = percent.clamp(0, 100) as u32;
                    info!("GUI Action: Set RGB brightness to {val}%");
                    if let Ok(mut cfg_guard) = config_action.lock() {
                        cfg_guard.rgb_brightness = val;
                        let _ = save_config_atomic(&cfg_guard);
                    }
                    match get_daemon_client().await {
                        Ok(client) => {
                            if let Err(e) = client.set_rgb_brightness(val).await {
                                error!("Failed to set RGB brightness on daemon: {e}");
                            } else {
                                info!("RGB brightness set to {val}% successfully.");
                            }
                        }
                        Err(e) => error!("Daemon unavailable for RGB brightness: {e}"),
                    }
                }
                GuiAction::SetRgbTimeout(seconds) => {
                    let val = seconds.max(0) as u32;
                    info!("GUI Action: Set RGB timeout to {val}s");
                    if let Ok(mut cfg_guard) = config_action.lock() {
                        cfg_guard.rgb_timeout_seconds = val;
                        let _ = save_config_atomic(&cfg_guard);
                    }
                    match get_daemon_client().await {
                        Ok(client) => {
                            if let Err(e) = client.set_rgb_timeout(val).await {
                                error!("Failed to set RGB timeout on daemon: {e}");
                            } else {
                                info!("RGB timeout set to {val}s successfully.");
                            }
                        }
                        Err(e) => error!("Daemon unavailable for RGB timeout: {e}"),
                    }
                }
                GuiAction::SetRgbTimeoutPolicy(policy_val) => {
                    let (policy_enum, policy_str) = match policy_val {
                        0 => (RgbTimeoutPolicy::Never, "never"),
                        1 => (RgbTimeoutPolicy::BatteryOnly, "battery"),
                        _ => (RgbTimeoutPolicy::Always, "always"),
                    };
                    info!("GUI Action: Set RGB timeout policy to {policy_str}");
                    if let Ok(mut cfg_guard) = config_action.lock() {
                        cfg_guard.rgb_timeout_policy = policy_enum;
                        let _ = save_config_atomic(&cfg_guard);
                    }
                    match get_daemon_client().await {
                        Ok(client) => {
                            if let Err(e) = client.set_rgb_timeout_policy(policy_str).await {
                                error!("Failed to set RGB timeout policy on daemon: {e}");
                            } else {
                                info!("RGB timeout policy set to {policy_str} successfully.");
                            }
                        }
                        Err(e) => error!("Daemon unavailable for RGB timeout policy: {e}"),
                    }
                }
                GuiAction::SetRgbPreset(preset) => {
                    info!("GUI Action: Set RGB preset to {preset}");
                    if let Ok(mut cfg_guard) = config_action.lock() {
                        cfg_guard.rgb_preset = preset;
                        let _ = save_config_atomic(&cfg_guard);
                    }
                    match preset {
                        0 => {
                            if let Err(e) = set_session_accent_sync(true).await {
                                warn!("Failed to enable session accent sync: {e}");
                            } else {
                                info!("Desktop accent synchronization enabled.");
                            }
                            if let Ok(conn) = zbus::Connection::session().await
                                && let Ok(Some((r, g, b))) = read_portal_accent_color(&conn).await
                            {
                                match get_daemon_client().await {
                                    Ok(client) => {
                                        if let Err(e) = client.set_rgb_color(r, g, b).await {
                                            error!("Failed to set Auto-Sync RGB color: {e}");
                                        } else {
                                            info!(
                                                "Auto-Sync RGB color (#{r:02X}{g:02X}{b:02X}) applied successfully."
                                            );
                                        }
                                    }
                                    Err(e) => error!("Daemon unavailable for Auto-Sync RGB: {e}"),
                                }
                            }
                        }
                        1 => {
                            let _ = set_session_accent_sync(false).await;
                            match get_daemon_client().await {
                                Ok(client) => {
                                    if let Err(e) = client.set_rgb_color(61, 174, 233).await {
                                        error!("Failed to set RGB preset Cyan: {e}");
                                    } else {
                                        info!("Preset Cyan applied successfully.");
                                    }
                                }
                                Err(e) => error!("Daemon unavailable for RGB preset Cyan: {e}"),
                            }
                        }
                        2 => {
                            let _ = set_session_accent_sync(false).await;
                            match get_daemon_client().await {
                                Ok(client) => {
                                    if let Err(e) = client.set_rgb_color(186, 104, 200).await {
                                        error!("Failed to set RGB preset Purple: {e}");
                                    } else {
                                        info!("Preset Purple applied successfully.");
                                    }
                                }
                                Err(e) => error!("Daemon unavailable for RGB preset Purple: {e}"),
                            }
                        }
                        3 => {
                            let _ = set_session_accent_sync(false).await;
                            match get_daemon_client().await {
                                Ok(client) => {
                                    if let Err(e) = client.set_rgb_color(255, 255, 255).await {
                                        error!("Failed to set RGB preset White: {e}");
                                    } else {
                                        info!("Preset White applied successfully.");
                                    }
                                }
                                Err(e) => error!("Daemon unavailable for RGB preset White: {e}"),
                            }
                        }
                        4 => {
                            let _ = set_session_accent_sync(false).await;
                            match get_daemon_client().await {
                                Ok(client) => {
                                    if let Err(e) = client.set_rgb_color(255, 140, 0).await {
                                        error!("Failed to set RGB preset Amber: {e}");
                                    } else {
                                        info!("Preset Amber applied successfully.");
                                    }
                                }
                                Err(e) => error!("Daemon unavailable for RGB preset Amber: {e}"),
                            }
                        }
                        _ => {}
                    }
                }
                GuiAction::ApplyCustomRgb(r, g, b) => {
                    info!("GUI Action: Apply custom RGB color ({r}, {g}, {b})");
                    if let Ok(mut cfg_guard) = config_action.lock() {
                        cfg_guard.custom_rgb = (r, g, b);
                        cfg_guard.rgb_preset = -1;
                        let _ = save_config_atomic(&cfg_guard);
                    }
                    let _ = set_session_accent_sync(false).await;
                    match get_daemon_client().await {
                        Ok(client) => {
                            if let Err(e) = client.set_rgb_color(r, g, b).await {
                                error!("Failed to set custom RGB color: {e}");
                            } else {
                                info!("Custom RGB color ({r}, {g}, {b}) applied successfully.");
                            }
                        }
                        Err(e) => error!("Daemon unavailable for custom RGB color: {e}"),
                    }
                }
                GuiAction::SetChargeLimit(limit) => {
                    let limit = limit.clamp(50, 100);
                    info!("GUI Action: Set charge limit to {limit}%");
                    if let Ok(mut cfg_guard) = config_action.lock() {
                        cfg_guard.charge_limit = limit;
                        let _ = save_config_atomic(&cfg_guard);
                    }
                    match get_daemon_client().await {
                        Ok(client) => {
                            if let Err(e) = client.set_charge_limit(limit).await {
                                error!("Failed to set charge limit on daemon: {e}");
                            } else {
                                info!("Charge limit set to {limit}% successfully.");
                            }
                        }
                        Err(e) => error!("Daemon unavailable for charge limit: {e}"),
                    }
                }
                GuiAction::RevealSerial => {
                    let serial_holder = Arc::clone(&revealed_serial_action);
                    tokio::spawn(async move {
                        let output = tokio::process::Command::new("pkexec")
                            .args(["cat", "/sys/class/dmi/id/product_serial"])
                            .output()
                            .await;
                        let serial = match output {
                            Ok(out) if out.status.success() => {
                                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                                if !s.is_empty() {
                                    s
                                } else {
                                    "Unavailable".to_string()
                                }
                            }
                            _ => {
                                if let Ok(client) = DaemonClient::connect().await
                                    && let Ok(s) = client.get_product_serial().await
                                    && !s.is_empty()
                                    && s != "Unavailable"
                                {
                                    s
                                } else {
                                    "Auth Failed".to_string()
                                }
                            }
                        };
                        if let Ok(mut guard) = serial_holder.lock() {
                            *guard = serial.clone();
                        }
                        let serial_disp = serial.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            with_app_window(|w| {
                                w.set_hw_serial(serial_disp.into());
                                w.set_hw_serial_revealed(true);
                            });
                        });
                    });
                }
                GuiAction::CopySerial => {
                    let serial_holder = Arc::clone(&revealed_serial_action);
                    tokio::spawn(async move {
                        let serial_to_copy =
                            serial_holder.lock().map(|g| g.clone()).unwrap_or_default();

                        if !serial_to_copy.is_empty()
                            && serial_to_copy != "••••••••••••"
                            && serial_to_copy != "Auth Failed"
                            && serial_to_copy != "Unavailable"
                        {
                            copy_to_clipboard(&serial_to_copy).await;
                            let _ = slint::invoke_from_event_loop(move || {
                                with_app_window(|w| {
                                    w.set_hw_copy_status("Copied to Clipboard".into());
                                });
                            });
                            tokio::spawn(async move {
                                tokio::time::sleep(Duration::from_millis(2500)).await;
                                let _ = slint::invoke_from_event_loop(move || {
                                    with_app_window(|w| {
                                        w.set_hw_copy_status("".into());
                                    });
                                });
                            });
                        }
                    });
                }
                GuiAction::RunDiagnostics => {
                    tokio::spawn(async move {
                        let _ = slint::invoke_from_event_loop(move || {
                            with_app_window(|w| {
                                w.set_is_diagnosing(true);
                            });
                        });

                        let diags = run_hardware_diagnostics().await;

                        let _ = slint::invoke_from_event_loop(move || {
                            with_app_window(|w| {
                                w.set_diag_wmi_status(diags.wmi.status.into());
                                w.set_diag_wmi_desc(diags.wmi.desc.into());

                                w.set_diag_rgb_status(diags.rgb.status.into());
                                w.set_diag_rgb_desc(diags.rgb.desc.into());

                                w.set_diag_oled_status(diags.oled.status.into());
                                w.set_diag_oled_desc(diags.oled.desc.into());

                                w.set_diag_hotkey_status(diags.hotkey.status.into());
                                w.set_diag_hotkey_desc(diags.hotkey.desc.into());

                                w.set_diag_charge_status(diags.charge.status.into());
                                w.set_diag_charge_desc(diags.charge.desc.into());

                                w.set_is_diagnosing(false);
                                tokio::spawn(sync_capabilities(w.as_weak()));
                            });
                        });
                    });
                }
                GuiAction::ToggleTray(enabled) => {
                    info!("GUI Action: Toggle system tray: {enabled}");
                    tray_enabled_action.store(enabled, Ordering::Relaxed);
                    if let Ok(mut cfg_guard) = config_action.lock() {
                        cfg_guard.tray_enabled = enabled;
                        let _ = save_config_atomic(&cfg_guard);
                    }
                }
                GuiAction::ToggleAutostart(enabled) => {
                    info!("GUI Action: Toggle launch on startup: {enabled}");
                    set_autostart_enabled(enabled);
                    if let Ok(mut cfg_guard) = config_action.lock() {
                        cfg_guard.autostart_enabled = enabled;
                        let _ = save_config_atomic(&cfg_guard);
                    }
                }
            }
        }
    });

    // Periodic telemetry and theme polling loop (1.5 seconds interval)
    let last_user_action_telemetry = last_user_action.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(1500));
        let mut daemon_client_opt = get_daemon_client().await.ok();
        let session_conn_opt = zbus::Connection::session().await.ok();

        loop {
            interval.tick().await;

            // Reconnect daemon client if necessary
            if daemon_client_opt.is_none() {
                daemon_client_opt = get_daemon_client().await.ok();
                if daemon_client_opt.is_some() {
                    let _ = slint::invoke_from_event_loop(|| {
                        with_app_window(|w| {
                            tokio::spawn(sync_capabilities(w.as_weak()));
                        });
                    });
                }
            }

            // Read sensors
            let thermal = read_thermal_telemetry();
            let cpu_temp_str = format!("{}°C", thermal.temp_c);
            let fan_rpm_str = if thermal.fan_rpm > 0 {
                format!("{} RPM", thermal.fan_rpm)
            } else {
                "0 RPM".to_string()
            };

            let mut root_daemon_active = false;
            let mut power_source_str = "AC".to_string();
            let mut thermal_mode_idx_opt = None;
            let mut rgb_brightness_opt = None;
            let mut charge_limit_opt = None;

            if let Some(ref client) = daemon_client_opt {
                root_daemon_active = true;
                if let Ok(on_ac) = client.get_on_ac().await {
                    power_source_str = if on_ac {
                        "AC".to_string()
                    } else {
                        "Battery".to_string()
                    };
                }
                if let Ok(mode) = client.get_firmware_mode().await {
                    thermal_mode_idx_opt = Some(match mode {
                        FirmwareMode::Quiet => 0,
                        FirmwareMode::Balanced => 1,
                        FirmwareMode::High => 2,
                        FirmwareMode::Full => 3,
                        FirmwareMode::Unknown(_) => 1,
                    });
                }
                if let Ok(rgb) = client.get_rgb_status().await {
                    rgb_brightness_opt = Some(rgb.brightness as i32);
                }
                if let Ok(limit) = client.get_charge_limit().await {
                    charge_limit_opt = Some(limit.clamp(50, 100) as i32);
                }
            } else {
                daemon_client_opt = None;
            }

            // Dynamic Desktop Accent Color polling
            let mut dynamic_palette_opt = None;
            if let Some(ref conn) = session_conn_opt
                && let Ok(Some(rgb)) = read_portal_accent_color(conn).await
            {
                dynamic_palette_opt = Some(derive_m3_palette(Some(rgb)));
            }

            let session_status = query_session_status().await;
            let display_info = query_display_info().await;
            let session_daemon_active = is_session_daemon_running().await || session_status.running;

            let refresh_rate_selection = if session_status.auto_refresh {
                0
            } else if display_info.refresh_rate >= 100.0 {
                120
            } else {
                60
            };

            let now_secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let user_recently_active =
                now_secs.saturating_sub(last_user_action_telemetry.load(Ordering::Relaxed)) < 3;

            let bat = get_battery_health_details();
            let _ = slint::invoke_from_event_loop(move || {
                with_app_window(|w| {
                    if !w.window().is_visible() {
                        return;
                    }

                    if let Some(palette) = dynamic_palette_opt {
                        w.set_accent_color(palette.accent_color);
                        w.set_accent_container(palette.accent_container);
                        w.set_on_accent_container(palette.on_accent_container);
                        w.set_on_accent(palette.on_accent);
                    }
                    w.set_cpu_temp(cpu_temp_str.into());
                    w.set_fan_rpm(fan_rpm_str.into());
                    w.set_power_source(power_source_str.into());
                    w.set_active_connector(display_info.connector.into());
                    w.set_refresh_rate(format!("{:.1} Hz", display_info.refresh_rate).into());
                    w.set_oled_screen_hours(
                        format!("{:.1}", session_status.pixel_refresh_hours).into(),
                    );
                    w.set_oled_refresh_count(session_status.pixel_refresh_count as i32);
                    w.set_root_daemon_status(if root_daemon_active {
                        "Active".into()
                    } else {
                        "Inactive".into()
                    });
                    w.set_session_daemon_status(if session_daemon_active {
                        "Active".into()
                    } else {
                        "Inactive".into()
                    });

                    // Keep live battery status & health current
                    w.set_hw_battery_status(bat.status.into());
                    w.set_hw_battery_pct(bat.pct.into());
                    w.set_hw_battery_health(bat.health.into());
                    w.set_hw_battery_cycles(bat.cycles.into());

                    // Only synchronize interactive settings if user is not actively adjusting them
                    if !user_recently_active {
                        if let Some(mode_idx) = thermal_mode_idx_opt {
                            w.set_thermal_mode(mode_idx);
                        }
                        if let Some(rgb_brightness) = rgb_brightness_opt {
                            w.set_rgb_brightness(rgb_brightness);
                        }
                        if let Some(limit) = charge_limit_opt {
                            w.set_charge_limit(limit);
                        }
                        if session_daemon_active {
                            w.set_selected_refresh_rate(refresh_rate_selection);
                            w.set_oled_care_enabled(session_status.oled_care);
                            w.set_oled_dim_level(session_status.oled_dim_level as i32);
                            if session_status.sync_accent {
                                w.set_selected_rgb_preset(0);
                            }
                        }
                    }
                });
            });
        }
    });

    slint::run_event_loop_until_quit()?;
    Ok(())
}

#[cfg(test)]
mod tests;
