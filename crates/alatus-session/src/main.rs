mod notifier;
mod tray;

use alatus_ipc::{BatteryProxy, LightingProxy, ThermalProxy};
use futures_util::StreamExt;
use notifier::DesktopNotifier;
use std::error::Error;
use std::sync::Arc;
use tray::{register_tray_watcher, StatusNotifierItemService, TRAY_OBJECT_PATH};
use zbus::connection::Builder;
use zbus::Connection;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting alatus-session user agent v0.1.0...");

    // 1. Connect to session bus for notifications and tray
    let session_conn = match Connection::session().await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to connect to user session D-Bus: {e}");
            return Err(e.into());
        }
    };

    // 2. Set up StatusNotifierItem tray
    let tray_service = Arc::new(StatusNotifierItemService::new());
    let _tray_conn = Builder::session()?
        .name("org.kde.StatusNotifierItem-alatus")?
        .serve_at(TRAY_OBJECT_PATH, StatusNotifierItemService::new())?
        .build()
        .await?;

    let _ = register_tray_watcher(&session_conn).await;

    // 3. Connect to system bus to monitor alatusd
    let system_conn = match Connection::system().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to connect to system bus: {e}. Waiting for daemon...");
            return Err(e.into());
        }
    };

    let notifier = Arc::new(DesktopNotifier::new(session_conn.clone()));

    // 4. Monitor Thermal Profile changes
    let thermal_proxy = match ThermalProxy::new(&system_conn).await {
        Ok(p) => Some(p),
        Err(e) => {
            tracing::warn!("Thermal service not available on daemon: {e}");
            None
        }
    };

    if let Some(proxy) = thermal_proxy {
        let notifier_clone = notifier.clone();
        let tray_clone = tray_service.clone();
        tokio::spawn(async move {
            if let Ok(mut stream) = proxy.receive_profile_changed().await {
                tracing::info!("Listening for thermal profile signals...");
                while let Some(signal) = stream.next().await {
                    if let Ok(args) = signal.args() {
                        tracing::info!("Thermal profile changed: {}", args.new_profile);
                        tray_clone.set_profile(&args.new_profile);
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

    // 5. Monitor Battery Limit changes
    let battery_proxy = match BatteryProxy::new(&system_conn).await {
        Ok(p) => Some(p),
        Err(e) => {
            tracing::warn!("Battery service not available on daemon: {e}");
            None
        }
    };

    if let Some(proxy) = battery_proxy {
        let notifier_clone = notifier.clone();
        tokio::spawn(async move {
            if let Ok(mut stream) = proxy.receive_limit_changed().await {
                tracing::info!("Listening for battery limit signals...");
                while let Some(signal) = stream.next().await {
                    if let Ok(args) = signal.args() {
                        tracing::info!("Battery charge limit changed: {}%", args.new_limit);
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

    // 6. Monitor Lighting state changes
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
    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down alatus-session...");
    Ok(())
}
