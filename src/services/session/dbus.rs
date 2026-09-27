// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! D-Bus service interface and client queries for user desktop sessions.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use zbus::Connection;

use crate::services::session::desktop::gnome::MutterGetCurrentStateReturn;
use crate::services::session::desktop::{create_kscreen_doctor_cmd, detect_desktop, DesktopEnv};
use crate::services::session::display_care::dimming::apply_flicker_free_dimming;
use crate::services::session::display_care::oled::{
    execute_pixel_refresh, load_oled_metrics, save_oled_metrics,
};
use crate::services::session::notifications::{send_osd_notification, thermal_mode_notification};
use crate::services::session::state::SessionState;

pub const SESSION_BUS_NAME: &str = "io.strixwolf.alatus.Session";
pub const SESSION_OBJECT_PATH: &str = "/io/strixwolf/alatus/Session";

pub struct SessionDbusInterface {
    pub state: Arc<SessionState>,
    pub session_conn: Connection,
    pub desktop: DesktopEnv,
    pub internal_connector: Option<String>,
}

#[zbus::interface(name = "io.strixwolf.alatus.Session")]
impl SessionDbusInterface {
    #[zbus(property)]
    fn sync_accent(&self) -> bool {
        self.state.sync_accent.load(Ordering::Relaxed)
    }

    #[zbus(name = "SetSyncAccent")]
    async fn set_sync_accent(&self, value: bool) {
        self.state.sync_accent.store(value, Ordering::Relaxed);
        tracing::info!("Session: Desktop Accent Sync dynamically set to {value}");
    }

    #[zbus(property)]
    fn auto_refresh(&self) -> bool {
        self.state.auto_refresh.load(Ordering::Relaxed)
    }

    #[zbus(name = "SetAutoRefresh")]
    async fn set_auto_refresh(&self, value: bool) {
        self.state.auto_refresh.store(value, Ordering::Relaxed);
        tracing::info!("Session: Panel Refresh Switching dynamically set to {value}");
    }

    #[zbus(property)]
    fn oled_care(&self) -> bool {
        self.state.oled_care.load(Ordering::Relaxed)
    }

    #[zbus(property)]
    async fn set_oled_care(&self, value: bool) {
        self.state.oled_care.store(value, Ordering::Relaxed);
        tracing::info!("Session: OLED Care dynamically set to {value}");
    }

    #[zbus(name = "SetOledCare")]
    async fn set_oled_care_method(&self, value: bool) {
        self.set_oled_care(value).await;
    }

    #[zbus(property)]
    fn pixel_refresh_hours(&self) -> f64 {
        (self.state.active_screen_seconds.load(Ordering::Relaxed) as f64) / 3600.0
    }

    #[zbus(property)]
    fn pixel_refresh_count(&self) -> u32 {
        self.state.pixel_refresh_count.load(Ordering::Relaxed)
    }

    #[zbus(property)]
    fn pixel_refresh_in_progress(&self) -> bool {
        self.state.pixel_refresh_in_progress.load(Ordering::Relaxed)
    }

    #[zbus(name = "TriggerPixelRefresh")]
    async fn trigger_pixel_refresh_method(&self) -> bool {
        execute_pixel_refresh(
            &self.session_conn,
            Arc::clone(&self.state),
            self.desktop,
            self.internal_connector.clone(),
        )
        .await
    }

    #[zbus(property)]
    fn power_monitor_active(&self) -> bool {
        self.state.power_monitor_active.load(Ordering::Relaxed)
    }

    #[zbus(property)]
    fn oled_dim_level(&self) -> u32 {
        self.state.oled_dim_level.load(Ordering::Relaxed)
    }

    #[zbus(name = "SetOledDimLevel")]
    async fn set_oled_dim_level(&self, value: u32) {
        let clamped = value.clamp(10, 100);
        self.state.oled_dim_level.store(clamped, Ordering::Relaxed);
        let mut metrics = load_oled_metrics();
        metrics.oled_dim_level = clamped;
        save_oled_metrics(&metrics);
        tracing::info!("Session: Flicker-Free OLED Dim Level dynamically set to {clamped}%");

        // Instant Compositor-level flicker-free dimming (never touches /sys/class/backlight)
        apply_flicker_free_dimming(self.internal_connector.as_deref(), clamped).await;
    }

    #[zbus(name = "SetFlickerFreeDimming")]
    async fn set_flicker_free_dimming(&self, value: u32) {
        self.set_oled_dim_level(value).await;
    }

    #[zbus(name = "GetStatus")]
    async fn get_status(&self) -> (bool, bool, bool, bool, f64, u32) {
        (
            self.state.sync_accent.load(Ordering::Relaxed),
            self.state.auto_refresh.load(Ordering::Relaxed),
            self.state.power_monitor_active.load(Ordering::Relaxed),
            self.state.oled_care.load(Ordering::Relaxed),
            (self.state.active_screen_seconds.load(Ordering::Relaxed) as f64) / 3600.0,
            self.state.pixel_refresh_count.load(Ordering::Relaxed),
        )
    }

    #[zbus(name = "ShowThermalOsd")]
    async fn show_thermal_osd(&self, mode: u32) {
        let (summary, body, icon) = thermal_mode_notification(mode);
        let _ = send_osd_notification(&self.session_conn, summary, body, &icon).await;
    }
}

/// Checks whether the desktop session daemon is currently running and owns its D-Bus name.
pub async fn is_session_daemon_running() -> bool {
    if let Ok(conn) = Connection::session().await
        && let Ok(proxy) = zbus::fdo::DBusProxy::new(&conn).await
        && let Ok(name) = zbus::names::WellKnownName::try_from(SESSION_BUS_NAME)
    {
        return proxy.name_has_owner(name.into()).await.unwrap_or(false);
    }
    false
}

#[derive(Debug, Clone, Default)]
pub struct SessionStatusInfo {
    pub running: bool,
    pub desktop: DesktopEnv,
    pub sync_accent: bool,
    pub auto_refresh: bool,
    pub power_monitor_active: bool,
    pub oled_care: bool,
    pub pixel_refresh_hours: f64,
    pub pixel_refresh_count: u32,
    pub oled_dim_level: u32,
}

pub async fn query_session_status() -> SessionStatusInfo {
    let desktop = detect_desktop();
    let metrics = load_oled_metrics();

    let Ok(conn) = Connection::session().await else {
        return SessionStatusInfo {
            running: false,
            desktop,
            sync_accent: true,
            auto_refresh: true,
            power_monitor_active: false,
            oled_care: true,
            pixel_refresh_hours: (metrics.active_screen_seconds as f64) / 3600.0,
            pixel_refresh_count: metrics.refresh_count,
            oled_dim_level: metrics.oled_dim_level,
        };
    };

    // Check if the well-known name has an active owner on the session bus
    let has_owner = if let Ok(dbus_proxy) = zbus::fdo::DBusProxy::new(&conn).await
        && let Ok(name) = zbus::names::WellKnownName::try_from(SESSION_BUS_NAME)
    {
        dbus_proxy
            .name_has_owner(name.into())
            .await
            .unwrap_or(false)
    } else {
        false
    };

    let mut sync_accent = true;
    let mut auto_refresh = true;
    let mut power_monitor_active = false;
    let mut oled_care = true;
    let mut pixel_refresh_hours = (metrics.active_screen_seconds as f64) / 3600.0;
    let mut pixel_refresh_count = metrics.refresh_count;
    let mut oled_dim_level = metrics.oled_dim_level;
    let mut call_succeeded = false;

    if let Ok(proxy) = zbus::Proxy::new(
        &conn,
        SESSION_BUS_NAME,
        SESSION_OBJECT_PATH,
        SESSION_BUS_NAME,
    )
    .await
    {
        // 1. Try 6-tuple GetStatus (v0.3.2+)
        if let Ok((sa, ar, pa, oc, rh, rc)) =
            proxy.call("GetStatus", &()).await as Result<(bool, bool, bool, bool, f64, u32), _>
        {
            sync_accent = sa;
            auto_refresh = ar;
            power_monitor_active = pa;
            oled_care = oc;
            pixel_refresh_hours = rh;
            pixel_refresh_count = rc;
            call_succeeded = true;
        } else if let Ok((sa, ar, pa)) =
            proxy.call("GetStatus", &()).await as Result<(bool, bool, bool), _>
        {
            // 2. Fallback to 3-tuple GetStatus (earlier versions)
            sync_accent = sa;
            auto_refresh = ar;
            power_monitor_active = pa;
            call_succeeded = true;

            if let Ok(oc) = proxy.get_property::<bool>("OledCare").await {
                oled_care = oc;
            }
            if let Ok(rh) = proxy.get_property::<f64>("PixelRefreshHours").await {
                pixel_refresh_hours = rh;
            }
            if let Ok(rc) = proxy.get_property::<u32>("PixelRefreshCount").await {
                pixel_refresh_count = rc;
            }
        } else if has_owner {
            // 3. If GetStatus call failed or timed out but daemon is running, query properties directly
            if let Ok(sa) = proxy.get_property::<bool>("SyncAccent").await {
                sync_accent = sa;
            }
            if let Ok(ar) = proxy.get_property::<bool>("AutoRefresh").await {
                auto_refresh = ar;
            }
            if let Ok(pa) = proxy.get_property::<bool>("PowerMonitorActive").await {
                power_monitor_active = pa;
            }
            if let Ok(oc) = proxy.get_property::<bool>("OledCare").await {
                oled_care = oc;
            }
            if let Ok(rh) = proxy.get_property::<f64>("PixelRefreshHours").await {
                pixel_refresh_hours = rh;
            }
            if let Ok(rc) = proxy.get_property::<u32>("PixelRefreshCount").await {
                pixel_refresh_count = rc;
            }
        }

        if let Ok(odl) = proxy.get_property::<u32>("OledDimLevel").await {
            oled_dim_level = odl;
        }
    }

    SessionStatusInfo {
        running: has_owner || call_succeeded,
        desktop,
        sync_accent,
        auto_refresh,
        power_monitor_active,
        oled_care,
        pixel_refresh_hours,
        pixel_refresh_count,
        oled_dim_level,
    }
}

pub async fn set_session_oled_dim_level(level: u32) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let level = level.clamp(10, 100);
    let mut metrics = load_oled_metrics();
    metrics.oled_dim_level = level;
    save_oled_metrics(&metrics);

    if let Ok(conn) = Connection::session().await {
        let proxy = zbus::Proxy::new(
            &conn,
            SESSION_BUS_NAME,
            SESSION_OBJECT_PATH,
            SESSION_BUS_NAME,
        )
        .await?;
        let () = proxy.call("SetOledDimLevel", &level).await?;
    }
    Ok(())
}

pub async fn trigger_session_pixel_refresh() -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let conn = Connection::session().await?;
    let proxy = zbus::Proxy::new(
        &conn,
        SESSION_BUS_NAME,
        SESSION_OBJECT_PATH,
        SESSION_BUS_NAME,
    )
    .await?;
    let res: bool = proxy.call("TriggerPixelRefresh", &()).await?;
    Ok(res)
}

pub async fn set_session_auto_refresh(enable: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let conn = Connection::session().await?;
    let proxy = zbus::Proxy::new(
        &conn,
        SESSION_BUS_NAME,
        SESSION_OBJECT_PATH,
        SESSION_BUS_NAME,
    )
    .await?;
    let () = proxy.call("SetAutoRefresh", &enable).await?;
    Ok(())
}

pub async fn set_session_oled_care(enable: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let conn = Connection::session().await?;
    let proxy = zbus::Proxy::new(
        &conn,
        SESSION_BUS_NAME,
        SESSION_OBJECT_PATH,
        SESSION_BUS_NAME,
    )
    .await?;
    let () = proxy.call("SetOledCare", &enable).await?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DisplayInfo {
    pub connector: String,
    pub width: u32,
    pub height: u32,
    pub refresh_rate: f64,
    pub auto_refresh: bool,
}

pub async fn query_display_info() -> DisplayInfo {
    let desktop = detect_desktop();
    let status = query_session_status().await;

    // 1. Try KDE via kscreen-doctor
    if desktop == DesktopEnv::Kde
        && let Some(mut cmd) = create_kscreen_doctor_cmd()
        && let Ok(output) = cmd.arg("-j").output()
        && output.status.success()
        && let Ok(json_str) = std::str::from_utf8(&output.stdout)
        && let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str)
        && let Some(outputs) = json.get("outputs").and_then(|o| o.as_array())
    {
        for out in outputs {
            let name = out.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let connected = out
                .get("connected")
                .and_then(|c| c.as_bool())
                .unwrap_or(true);
            if name.starts_with("eDP") && connected {
                let mut width = out
                    .get("size")
                    .and_then(|s| s.get("width"))
                    .and_then(|w| w.as_u64())
                    .unwrap_or(0) as u32;
                let mut height = out
                    .get("size")
                    .and_then(|s| s.get("height"))
                    .and_then(|h| h.as_u64())
                    .unwrap_or(0) as u32;
                let mut refresh = 60.0;

                if let Some(modes) = out.get("modes").and_then(|m| m.as_array()) {
                    for m in modes {
                        let is_cur = m.get("current").and_then(|c| c.as_bool()).unwrap_or(false)
                            || m.get("isCurrent")
                                .and_then(|c| c.as_bool())
                                .unwrap_or(false);
                        if is_cur {
                            if let Some(size) = m.get("size") {
                                if let Some(w) = size.get("width").and_then(|v| v.as_u64()) {
                                    width = w as u32;
                                }
                                if let Some(h) = size.get("height").and_then(|v| v.as_u64()) {
                                    height = h as u32;
                                }
                            }
                            if let Some(r) = m.get("refreshRate").and_then(|v| v.as_f64()) {
                                refresh = r;
                            }
                            break;
                        }
                    }
                }

                return DisplayInfo {
                    connector: name.to_string(),
                    width,
                    height,
                    refresh_rate: refresh,
                    auto_refresh: status.auto_refresh,
                };
            }
        }
    }

    // 2. Try GNOME Mutter DisplayConfig
    if desktop == DesktopEnv::Gnome
        && let Ok(conn) = Connection::session().await
        && let Ok(proxy) = zbus::Proxy::new(
            &conn,
            "org.gnome.Mutter.DisplayConfig",
            "/org/gnome/Mutter/DisplayConfig",
            "org.gnome.Mutter.DisplayConfig",
        )
        .await
        && let Ok(state) =
            proxy.call("GetCurrentState", &()).await as Result<MutterGetCurrentStateReturn, _>
    {
        let (_serial, monitors, _logical, _props) = state;
        if let Some(edp) = monitors.iter().find(|m| m.0.0.starts_with("eDP")) {
            let conn_name = edp.0.0.clone();
            let mut width = 0;
            let mut height = 0;
            let mut refresh = 60.0;

            for mode in &edp.1 {
                let is_cur = mode
                    .6
                    .get("is-current")
                    .and_then(|v| bool::try_from(v).ok())
                    .unwrap_or(false);
                if is_cur {
                    width = mode.1 as u32;
                    height = mode.2 as u32;
                    refresh = mode.3;
                    break;
                }
            }

            return DisplayInfo {
                connector: conn_name,
                width,
                height,
                refresh_rate: refresh,
                auto_refresh: status.auto_refresh,
            };
        }
    }

    // 3. Fallback: DRM sysfs or generic detection
    let mut fallback_connector = "eDP-1".to_string();
    let mut fallback_width = 2880;
    let mut fallback_height = 1800;

    if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.contains("eDP") {
                if let Some(idx) = name.find("eDP") {
                    fallback_connector = name[idx..].to_string();
                }
                if let Ok(modes) = std::fs::read_to_string(entry.path().join("modes"))
                    && let Some(first_mode) = modes.lines().next()
                {
                    let parts: Vec<&str> = first_mode.split('x').collect();
                    if parts.len() == 2
                        && let (Ok(w), Ok(h)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>())
                    {
                        fallback_width = w;
                        fallback_height = h;
                    }
                }
                break;
            }
        }
    }

    DisplayInfo {
        connector: fallback_connector,
        width: fallback_width,
        height: fallback_height,
        refresh_rate: 120.0,
        auto_refresh: status.auto_refresh,
    }
}
