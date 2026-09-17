// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

pub mod dbus_interface;
pub mod hardware_listener;
pub mod inactivity;
pub mod power;

pub use dbus_interface::*;
pub use hardware_listener::*;
pub use inactivity::*;
pub use power::*;

use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use zbus::Connection;

pub async fn wait_for_devices() {
    tracing::info!("Waiting for core system hardware interfaces to stabilize...");
    for attempt in 1..=20 {
        let has_power = Path::new("/sys/class/power_supply").exists();
        let has_wmi = Path::new("/sys/kernel/debug/asus-nb-wmi/dev_id").exists()
            || Path::new("/sys/devices/platform/asus-nb-wmi").exists();
        if has_power && has_wmi {
            tracing::info!("Core hardware interfaces available after {attempt} attempts");
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
}

pub async fn run_daemon() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting alatusd");

    wait_for_devices().await;

    // Load saved user configuration to re-apply on startup
    let saved_config = crate::services::config::load_config();

    let initial_charge_limit = Some(saved_config.charge_limit);
    let initial_firmware_mode = Some(saved_config.thermal_mode);
    let initial_on_ac = check_is_on_ac();
    let initial_rgb_timeout = saved_config.rgb_timeout_seconds;

    // Apply saved configurations immediately to hardware
    if let Some(limit) = initial_charge_limit {
        tracing::info!("Applying startup battery charge limit: {limit}%");
        let _ = write_charge_limit(limit);
    }
    if let Some(mode) = initial_firmware_mode {
        tracing::info!("Applying startup firmware mode: {mode}");
        let _ = write_wmi_firmware_mode(mode);
    }

    let state = Arc::new(Mutex::new(DaemonState::new(
        initial_charge_limit,
        initial_firmware_mode,
        initial_on_ac,
        false,
        0,
        1,
    )));

    let rgb_service = Arc::new(crate::services::rgb::RgbService::new());

    // Restore saved RGB brightness and color on startup
    tracing::info!(
        "Applying startup RGB backlight: brightness={}%, color=({},{},{})",
        saved_config.rgb_brightness,
        saved_config.custom_rgb.0,
        saved_config.custom_rgb.1,
        saved_config.custom_rgb.2
    );
    let _ = rgb_service.set_brightness(saved_config.rgb_brightness);
    let _ = rgb_service.set_color(
        saved_config.custom_rgb.0,
        saved_config.custom_rgb.1,
        saved_config.custom_rgb.2,
    );

    let inactivity = Arc::new(InactivityState::new(
        initial_rgb_timeout,
        saved_config.rgb_timeout_policy,
    ));

    let interface = DaemonInterface::new(
        Arc::clone(&state),
        Arc::clone(&rgb_service),
        Arc::clone(&inactivity),
    );

    let connection = Connection::system().await?;

    connection
        .object_server()
        .at("/io/strixwolf/alatus/Daemon", interface)
        .await?;

    connection
        .request_name("io.strixwolf.alatus.Daemon")
        .await?;

    tracing::info!("Registered D-Bus interface and requested name io.strixwolf.alatus.Daemon");

    let conn_clone = connection.clone();
    let state_clone = Arc::clone(&state);
    let rgb_service_clone = Arc::clone(&rgb_service);
    let inactivity_clone = Arc::clone(&inactivity);
    tokio::spawn(async move {
        if let Err(e) =
            run_background_listener(conn_clone, state_clone, rgb_service_clone, inactivity_clone)
                .await
        {
            tracing::error!("Background listener ended with error: {e}");
        }
    });

    let conn_hotkey = connection.clone();
    let state_hotkey = Arc::clone(&state);
    let rgb_hotkey = Arc::clone(&rgb_service);
    let inactivity_hotkey = Arc::clone(&inactivity);
    let handle = tokio::runtime::Handle::current();
    std::thread::spawn(move || {
        run_asus_hotkey_listener(
            conn_hotkey,
            state_hotkey,
            rgb_hotkey,
            inactivity_hotkey,
            handle,
        );
    });

    // Run forever
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_dbus_value_to_mode() {
        assert!(map_dbus_value_to_mode(0).is_ok());
        assert!(map_dbus_value_to_mode(1).is_ok());
        assert!(map_dbus_value_to_mode(2).is_ok());
        assert!(map_dbus_value_to_mode(3).is_ok());
        assert!(map_dbus_value_to_mode(4).is_err());
        assert!(map_dbus_value_to_mode(u32::MAX).is_err());
    }

    #[test]
    fn test_charge_limit_validation() {
        assert_eq!(validate_charge_limit(0).unwrap(), 0);
        assert_eq!(validate_charge_limit(60).unwrap(), 60);
        assert_eq!(validate_charge_limit(80).unwrap(), 80);
        assert_eq!(validate_charge_limit(100).unwrap(), 100);
        assert!(matches!(
            validate_charge_limit(101),
            Err(DaemonError::InvalidArgument(_))
        ));
        assert!(matches!(
            validate_charge_limit(u32::MAX),
            Err(DaemonError::InvalidArgument(_))
        ));
    }

    #[test]
    fn test_deep_sleep_parsing() {
        assert!(parse_mem_sleep_active("s2idle [deep]"));
        assert!(!parse_mem_sleep_active("[s2idle] deep"));
        assert!(!parse_mem_sleep_active("s2idle"));

        assert!(is_mem_sleep_mode_supported("s2idle [deep]", "deep"));
        assert!(is_mem_sleep_mode_supported("s2idle [deep]", "s2idle"));
        assert!(is_mem_sleep_mode_supported("[s2idle] deep", "deep"));
        assert!(is_mem_sleep_mode_supported("[s2idle] deep", "s2idle"));
        assert!(!is_mem_sleep_mode_supported("s2idle", "deep"));
    }

    #[test]
    fn test_polkit_authorization_denied() {
        let res = parse_polkit_result(false);
        assert!(matches!(res, Err(DaemonError::PermissionDenied(_))));
        let fdo: zbus::fdo::Error = res.unwrap_err().into();
        assert!(matches!(fdo, zbus::fdo::Error::Failed(msg) if msg.contains("authorization")));
    }

    #[test]
    fn test_polkit_authorization_allowed() {
        let res = parse_polkit_result(true);
        assert!(res.is_ok());
    }
}
