// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::services::firmware_mode::FirmwareMode;
use std::sync::Arc;
use tokio::sync::OnceCell;
use zbus::{Connection, Proxy};

static LAZY_CLIENT: OnceCell<DaemonClient> = OnceCell::const_new();

/// Returns the global shared DaemonClient, connecting if not already connected.
pub async fn get_daemon_client() -> Result<DaemonClient, DaemonClientError> {
    LAZY_CLIENT
        .get_or_try_init(DaemonClient::connect)
        .await
        .cloned()
}

#[derive(Debug, Clone)]
pub enum DaemonClientError {
    InvalidArgument(String),
    PermissionDenied(String),
    NotSupported(String),
    IoError(String),
    ConnectionFailed(String),
    ServiceNotRunning(String),
}

impl std::fmt::Display for DaemonClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidArgument(msg) => write!(f, "InvalidArgument: {msg}"),
            Self::PermissionDenied(msg) => write!(f, "PermissionDenied: {msg}"),
            Self::NotSupported(msg) => write!(f, "NotSupported: {msg}"),
            Self::IoError(msg) => write!(f, "IoError: {msg}"),
            Self::ConnectionFailed(msg) => write!(f, "ConnectionFailed: {msg}"),
            Self::ServiceNotRunning(msg) => write!(f, "ServiceNotRunning: {msg}"),
        }
    }
}

impl std::error::Error for DaemonClientError {}

impl From<zbus::Error> for DaemonClientError {
    fn from(e: zbus::Error) -> Self {
        match e {
            zbus::Error::FDO(fdo_err) => DaemonClientError::from(*fdo_err),
            other => DaemonClientError::ConnectionFailed(other.to_string()),
        }
    }
}

impl From<zbus::fdo::Error> for DaemonClientError {
    fn from(e: zbus::fdo::Error) -> Self {
        match e {
            zbus::fdo::Error::InvalidArgs(msg) => DaemonClientError::InvalidArgument(msg),
            zbus::fdo::Error::ServiceUnknown(msg) | zbus::fdo::Error::NameHasNoOwner(msg) => {
                DaemonClientError::ServiceNotRunning(format!(
                    "Daemon service inactive on D-Bus: {msg}. Start it with 'sudo systemctl start alatusd.service'"
                ))
            }
            zbus::fdo::Error::Failed(msg) => {
                if msg.contains("PermissionDenied") || msg.contains("authorization") {
                    DaemonClientError::PermissionDenied(msg)
                } else {
                    DaemonClientError::IoError(msg)
                }
            }
            zbus::fdo::Error::NotSupported(msg) => DaemonClientError::NotSupported(msg),
            other => DaemonClientError::IoError(other.to_string()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DaemonClient {
    proxy: Arc<Proxy<'static>>,
}

impl DaemonClient {
    /// Connects to the io.strixwolf.alatus.Daemon on the system bus.
    pub async fn connect() -> Result<Self, DaemonClientError> {
        let conn = Connection::system()
            .await
            .map_err(|e| DaemonClientError::ConnectionFailed(e.to_string()))?;
        let proxy = Proxy::new(
            &conn,
            "io.strixwolf.alatus.Daemon",
            "/io/strixwolf/alatus/Daemon",
            "io.strixwolf.alatus.Daemon",
        )
        .await
        .map_err(|e| DaemonClientError::ConnectionFailed(e.to_string()))?;

        Ok(Self {
            proxy: Arc::new(proxy),
        })
    }

    /// Check if the daemon has an active owner on the system bus.
    pub async fn check_active(&self) -> Result<bool, DaemonClientError> {
        let dbus_proxy = zbus::fdo::DBusProxy::new(self.proxy.connection())
            .await
            .map_err(|e| DaemonClientError::ConnectionFailed(e.to_string()))?;
        let name = zbus::names::WellKnownName::try_from("io.strixwolf.alatus.Daemon")
            .map_err(|e| DaemonClientError::InvalidArgument(e.to_string()))?;
        let active = dbus_proxy
            .name_has_owner(name.into())
            .await
            .unwrap_or(false);
        Ok(active)
    }

    /// Ping the daemon, returning an actionable error if the service is inactive.
    pub async fn ping(&self) -> Result<(), DaemonClientError> {
        if !self.check_active().await? {
            return Err(DaemonClientError::ServiceNotRunning(
                "System hardware daemon 'alatusd' is not running. Start it with: sudo systemctl start alatusd.service".to_string(),
            ));
        }
        Ok(())
    }

    /// Helper to get the underlying connection for match rule streams.
    pub fn connection(&self) -> &Connection {
        self.proxy.connection()
    }

    pub async fn get_firmware_mode(&self) -> Result<FirmwareMode, DaemonClientError> {
        let val: u32 = self.proxy.get_property("FirmwareMode").await?;
        match val {
            0 => Ok(FirmwareMode::Balanced),
            1 => Ok(FirmwareMode::Quiet),
            2 => Ok(FirmwareMode::High),
            3 => Ok(FirmwareMode::Full),
            n => Ok(FirmwareMode::Unknown(n as u8)),
        }
    }

    pub async fn set_firmware_mode(&self, mode: FirmwareMode) -> Result<(), DaemonClientError> {
        let val = match mode {
            FirmwareMode::Balanced => 0,
            FirmwareMode::Quiet => 1,
            FirmwareMode::High => 2,
            FirmwareMode::Full => 3,
            FirmwareMode::Unknown(n) => n as u32,
        };
        self.proxy.set_property("FirmwareMode", val).await?;
        Ok(())
    }

    pub async fn get_charge_limit(&self) -> Result<u32, DaemonClientError> {
        let val: u32 = self.proxy.get_property("ChargeLimit").await?;
        Ok(val)
    }

    pub async fn set_charge_limit(&self, limit: u32) -> Result<(), DaemonClientError> {
        self.proxy.set_property("ChargeLimit", limit).await?;
        Ok(())
    }

    pub async fn get_product_serial(&self) -> Result<String, DaemonClientError> {
        let val: String = self.proxy.get_property("ProductSerial").await?;
        Ok(val)
    }

    pub async fn get_deep_sleep_active(&self) -> Result<bool, DaemonClientError> {
        let val: bool = self.proxy.get_property("DeepSleepActive").await?;
        Ok(val)
    }

    pub async fn set_deep_sleep_active(&self, active: bool) -> Result<(), DaemonClientError> {
        self.proxy.set_property("DeepSleepActive", active).await?;
        Ok(())
    }

    pub async fn get_on_ac(&self) -> Result<bool, DaemonClientError> {
        let val: bool = self.proxy.get_property("OnAc").await?;
        Ok(val)
    }

    pub async fn get_auto_thermal_profile(&self) -> Result<bool, DaemonClientError> {
        let val: bool = self.proxy.get_property("AutoThermalProfile").await?;
        Ok(val)
    }

    pub async fn set_auto_thermal_profile(&self, enable: bool) -> Result<(), DaemonClientError> {
        self.proxy
            .set_property("AutoThermalProfile", enable)
            .await?;
        Ok(())
    }

    pub async fn get_has_conflicting_power_daemon(&self) -> Result<bool, DaemonClientError> {
        let val: bool = self.proxy.get_property("HasConflictingPowerDaemon").await?;
        Ok(val)
    }

    pub async fn receive_firmware_mode_changed(&self) -> zbus::proxy::PropertyStream<'_, u32> {
        self.proxy.receive_property_changed("FirmwareMode").await
    }

    pub async fn receive_charge_limit_changed(&self) -> zbus::proxy::PropertyStream<'_, u32> {
        self.proxy.receive_property_changed("ChargeLimit").await
    }

    pub async fn receive_deep_sleep_active_changed(&self) -> zbus::proxy::PropertyStream<'_, bool> {
        self.proxy.receive_property_changed("DeepSleepActive").await
    }

    pub async fn get_rgb_status(
        &self,
    ) -> Result<crate::services::rgb::RgbStatus, DaemonClientError> {
        let json_str: String = self.proxy.call("GetRgbStatus", &()).await?;
        serde_json::from_str(&json_str).map_err(|e| {
            DaemonClientError::IoError(format!("Failed to parse RGB status JSON: {e}"))
        })
    }

    pub async fn set_rgb_color(&self, r: u8, g: u8, b: u8) -> Result<(), DaemonClientError> {
        let _: () = self.proxy.call("SetRgbColor", &(r, g, b)).await?;
        Ok(())
    }

    pub async fn set_rgb_brightness(&self, brightness: u32) -> Result<(), DaemonClientError> {
        let _: () = self.proxy.call("SetRgbBrightness", &(brightness,)).await?;
        Ok(())
    }

    pub async fn get_hardware_diagnostics(&self) -> Result<(bool, String), DaemonClientError> {
        self.proxy
            .call("GetHardwareDiagnostics", &())
            .await
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gui_firmware_mode_validation() {
        let check_mode = |val: u32| match val {
            0 => Ok(FirmwareMode::Balanced),
            1 => Ok(FirmwareMode::Quiet),
            2 => Ok(FirmwareMode::High),
            3 => Ok(FirmwareMode::Full),
            n => Err(DaemonClientError::InvalidArgument(format!(
                "Invalid firmware mode: {n}"
            ))),
        };

        assert_eq!(check_mode(0).unwrap(), FirmwareMode::Balanced);
        assert_eq!(check_mode(1).unwrap(), FirmwareMode::Quiet);
        assert_eq!(check_mode(2).unwrap(), FirmwareMode::High);
        assert_eq!(check_mode(3).unwrap(), FirmwareMode::Full);
        assert!(check_mode(4).is_err());
    }

    #[test]
    fn test_gui_charge_limit_validation() {
        let check_limit = |limit: u32| {
            if limit <= 100 {
                Ok(limit)
            } else {
                Err(DaemonClientError::InvalidArgument(
                    "Charge limit must be 0..=100".to_string(),
                ))
            }
        };

        assert_eq!(check_limit(0).unwrap(), 0);
        assert_eq!(check_limit(80).unwrap(), 80);
        assert_eq!(check_limit(100).unwrap(), 100);
        assert!(check_limit(101).is_err());
    }

    #[test]
    fn test_daemon_error_mapping() {
        let map_error = |e: zbus::Error| -> DaemonClientError { DaemonClientError::from(e) };

        let fdo_invalid =
            zbus::Error::FDO(Box::new(zbus::fdo::Error::InvalidArgs("test".to_string())));
        let fdo_not_supported =
            zbus::Error::FDO(Box::new(zbus::fdo::Error::NotSupported("test".to_string())));
        let fdo_failed_perm = zbus::Error::FDO(Box::new(zbus::fdo::Error::Failed(
            "PermissionDenied: unauthorized".to_string(),
        )));
        let fdo_failed_io = zbus::Error::FDO(Box::new(zbus::fdo::Error::Failed(
            "IoError: read failure".to_string(),
        )));

        assert!(matches!(
            map_error(fdo_invalid),
            DaemonClientError::InvalidArgument(_)
        ));
        assert!(matches!(
            map_error(fdo_not_supported),
            DaemonClientError::NotSupported(_)
        ));
        assert!(matches!(
            map_error(fdo_failed_perm),
            DaemonClientError::PermissionDenied(_)
        ));
        assert!(matches!(
            map_error(fdo_failed_io),
            DaemonClientError::IoError(_)
        ));
    }
}
