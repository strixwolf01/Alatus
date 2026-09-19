// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

pub mod dbus_interface;
pub mod hardware_listener;
pub mod inactivity;
pub mod policy;
pub mod power;
pub mod state;

pub use dbus_interface::*;
pub use hardware_listener::*;
pub use inactivity::*;
pub use policy::*;
pub use power::*;
pub use state::*;

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
    crate::telemetry::init_tracing("alatusd");
    tracing::info!("Starting alatusd");

    wait_for_devices().await;

    // Load authoritative system persistent state
    let mut persistent_state = DaemonPersistentState::load();

    // Fall back to / seed from user config if empty
    let saved_config = crate::services::config::load_config();
    if persistent_state.charge_limit.is_none() {
        persistent_state.charge_limit = Some(saved_config.charge_limit);
    }
    if persistent_state.thermal_mode.is_none()
        && let Ok(mode) = crate::domain::ThermalMode::try_from(saved_config.thermal_mode)
    {
        persistent_state.thermal_mode = Some(mode);
    }
    if persistent_state.rgb_brightness.is_none() {
        persistent_state.rgb_brightness = Some(saved_config.rgb_brightness as u8);
    }
    if persistent_state.rgb_color.is_none() {
        persistent_state.rgb_color = Some(crate::domain::ColorRgb::new(
            saved_config.custom_rgb.0,
            saved_config.custom_rgb.1,
            saved_config.custom_rgb.2,
        ));
    }
    if persistent_state.rgb_timeout_seconds.is_none() {
        persistent_state.rgb_timeout_seconds = Some(saved_config.rgb_timeout_seconds);
    }
    if persistent_state.rgb_timeout_policy.is_none() {
        persistent_state.rgb_timeout_policy = Some(saved_config.rgb_timeout_policy.to_string());
    }
    // Save to disk to ensure /var/lib/alatus/state.json is seeded
    let _ = persistent_state.save_atomic();

    let initial_charge_limit = persistent_state.charge_limit;
    let initial_thermal_mode_enum = persistent_state.thermal_mode;
    let initial_firmware_mode = initial_thermal_mode_enum.map(|m| m.as_u32());
    let initial_on_ac = check_is_on_ac();
    let initial_rgb_timeout = persistent_state
        .rgb_timeout_seconds
        .unwrap_or(saved_config.rgb_timeout_seconds);
    let initial_rgb_policy = persistent_state
        .rgb_timeout_policy
        .as_deref()
        .and_then(|p| p.parse().ok())
        .unwrap_or(saved_config.rgb_timeout_policy);

    let device_context = Arc::new(Mutex::new(crate::hardware::DeviceContext::new()));
    {
        let ctx = device_context.lock().await;
        tracing::info!(
            "Initialized DeviceContext with profile: {} ({})",
            ctx.profile.device.name,
            ctx.profile.device.vendor
        );
        tracing::info!("Probed system capabilities: {:?}", ctx.capabilities);
    }

    let policy_engine = Arc::new(Mutex::new(PolicyEngine::new()));
    {
        let mut engine = policy_engine.lock().await;
        engine.manual_thermal_override = persistent_state.manual_thermal_override;
        if let Some(mode) = initial_thermal_mode_enum {
            engine.last_applied_thermal_mode = Some(mode);
            engine.current_thermal_mode = Some(mode);
        }
    }

    // Startup State Reconciliation: query current hardware status into snapshot
    let initial_battery_level =
        crate::services::telemetry::read_battery_telemetry().map(|t| t.capacity as u8);
    let current_hw_mode = {
        let ctx = device_context.lock().await;
        ctx.thermal.as_ref().and_then(|th| th.get_mode().ok())
    };
    let startup_snapshot =
        HardwareSnapshot::new(initial_on_ac, initial_battery_level, current_hw_mode);

    let startup_decisions = {
        let mut engine = policy_engine.lock().await;
        engine.reconcile_startup(startup_snapshot)
    };

    tracing::info!(
        "Startup reconciliation evaluated {} decisions",
        startup_decisions.len()
    );
    {
        let mut ctx = device_context.lock().await;
        for decision in startup_decisions {
            SafetyGate::dispatch(&decision, &mut ctx);
        }
    }

    // Apply saved configurations immediately to hardware
    if let Some(limit) = initial_charge_limit {
        tracing::info!("Applying startup battery charge limit: {limit}%");
        let threshold = if limit == 0 {
            crate::domain::ChargeThreshold::new(100).ok()
        } else {
            crate::domain::ChargeThreshold::new(limit as u8).ok()
        };
        if let Some(threshold) = threshold {
            let mut ctx = device_context.lock().await;
            if let Some(ref mut bat) = ctx.battery {
                let _ = bat.set_charge_threshold(threshold);
            }
        }
    }
    if let Some(mode) = initial_thermal_mode_enum {
        tracing::info!("Applying startup firmware mode: {mode:?}");
        let mut ctx = device_context.lock().await;
        if let Some(ref mut th) = ctx.thermal {
            let _ = th.set_mode(mode);
        }
    }

    // Apply saved platform power attributes (PPT limits)
    if !persistent_state.ppt_limits.is_empty() {
        let mut ctx = device_context.lock().await;
        if let Some(ref mut plat) = ctx.platform {
            for (attr, val) in &persistent_state.ppt_limits {
                if let Err(e) = plat.set_attribute(attr, *val) {
                    tracing::warn!("Failed to apply startup PPT attribute '{attr}': {e}");
                } else {
                    tracing::info!("Applied startup PPT attribute '{attr}'={val}");
                }
            }
        }
    }

    // Apply saved panel overdrive
    if let Some(od) = persistent_state.panel_overdrive {
        let mut ctx = device_context.lock().await;
        if let Some(ref mut plat) = ctx.platform {
            if let Err(e) = plat.set_panel_od(od) {
                tracing::warn!("Failed to apply startup panel overdrive: {e}");
            } else {
                tracing::info!("Applied startup panel overdrive={od}");
            }
        }
    }

    // Apply saved GPU MUX mode
    if let Some(mux) = persistent_state.gpu_mux_mode {
        let mut ctx = device_context.lock().await;
        if let Some(ref mut plat) = ctx.platform {
            if let Err(e) = plat.set_gpu_mux_mode(mux as u32) {
                tracing::warn!("Failed to apply startup GPU MUX mode: {e}");
            } else {
                tracing::info!("Applied startup GPU MUX mode={mux}");
            }
        }
    }

    let rgb_service = Arc::new(crate::services::rgb::RgbService::new());
    if let Some(bri) = persistent_state.rgb_brightness {
        tracing::info!("Applying startup RGB brightness: {bri}%");
        let _ = rgb_service.set_brightness(bri as u32);
        let mut ctx = device_context.lock().await;
        if let Some(ref mut rgb) = ctx.rgb
            && let Ok(b_pct) = crate::domain::BrightnessPercent::new(bri)
        {
            let _ = rgb.set_brightness(b_pct);
        }
    }
    if let Some(color) = persistent_state.rgb_color {
        tracing::info!("Applying startup RGB color: {:?}", color);
        let _ = rgb_service.set_color(color.r, color.g, color.b);
        let mut ctx = device_context.lock().await;
        if let Some(ref mut rgb) = ctx.rgb {
            let _ = rgb.set_color(color);
        }
    }

    let inactivity = Arc::new(InactivityState::new(
        initial_rgb_timeout,
        initial_rgb_policy,
    ));

    let auto_thermal_profile = !persistent_state.manual_thermal_override;

    let state = Arc::new(Mutex::new(DaemonState::new(
        Arc::clone(&device_context),
        Arc::clone(&policy_engine),
        initial_charge_limit,
        initial_firmware_mode,
        initial_on_ac,
        auto_thermal_profile,
        0,
        1,
        persistent_state,
    )));

    let interface = DaemonInterface::new(
        Arc::clone(&state),
        Arc::clone(&device_context),
        Arc::clone(&policy_engine),
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

    #[test]
    fn test_daemon_dbus_error_names() {
        use zbus::DBusError;
        let err_unavail = DaemonError::CapabilityUnavailable("Missing kernel node".to_string());
        assert_eq!(
            err_unavail.name().as_str(),
            "io.strixwolf.alatus.Error.CapabilityUnavailable"
        );

        let err_unsupp = DaemonError::CapabilityUnsupported("Hardware absent".to_string());
        assert_eq!(
            err_unsupp.name().as_str(),
            "io.strixwolf.alatus.Error.CapabilityUnsupported"
        );
    }
}
