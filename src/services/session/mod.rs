// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! User-space desktop session integration agent.
//!
//! Provides event-driven synchronization with periodic reconciliation between the active desktop
//! environment (KDE Plasma & GNOME) and Alatus hardware controllers:
//! 1. XDG Desktop Portal Accent Color -> Keyboard RGB Backlight.
//! 2. Power Source (AC vs Battery) -> Display Panel Refresh Rate (e.g. 120Hz vs 60Hz).
//! 3. OLED Care -> usage tracking, idle dimming, subpixel stress conditioning.

pub mod accent;
pub mod dbus;
pub mod desktop;
pub mod display_care;
pub mod listeners;
pub mod notifications;
pub mod state;

pub use accent::*;
pub use dbus::*;
pub use desktop::*;
pub use display_care::*;
pub use listeners::*;
pub use notifications::*;
pub use state::*;

use std::sync::atomic::Ordering;
use std::sync::Arc;
use futures_util::StreamExt;
use zbus::Connection;

use crate::services::daemon_client::DaemonClient;

#[derive(Debug, Clone)]
pub struct DesktopSessionOptions {
    pub sync_accent: bool,
    pub auto_refresh: bool,
    pub oled_care: bool,
}

pub async fn run_desktop_session(
    options: DesktopSessionOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = Arc::new(SessionState::new(
        options.sync_accent,
        options.auto_refresh,
        options.oled_care,
    ));

    let daemon_client =
        DaemonClient::connect_with_retry(30, std::time::Duration::from_millis(500)).await?;

    // Synchronize configured RGB inactivity timeout and policy to daemon
    let config = crate::services::config::load_config();
    let _ = daemon_client
        .set_rgb_timeout(config.rgb_timeout_seconds)
        .await;
    let _ = daemon_client
        .set_rgb_timeout_policy(&config.rgb_timeout_policy.to_string())
        .await;
    let session_conn = Connection::session().await?;
    let system_conn = daemon_client.connection();

    let desktop = detect_desktop();
    let kde_internal_connector = if desktop == DesktopEnv::Kde {
        find_kde_internal_connector().await
    } else {
        None
    };

    // Register Session D-Bus interface for live status, dynamic toggling, and pixel refresh
    let iface = SessionDbusInterface {
        state: Arc::clone(&state),
        session_conn: session_conn.clone(),
        desktop,
        internal_connector: kde_internal_connector.clone(),
    };
    let _ = session_conn
        .object_server()
        .at(SESSION_OBJECT_PATH, iface)
        .await;

    let _ = session_conn.request_name(SESSION_BUS_NAME).await;

    // Spawn kernel netlink uevent watcher for instant power changes
    let mut uevent_rx = crate::services::power_uevent::spawn_power_uevent_listener();
    let power_monitor_active = uevent_rx.is_some();
    state
        .power_monitor_active
        .store(power_monitor_active, Ordering::Relaxed);

    tracing::info!("Starting Alatus desktop session agent");
    tracing::info!("[Power Monitor: ACTIVE (Live uevent + D-Bus)]");
    tracing::info!(
        "[Panel Refresh Switching: {}]",
        if state.auto_refresh.load(Ordering::Relaxed) {
            "ACTIVE"
        } else {
            "DISABLED"
        }
    );
    tracing::info!(
        "[Desktop Accent Sync: {}]",
        if state.sync_accent.load(Ordering::Relaxed) {
            "ACTIVE"
        } else {
            "DISABLED"
        }
    );
    tracing::info!(
        "[OLED Care: {}]",
        if state.oled_care.load(Ordering::Relaxed) {
            "ACTIVE"
        } else {
            "DISABLED"
        }
    );

    // 1. Initial Accent Color Sync
    if state.sync_accent.load(Ordering::Relaxed) {
        let (r, g, b) = match read_portal_accent_color(&session_conn).await {
            Ok(Some((r, g, b))) => {
                tracing::info!("Initial XDG accent color: #{r:02X}{g:02X}{b:02X}");
                (r, g, b)
            }
            Ok(None) => {
                tracing::info!(
                    "No accent color returned by XDG Desktop Portal; falling back to ASUS Cyan (#00D2FF)"
                );
                (0x00, 0xD2, 0xFF)
            }
            Err(e) => {
                tracing::warn!(
                    "XDG Desktop Portal Settings read failed ({e}); falling back to ASUS Cyan (#00D2FF)"
                );
                (0x00, 0xD2, 0xFF)
            }
        };
        if let Err(e) = daemon_client.set_rgb_color(r, g, b).await {
            tracing::warn!("Failed to set initial RGB color via daemon: {e}");
        }
    }

    // 2. Initial Panel Refresh Rate Sync
    let mut last_known_on_ac = daemon_client.get_on_ac().await.unwrap_or(true);
    if state.auto_refresh.load(Ordering::Relaxed) {
        tracing::info!("Initial power state: OnAC={last_known_on_ac}");
        apply_panel_refresh_rate(last_known_on_ac).await;
    }

    // 3. Setup Match Streams
    let mut stream_portal = create_portal_stream(&session_conn).await?;
    let mut stream_power = create_power_stream(system_conn).await?;
    let mut stream_thermal_osd = create_thermal_osd_stream(system_conn).await?;
    let (mut stream_screensaver_fdo, mut stream_screensaver_gnome) =
        create_screensaver_streams(&session_conn).await?;

    tracing::info!("Desktop session agent initialized and listening for events");

    let mut usage_timer = tokio::time::interval(tokio::time::Duration::from_secs(60));
    usage_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut last_notified_threshold_hours: u32 = 0;

    loop {
        tokio::select! {
            _ = usage_timer.tick() => {
                if !state.is_dimmed.load(Ordering::SeqCst) {
                    let current_secs = state.active_screen_seconds.fetch_add(60, Ordering::Relaxed) + 60;
                    let hours = (current_secs / 3600) as u32;

                    // Persist usage metrics periodically every 10 minutes
                    if current_secs.is_multiple_of(600) {
                        save_oled_metrics(&OledMetrics {
                            active_screen_seconds: current_secs,
                            refresh_count: state.pixel_refresh_count.load(Ordering::Relaxed),
                            last_refresh_timestamp: get_epoch_seconds(),
                            oled_dim_level: state.oled_dim_level.load(Ordering::Relaxed),
                        });
                    }

                    // Alert user after 4+ hours of active screen time
                    if hours >= 4 && hours > last_notified_threshold_hours && state.oled_care.load(Ordering::Relaxed) {
                        last_notified_threshold_hours = hours;
                        let summary = "ASUS OLED Care: Pixel Refresh Recommended";
                        let body = format!(
                            "Your display has been active for {hours} hours. Run 'alatus session pixel-refresh' or take a break to relieve OLED pixel stress."
                        );
                        let _ = send_desktop_notification(&session_conn, summary, &body).await;
                    }
                }
            }
            Some(msg_res) = stream_portal.next() => {
                if !state.sync_accent.load(Ordering::Relaxed) {
                    continue;
                }
                let Ok(msg) = msg_res else { continue };
                let Ok((namespace, key, val)) = msg.body().deserialize::<(String, String, zbus::zvariant::OwnedValue)>() else {
                    continue;
                };

                if namespace == "org.freedesktop.appearance"
                    && key == "accent-color"
                    && let Some((r, g, b)) = parse_accent_color(&val)
                {
                    tracing::info!("Accent color updated: #{r:02X}{g:02X}{b:02X}");
                    if let Err(e) = daemon_client.set_rgb_color(r, g, b).await {
                        tracing::warn!("Failed to set RGB color via daemon: {e}");
                    }
                }
            }
            Some(msg_res) = stream_power.next() => {
                let Ok(msg) = msg_res else { continue };
                if let Ok(on_ac) = msg.body().deserialize::<bool>() {
                    last_known_on_ac = on_ac;
                    if state.auto_refresh.load(Ordering::Relaxed) {
                        tracing::info!("Received PowerSourceChanged D-Bus signal: OnAC={on_ac}");
                        apply_panel_refresh_rate(on_ac).await;
                    }
                }
            }
            Some(msg_res) = stream_thermal_osd.next() => {
                let Ok(msg) = msg_res else { continue };
                let Ok((mode,)) = msg.body().deserialize::<(u32,)>() else {
                    continue;
                };
                let (summary, body, icon) = thermal_mode_notification(mode);
                let _ = send_osd_notification(&session_conn, summary, body, &icon).await;
            }
            Some(msg_res) = stream_screensaver_fdo.next() => {
                let Ok(msg) = msg_res else { continue };
                if let Ok(is_idle) = msg.body().deserialize::<bool>() {
                    tracing::info!("ScreenSaver ActiveChanged (FDO) signal received: is_idle={is_idle}");
                    apply_oled_care_dimming(
                        &session_conn,
                        &daemon_client,
                        &state,
                        desktop,
                        kde_internal_connector.as_deref(),
                        is_idle,
                    ).await;
                    if !is_idle {
                        let _ = daemon_client.wake_rgb().await;
                    }
                }
            }
            Some(msg_res) = stream_screensaver_gnome.next() => {
                let Ok(msg) = msg_res else { continue };
                if let Ok(is_idle) = msg.body().deserialize::<bool>() {
                    tracing::info!("ScreenSaver ActiveChanged (GNOME) signal received: is_idle={is_idle}");
                    apply_oled_care_dimming(
                        &session_conn,
                        &daemon_client,
                        &state,
                        desktop,
                        kde_internal_connector.as_deref(),
                        is_idle,
                    ).await;
                    if !is_idle {
                        let _ = daemon_client.wake_rgb().await;
                    }
                }
            }
            Some(()) = async {
                match uevent_rx.as_mut() {
                    Some(rx) => rx.recv().await,
                    None => futures_util::future::pending().await,
                }
            } => {
                if let Ok(current_on_ac) = daemon_client.get_on_ac().await
                    && current_on_ac != last_known_on_ac
                {
                    last_known_on_ac = current_on_ac;
                    if state.auto_refresh.load(Ordering::Relaxed) {
                        tracing::info!(
                            "Kernel uevent power transition detected: OnAC={current_on_ac}"
                        );
                        apply_panel_refresh_rate(current_on_ac).await;
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Received exit signal. Terminating desktop session agent.");
                break;
            }
        }
    }

    Ok(())
}
