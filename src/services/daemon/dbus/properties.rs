// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Property handlers for the Alatus root daemon D-Bus interface.

use tokio::sync::Mutex;
use zbus::Connection;

use super::super::error::DaemonError;
use super::super::policy::PolicyEngine;
use super::super::polkit::check_polkit;
use super::super::power::{
    check_conflicting_power_daemons_sync, read_deep_sleep, read_product_serial, write_deep_sleep,
};
use super::{DaemonInterface, DaemonState};
use crate::hardware::capabilities::CapabilityState;
use crate::hardware::DeviceContext;

pub async fn get_firmware_mode(
    device_context: &Mutex<DeviceContext>,
) -> zbus::fdo::Result<u32> {
    let ctx = device_context.lock().await;
    match &ctx.capabilities.thermal {
        CapabilityState::Unsupported => Err(DaemonError::CapabilityUnsupported(
            "Thermal mode control unsupported on this device".to_string(),
        )
        .into()),
        CapabilityState::Unavailable(reason) => Err(DaemonError::CapabilityUnavailable(
            format!("Thermal driver unavailable: {reason}"),
        )
        .into()),
        CapabilityState::Supported(_) => {
            if let Some(ref th) = ctx.thermal {
                let mode = th
                    .get_mode()
                    .map_err(|e| DaemonError::IoError(e.to_string()))?;
                Ok(mode.as_u32())
            } else {
                Err(DaemonError::CapabilityUnavailable("Thermal driver missing".to_string()).into())
            }
        }
    }
}

pub async fn set_firmware_mode(
    device_context: &Mutex<DeviceContext>,
    state: &Mutex<DaemonState>,
    policy_engine: &Mutex<PolicyEngine>,
    header: Option<zbus::message::Header<'_>>,
    conn: &Connection,
    value: u32,
) -> zbus::Result<()> {
    let thermal_mode = crate::domain::ThermalMode::try_from(value).map_err(|_| {
        zbus::Error::from(DaemonError::InvalidArgument(
            "Firmware mode must be 0..=3".to_string(),
        ))
    })?;

    let sender = header
        .and_then(|h| h.sender().map(|s| s.to_owned()))
        .ok_or_else(|| {
            zbus::Error::from(DaemonError::PermissionDenied("No sender found".to_string()))
        })?;

    check_polkit(conn, &sender, "io.strixwolf.alatus.set-firmware-mode").await?;

    {
        let mut ctx = device_context.lock().await;
        match &ctx.capabilities.thermal {
            CapabilityState::Unsupported => {
                return Err(zbus::Error::from(DaemonError::CapabilityUnsupported(
                    "Thermal mode control unsupported on this device".to_string(),
                )));
            }
            CapabilityState::Unavailable(reason) => {
                return Err(zbus::Error::from(DaemonError::CapabilityUnavailable(
                    format!("Thermal driver unavailable: {reason}"),
                )));
            }
            CapabilityState::Supported(_) => {
                if let Some(ref mut th) = ctx.thermal {
                    th.set_mode(thermal_mode)
                        .map_err(|e| DaemonError::IoError(e.to_string()))?;
                } else {
                    return Err(zbus::Error::from(DaemonError::CapabilityUnavailable(
                        "Thermal driver missing".to_string(),
                    )));
                }
            }
        }
    }

    {
        let mut s = state.lock().await;
        s.last_firmware_mode = Some(value);
    }

    {
        let mut engine = policy_engine.lock().await;
        engine.last_applied_thermal_mode = Some(thermal_mode);
        engine.current_thermal_mode = Some(thermal_mode);
    }

    // Emit PropertiesChanged signal and ThermalModeChanged signal
    let interface_ref = conn
        .object_server()
        .interface::<_, DaemonInterface>("/io/strixwolf/alatus/Daemon")
        .await?;
    let emitter = interface_ref.signal_emitter();
    interface_ref
        .get()
        .await
        .firmware_mode_changed(emitter)
        .await?;
    let _ = DaemonInterface::thermal_mode_changed(emitter, value).await;

    Ok(())
}

pub async fn get_charge_limit(
    device_context: &Mutex<DeviceContext>,
) -> zbus::fdo::Result<u32> {
    let ctx = device_context.lock().await;
    match &ctx.capabilities.battery {
        CapabilityState::Unsupported => Err(DaemonError::CapabilityUnsupported(
            "Battery charge limit unsupported on this device".to_string(),
        )
        .into()),
        CapabilityState::Unavailable(reason) => Err(DaemonError::CapabilityUnavailable(
            format!("Battery driver unavailable: {reason}"),
        )
        .into()),
        CapabilityState::Supported(_) => {
            if let Some(ref bat) = ctx.battery {
                let threshold = bat
                    .get_charge_threshold()
                    .map_err(|e| DaemonError::IoError(e.to_string()))?;
                Ok(threshold.value() as u32)
            } else {
                Err(DaemonError::CapabilityUnavailable("Battery driver missing".to_string()).into())
            }
        }
    }
}

pub async fn set_charge_limit(
    device_context: &Mutex<DeviceContext>,
    state: &Mutex<DaemonState>,
    header: Option<zbus::message::Header<'_>>,
    conn: &Connection,
    value: u32,
) -> zbus::Result<()> {
    let threshold = if value == 0 {
        crate::domain::ChargeThreshold::new(100).unwrap()
    } else {
        crate::domain::ChargeThreshold::new(value as u8).map_err(|e| {
            zbus::Error::from(DaemonError::InvalidArgument(format!(
                "Invalid charge limit: {e}"
            )))
        })?
    };

    let sender = header
        .and_then(|h| h.sender().map(|s| s.to_owned()))
        .ok_or_else(|| {
            zbus::Error::from(DaemonError::PermissionDenied("No sender found".to_string()))
        })?;

    check_polkit(conn, &sender, "io.strixwolf.alatus.set-charge-limit").await?;

    {
        let mut ctx = device_context.lock().await;
        match &ctx.capabilities.battery {
            CapabilityState::Unsupported => {
                return Err(zbus::Error::from(DaemonError::CapabilityUnsupported(
                    "Battery charge limit unsupported on this device".to_string(),
                )));
            }
            CapabilityState::Unavailable(reason) => {
                return Err(zbus::Error::from(DaemonError::CapabilityUnavailable(
                    format!("Battery driver unavailable: {reason}"),
                )));
            }
            CapabilityState::Supported(_) => {
                if let Some(ref mut bat) = ctx.battery {
                    bat.set_charge_threshold(threshold)
                        .map_err(|e| DaemonError::IoError(e.to_string()))?;
                } else {
                    return Err(zbus::Error::from(DaemonError::CapabilityUnavailable(
                        "Battery driver missing".to_string(),
                    )));
                }
            }
        }
    }

    let mut s = state.lock().await;
    s.last_charge_limit = Some(value);

    // Emit PropertiesChanged signal
    let interface_ref = conn
        .object_server()
        .interface::<_, DaemonInterface>("/io/strixwolf/alatus/Daemon")
        .await?;
    interface_ref
        .get()
        .await
        .charge_limit_changed(interface_ref.signal_emitter())
        .await?;

    Ok(())
}

pub async fn get_deep_sleep_active() -> zbus::fdo::Result<bool> {
    read_deep_sleep().map_err(Into::into)
}

pub async fn set_deep_sleep_active(
    header: Option<zbus::message::Header<'_>>,
    conn: &Connection,
    value: bool,
) -> zbus::Result<()> {
    let sender = header
        .and_then(|h| h.sender().map(|s| s.to_owned()))
        .ok_or_else(|| {
            zbus::Error::from(DaemonError::PermissionDenied("No sender found".to_string()))
        })?;

    check_polkit(conn, &sender, "io.strixwolf.alatus.set-deep-sleep").await?;
    write_deep_sleep(value)?;

    let interface_ref = conn
        .object_server()
        .interface::<_, DaemonInterface>("/io/strixwolf/alatus/Daemon")
        .await?;
    interface_ref
        .get()
        .await
        .deep_sleep_active_changed(interface_ref.signal_emitter())
        .await?;

    Ok(())
}

pub fn get_product_serial() -> zbus::fdo::Result<String> {
    Ok(read_product_serial())
}

pub fn has_conflicting_power_daemon() -> zbus::fdo::Result<bool> {
    Ok(check_conflicting_power_daemons_sync())
}

pub async fn get_on_ac(state: &Mutex<DaemonState>) -> zbus::fdo::Result<bool> {
    let s = state.lock().await;
    Ok(s.on_ac)
}

pub async fn get_auto_thermal_profile(state: &Mutex<DaemonState>) -> zbus::fdo::Result<bool> {
    let s = state.lock().await;
    Ok(s.auto_thermal_profile)
}

pub async fn set_auto_thermal_profile(
    device_context: &Mutex<DeviceContext>,
    state: &Mutex<DaemonState>,
    header: Option<zbus::message::Header<'_>>,
    conn: &Connection,
    value: bool,
) -> zbus::Result<()> {
    let sender = header
        .and_then(|h| h.sender().map(|s| s.to_owned()))
        .ok_or_else(|| {
            zbus::Error::from(DaemonError::PermissionDenied("No sender found".to_string()))
        })?;

    check_polkit(conn, &sender, "io.strixwolf.alatus.set-firmware-mode").await?;

    let (on_ac, target_mode) = {
        let mut s = state.lock().await;
        s.auto_thermal_profile = value;
        let target = if s.on_ac {
            s.ac_thermal_mode
        } else {
            s.battery_thermal_mode
        };
        (s.on_ac, target)
    };

    let interface_ref = conn
        .object_server()
        .interface::<_, DaemonInterface>("/io/strixwolf/alatus/Daemon")
        .await?;
    interface_ref
        .get()
        .await
        .auto_thermal_profile_changed(interface_ref.signal_emitter())
        .await?;

    if value {
        tracing::info!(
            "Auto-thermal profile enabled (OnAC={on_ac}). Setting mode {target_mode}"
        );
        if let Ok(thermal_mode) = crate::domain::ThermalMode::try_from(target_mode) {
            let mut ctx = device_context.lock().await;
            if let Some(ref mut th) = ctx.thermal {
                let _ = th.set_mode(thermal_mode);
            }
        }
        interface_ref
            .get()
            .await
            .firmware_mode_changed(interface_ref.signal_emitter())
            .await?;
    }

    Ok(())
}
