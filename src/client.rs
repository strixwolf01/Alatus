// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Unified D-Bus client layer for Alatus system daemon and desktop session agent.

pub use crate::services::daemon_client::{DaemonClient, DaemonClientError};
pub use crate::services::session::accent::set_session_accent_sync;
pub use crate::services::session::dbus::{
    query_display_info, query_session_status, set_session_auto_refresh,
    set_session_oled_care, set_session_oled_dim_level, trigger_session_pixel_refresh, DisplayInfo,
    SessionStatusInfo,
};
pub use crate::services::session::display_care::set_panel_refresh_rate;

use std::time::Duration;
use zbus::Connection;

/// Unified client interface connecting to both `alatusd` (system bus)
/// and `alatus-session` (user session bus).
#[derive(Clone, Debug)]
pub struct AlatusClient {
    daemon: Option<DaemonClient>,
    session_conn: Option<Connection>,
}

impl AlatusClient {
    /// Connects to the system daemon. Session bus connection is established lazily if needed.
    pub async fn connect() -> Result<Self, DaemonClientError> {
        let daemon = DaemonClient::connect().await?;
        let session_conn = Connection::session().await.ok();
        Ok(Self {
            daemon: Some(daemon),
            session_conn,
        })
    }

    /// Creates a client for session-only operations (when alatusd is not running).
    pub async fn session_only() -> Self {
        let session_conn = Connection::session().await.ok();
        Self {
            daemon: None,
            session_conn,
        }
    }

    /// Connects to the system daemon with retry policy.
    pub async fn connect_with_retry(
        max_retries: u32,
        delay: Duration,
    ) -> Result<Self, DaemonClientError> {
        let daemon = DaemonClient::connect_with_retry(max_retries, delay).await?;
        let session_conn = Connection::session().await.ok();
        Ok(Self {
            daemon: Some(daemon),
            session_conn,
        })
    }

    fn require_daemon(&self) -> Result<&DaemonClient, DaemonClientError> {
        self.daemon.as_ref().ok_or_else(|| {
            DaemonClientError::ServiceNotRunning("alatusd daemon is not running".to_string())
        })
    }

    /// Access the underlying privileged system daemon client.
    pub fn daemon(&self) -> &DaemonClient {
        self.daemon.as_ref().expect("DaemonClient requested on session-only client")
    }

    /// Access the system D-Bus connection.
    pub fn connection(&self) -> &Connection {
        self.daemon.as_ref().expect("Connection requested on session-only client").connection()
    }

    /// Access the user session D-Bus connection if available.
    pub fn session_connection(&self) -> Option<&Connection> {
        self.session_conn.as_ref()
    }

    // --- High-level daemon facade methods ---

    pub async fn ping(&self) -> Result<(), DaemonClientError> {
        self.require_daemon()?.ping().await
    }

    pub async fn get_auto_thermal_profile(&self) -> Result<bool, DaemonClientError> {
        self.require_daemon()?.get_auto_thermal_profile().await
    }

    pub async fn set_auto_thermal_profile(&self, enable: bool) -> Result<(), DaemonClientError> {
        self.require_daemon()?.set_auto_thermal_profile(enable).await
    }

    pub async fn get_firmware_mode(&self) -> Result<crate::services::firmware_mode::FirmwareMode, DaemonClientError> {
        self.require_daemon()?.get_firmware_mode().await
    }

    pub async fn set_firmware_mode(
        &self,
        mode: crate::services::firmware_mode::FirmwareMode,
    ) -> Result<(), DaemonClientError> {
        self.require_daemon()?.set_firmware_mode(mode).await
    }

    pub async fn get_charge_limit(&self) -> Result<u32, DaemonClientError> {
        self.require_daemon()?.get_charge_limit().await
    }

    pub async fn set_charge_limit(&self, limit: u32) -> Result<(), DaemonClientError> {
        self.require_daemon()?.set_charge_limit(limit).await
    }

    pub async fn get_capabilities(&self) -> Result<crate::hardware::capabilities::SystemCapabilities, DaemonClientError> {
        self.require_daemon()?.get_capabilities().await
    }

    pub async fn get_rgb_status(&self) -> Result<crate::services::rgb::RgbStatus, DaemonClientError> {
        self.require_daemon()?.get_rgb_status().await
    }

    pub async fn set_rgb_color(&self, r: u8, g: u8, b: u8) -> Result<(), DaemonClientError> {
        self.require_daemon()?.set_rgb_color(r, g, b).await
    }

    pub async fn set_rgb_brightness(&self, brightness: u32) -> Result<(), DaemonClientError> {
        self.require_daemon()?.set_rgb_brightness(brightness).await
    }

    pub async fn get_rgb_timeout(&self) -> Result<u32, DaemonClientError> {
        self.require_daemon()?.get_rgb_timeout().await
    }

    pub async fn set_rgb_timeout(&self, seconds: u32) -> Result<(), DaemonClientError> {
        self.require_daemon()?.set_rgb_timeout(seconds).await
    }

    pub async fn get_rgb_timeout_policy(&self) -> Result<String, DaemonClientError> {
        self.require_daemon()?.get_rgb_timeout_policy().await
    }

    pub async fn set_rgb_timeout_policy(&self, policy: &str) -> Result<(), DaemonClientError> {
        self.require_daemon()?.set_rgb_timeout_policy(policy).await
    }

    pub async fn wake_rgb(&self) -> Result<(), DaemonClientError> {
        self.require_daemon()?.wake_rgb().await
    }

    pub async fn notify_activity(&self) -> Result<(), DaemonClientError> {
        self.require_daemon()?.notify_activity().await
    }

    pub async fn get_deep_sleep_active(&self) -> Result<bool, DaemonClientError> {
        self.require_daemon()?.get_deep_sleep_active().await
    }

    pub async fn set_deep_sleep_active(&self, active: bool) -> Result<(), DaemonClientError> {
        self.require_daemon()?.set_deep_sleep_active(active).await
    }

    pub async fn get_on_ac(&self) -> Result<bool, DaemonClientError> {
        self.require_daemon()?.get_on_ac().await
    }

    pub async fn get_hardware_diagnostics(&self) -> Result<(bool, String), DaemonClientError> {
        self.require_daemon()?.get_hardware_diagnostics().await
    }

    // --- High-level session facade methods ---

    pub async fn query_session_status(&self) -> SessionStatusInfo {
        query_session_status().await
    }

    pub async fn query_display_info(&self) -> DisplayInfo {
        query_display_info().await
    }

    pub async fn set_oled_dim_level(&self, level: u32) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        set_session_oled_dim_level(level).await
    }

    pub async fn trigger_pixel_refresh(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        trigger_session_pixel_refresh().await
    }

    pub async fn set_accent_sync(&self, enable: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        set_session_accent_sync(enable).await
    }

    pub async fn set_auto_refresh(&self, enable: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        set_session_auto_refresh(enable).await
    }

    pub async fn set_oled_care(&self, enable: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        set_session_oled_care(enable).await
    }

    pub async fn set_refresh_rate(&self, target_hz: f64) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        set_panel_refresh_rate(target_hz).await
    }
}
