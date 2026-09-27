// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Method handlers for the Alatus root daemon D-Bus interface.

use std::path::Path;
use std::sync::atomic::Ordering;
use tokio::sync::Mutex;
use zbus::Connection;

use super::super::error::DaemonError;
use super::super::inactivity::InactivityState;
use super::super::polkit::check_polkit;
use super::DaemonState;
use crate::hardware::capabilities::CapabilityState;
use crate::hardware::DeviceContext;
use crate::services::config::RgbTimeoutPolicy;
use crate::services::firmware_mode;
use crate::services::rgb::RgbService;

pub async fn get_capabilities(
    device_context: &Mutex<DeviceContext>,
) -> zbus::fdo::Result<String> {
    let ctx = device_context.lock().await;
    serde_json::to_string(&ctx.capabilities)
        .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
}

pub async fn get_rgb_status(
    device_context: &Mutex<DeviceContext>,
    rgb_service: &RgbService,
) -> zbus::fdo::Result<String> {
    let ctx = device_context.lock().await;
    if ctx.capabilities.rgb.is_unsupported() {
        return Err(zbus::fdo::Error::from(DaemonError::CapabilityUnsupported(
            "RGB backlighting unsupported on this device".to_string(),
        )));
    }
    let status = rgb_service.get_status();
    serde_json::to_string(&status).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
}

pub async fn set_rgb_color(
    device_context: &Mutex<DeviceContext>,
    rgb_service: &RgbService,
    header: zbus::message::Header<'_>,
    conn: &Connection,
    r: u8,
    g: u8,
    b: u8,
) -> zbus::fdo::Result<()> {
    let sender = header.sender().ok_or_else(|| {
        zbus::fdo::Error::from(DaemonError::PermissionDenied("No sender found".to_string()))
    })?;

    check_polkit(conn, sender, "io.strixwolf.alatus.set-rgb")
        .await
        .map_err(zbus::fdo::Error::from)?;

    let mut ctx = device_context.lock().await;
    match &ctx.capabilities.rgb {
        CapabilityState::Unsupported => Err(DaemonError::CapabilityUnsupported(
            "RGB backlighting unsupported on this device".to_string(),
        )
        .into()),
        CapabilityState::Unavailable(reason) => Err(DaemonError::CapabilityUnavailable(
            format!("RGB driver unavailable: {reason}"),
        )
        .into()),
        CapabilityState::Supported(_) => {
            if let Some(ref mut rgb) = ctx.rgb {
                rgb.set_color(crate::domain::ColorRgb::new(r, g, b))
                    .map_err(|e| DaemonError::IoError(e.to_string()))?;
                let _ = rgb_service.set_color(r, g, b);
                Ok(())
            } else {
                Err(DaemonError::CapabilityUnavailable("RGB driver missing".to_string()).into())
            }
        }
    }
}

pub async fn set_rgb_brightness(
    device_context: &Mutex<DeviceContext>,
    rgb_service: &RgbService,
    inactivity: &InactivityState,
    header: zbus::message::Header<'_>,
    conn: &Connection,
    brightness: u32,
) -> zbus::fdo::Result<()> {
    let brightness_pct = crate::domain::BrightnessPercent::new(brightness as u8)
        .map_err(|e| zbus::fdo::Error::from(DaemonError::InvalidArgument(e.to_string())))?;

    let sender = header.sender().ok_or_else(|| {
        zbus::fdo::Error::from(DaemonError::PermissionDenied("No sender found".to_string()))
    })?;

    check_polkit(conn, sender, "io.strixwolf.alatus.set-rgb")
        .await
        .map_err(zbus::fdo::Error::from)?;

    let mut ctx = device_context.lock().await;
    match &ctx.capabilities.rgb {
        CapabilityState::Unsupported => Err(DaemonError::CapabilityUnsupported(
            "RGB backlighting unsupported on this device".to_string(),
        )
        .into()),
        CapabilityState::Unavailable(reason) => Err(DaemonError::CapabilityUnavailable(
            format!("RGB driver unavailable: {reason}"),
        )
        .into()),
        CapabilityState::Supported(_) => {
            if let Some(ref mut rgb) = ctx.rgb {
                if brightness > 0 {
                    inactivity.wake_if_timed_out(rgb_service);
                }
                rgb.set_brightness(brightness_pct)
                    .map_err(|e| DaemonError::IoError(e.to_string()))?;
                let _ = rgb_service.set_brightness(brightness);
                inactivity.record_activity(rgb_service);
                Ok(())
            } else {
                Err(DaemonError::CapabilityUnavailable("RGB driver missing".to_string()).into())
            }
        }
    }
}

pub fn get_rgb_timeout(inactivity: &InactivityState) -> zbus::fdo::Result<u32> {
    Ok(inactivity.timeout_seconds.load(Ordering::Relaxed))
}

pub async fn set_rgb_timeout(
    rgb_service: &RgbService,
    inactivity: &InactivityState,
    header: zbus::message::Header<'_>,
    conn: &Connection,
    seconds: u32,
) -> zbus::fdo::Result<()> {
    let sender = header.sender().ok_or_else(|| {
        zbus::fdo::Error::from(DaemonError::PermissionDenied("No sender found".to_string()))
    })?;

    check_polkit(conn, sender, "io.strixwolf.alatus.set-rgb")
        .await
        .map_err(zbus::fdo::Error::from)?;

    tracing::info!("D-Bus: Setting RGB inactivity timeout to {seconds} seconds");
    inactivity
        .timeout_seconds
        .store(seconds, Ordering::Relaxed);
    inactivity.record_activity(rgb_service);
    Ok(())
}

pub fn get_rgb_timeout_policy(inactivity: &InactivityState) -> zbus::fdo::Result<String> {
    Ok(inactivity.get_policy().to_string())
}

pub async fn set_rgb_timeout_policy(
    state: &Mutex<DaemonState>,
    rgb_service: &RgbService,
    inactivity: &InactivityState,
    header: zbus::message::Header<'_>,
    conn: &Connection,
    policy_str: String,
) -> zbus::fdo::Result<()> {
    let sender = header.sender().ok_or_else(|| {
        zbus::fdo::Error::from(DaemonError::PermissionDenied("No sender found".to_string()))
    })?;

    check_polkit(conn, sender, "io.strixwolf.alatus.set-rgb")
        .await
        .map_err(zbus::fdo::Error::from)?;

    let policy = policy_str
        .parse::<RgbTimeoutPolicy>()
        .map_err(zbus::fdo::Error::InvalidArgs)?;

    let on_ac = {
        let s = state.lock().await;
        s.on_ac
    };

    tracing::info!("D-Bus: Setting RGB inactivity timeout policy to '{policy}'");
    inactivity.set_policy(policy, rgb_service, on_ac);
    Ok(())
}

pub fn wake_rgb(
    rgb_service: &RgbService,
    inactivity: &InactivityState,
) -> zbus::fdo::Result<()> {
    inactivity.wake(rgb_service);
    Ok(())
}

pub fn notify_activity(
    rgb_service: &RgbService,
    inactivity: &InactivityState,
) -> zbus::fdo::Result<()> {
    inactivity.record_activity(rgb_service);
    Ok(())
}

pub fn get_hardware_diagnostics() -> zbus::fdo::Result<(bool, String)> {
    match firmware_mode::read_firmware_mode() {
        Ok(mode) => Ok((
            true,
            format!("ASUS WMI DebugFS active (current mode: {mode:?})"),
        )),
        Err(e) => {
            if Path::new("/sys/kernel/debug/asus-nb-wmi/dev_id").exists() {
                Ok((
                    false,
                    format!("DebugFS endpoint accessible but read failed: {e}"),
                ))
            } else if Path::new("/sys/devices/platform/asus-nb-wmi").exists() {
                Ok((
                    false,
                    "Platform driver loaded but DebugFS endpoints not mounted".to_string(),
                ))
            } else {
                Ok((false, "ASUS WMI platform driver not detected".to_string()))
            }
        }
    }
}
