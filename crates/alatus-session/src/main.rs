mod gestures;
mod notifier;
mod tray;

use alatus_ipc::{BatteryProxy, LightingProxy, ThermalProxy};
use futures_util::StreamExt;
use notifier::DesktopNotifier;
use std::error::Error;
use std::sync::Arc;
use zbus::Connection;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting alatus-session user agent v0.1.0...");

    // 1. Connect to session bus for notifications and desktop features
    let session_conn = match Connection::session().await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to connect to user session D-Bus: {e}");
            return Err(e.into());
        }
    };

    // 2. Spawn StatusNotifierItem tray with rich menu
    let tray_handle = tray::spawn_tray();

    // 3. Spawn Touchpad Edge Gestures service
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    tokio::spawn(gestures::run_gestures_listener(shutdown_rx));

    // 4. Connect to system bus to monitor alatusd
    let system_conn = match Connection::system().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to connect to system bus: {e}. Daemon might not be running.");
            return Err(e.into());
        }
    };

    let notifier = Arc::new(DesktopNotifier::new(session_conn.clone()));

    // 5. Monitor Thermal Profile changes
    let thermal_proxy = match ThermalProxy::new(&system_conn).await {
        Ok(p) => Some(p),
        Err(e) => {
            tracing::warn!("Thermal service not available on daemon: {e}");
            None
        }
    };

    if let Some(ref proxy) = thermal_proxy {
        // Query initial profile
        if let Ok(profile) = proxy.get_current_profile().await {
            let p_str = profile.clone();
            tray_handle.update(move |tray| {
                tray.current_profile = p_str;
            });
        }

        let notifier_clone = notifier.clone();
        let tray_clone = tray_handle.clone();
        let proxy_stream = proxy.clone();
        tokio::spawn(async move {
            if let Ok(mut stream) = proxy_stream.receive_profile_changed().await {
                tracing::info!("Listening for thermal profile signals...");
                while let Some(signal) = stream.next().await {
                    if let Ok(args) = signal.args() {
                        tracing::info!("Thermal profile changed: {}", args.new_profile);
                        let p = args.new_profile.clone();
                        tray_clone.update(move |t| {
                            t.current_profile = p;
                        });
                        let _ = notifier_clone
                            .notify(
                                "Thermal Profile",
                                &format!("Mode: {}", args.new_profile),
                                "preferences-system-power",
                                2000,
                            )
                            .await;
                    }
                }
            }
        });
    }

    // 6. Monitor Battery Limit changes
    let battery_proxy = match BatteryProxy::new(&system_conn).await {
        Ok(p) => Some(p),
        Err(e) => {
            tracing::warn!("Battery service not available on daemon: {e}");
            None
        }
    };

    if let Some(ref proxy) = battery_proxy {
        // Query initial battery limit
        if let Ok(info) = proxy.get_info().await {
            if let Some(limit) = info.charge_limit {
                tray_handle.update(move |tray| {
                    tray.current_limit = limit as u32;
                });
            }
        }

        let notifier_clone = notifier.clone();
        let tray_clone = tray_handle.clone();
        let proxy_stream = proxy.clone();
        tokio::spawn(async move {
            if let Ok(mut stream) = proxy_stream.receive_limit_changed().await {
                tracing::info!("Listening for battery limit signals...");
                while let Some(signal) = stream.next().await {
                    if let Ok(args) = signal.args() {
                        tracing::info!("Battery charge limit changed: {}%", args.new_limit);
                        let lim = args.new_limit as u32;
                        tray_clone.update(move |t| {
                            t.current_limit = lim;
                        });
                        let _ = notifier_clone
                            .notify(
                                "Battery Care",
                                &format!("Charge limit set to {}%", args.new_limit),
                                "battery",
                                2000,
                            )
                            .await;
                    }
                }
            }
        });
    }

    // 7. Monitor Lighting state changes
    let lighting_proxy = match LightingProxy::new(&system_conn).await {
        Ok(p) => Some(p),
        Err(e) => {
            tracing::warn!("Lighting service not available on daemon: {e}");
            None
        }
    };

    if let Some(proxy) = lighting_proxy {
        let notifier_clone = notifier.clone();
        tokio::spawn(async move {
            if let Ok(mut stream) = proxy.receive_state_changed().await {
                tracing::info!("Listening for lighting state signals...");
                while let Some(signal) = stream.next().await {
                    if let Ok(args) = signal.args() {
                        tracing::info!("Lighting state changed: mode={}", args.state.mode);
                        let _ = notifier_clone
                            .notify(
                                "Keyboard Backlight",
                                &format!(
                                    "Mode: {} (Brightness: {}/3)",
                                    args.state.mode, args.state.brightness
                                ),
                                "keyboard-brightness",
                                1500,
                            )
                            .await;
                    }
                }
            }
        });
    }

    tracing::info!("alatus-session is active and running.");
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received SIGINT, shutting down alatus-session...");
        }
        _ = sigterm.recv() => {
            tracing::info!("Received SIGTERM, shutting down alatus-session...");
        }
    }
    let _ = shutdown_tx.send(true);
    Ok(())
}
