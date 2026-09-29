//! Server-side D-Bus service implementation for org.alatus.Thermal.

use crate::polkit::check_authorization;
use alatus_core::thermal::{ThermalDriver, ThermalProfileMode};
use alatus_ipc::FanStatusMsg;
use std::sync::Arc;
use zbus::interface;
use zbus::message::Header;
use zbus::object_server::SignalContext;
use zbus::Connection;

pub struct ThermalService {
    driver: Arc<dyn ThermalDriver>,
}

impl ThermalService {
    pub fn new(driver: Arc<dyn ThermalDriver>) -> Self {
        Self { driver }
    }
}

#[interface(name = "org.alatus.Thermal")]
impl ThermalService {
    async fn get_current_profile(&self) -> zbus::fdo::Result<String> {
        let profile = self
            .driver
            .get_current_profile()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        Ok(profile.to_string())
    }

    async fn set_profile(
        &self,
        #[zbus(signal_context)] ctxt: SignalContext<'_>,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
        mode: String,
    ) -> zbus::fdo::Result<()> {
        check_authorization(conn, &hdr, "org.alatus.manage-thermal").await?;

        let parsed_mode = match mode.to_lowercase().replace([' ', '_', '-'], "").as_str() {
            "quiet" => ThermalProfileMode::Quiet,
            "balanced" => ThermalProfileMode::Balanced,
            "performance" => ThermalProfileMode::Performance,
            "fullspeed" => ThermalProfileMode::FullSpeed,
            other => {
                return Err(zbus::fdo::Error::InvalidArgs(format!(
                    "Invalid thermal profile: '{other}'. Expected Quiet, Balanced, Performance, or FullSpeed"
                )));
            }
        };

        self.driver
            .set_profile(parsed_mode)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        let mode_str = parsed_mode.to_string();
        Self::profile_changed(&ctxt, &mode_str).await?;
        Ok(())
    }

    async fn list_profiles(&self) -> zbus::fdo::Result<Vec<String>> {
        let profiles = self
            .driver
            .available_profiles()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        Ok(profiles.into_iter().map(|p| p.to_string()).collect())
    }

    async fn get_fans(&self) -> zbus::fdo::Result<Vec<FanStatusMsg>> {
        let fans = self
            .driver
            .get_fans()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        Ok(fans.into_iter().map(Into::into).collect())
    }

    async fn is_cpu_only(&self) -> zbus::fdo::Result<bool> {
        Ok(self.driver.is_cpu_only())
    }

    #[zbus(signal)]
    pub async fn profile_changed(
        signal_ctxt: &SignalContext<'_>,
        new_profile: &str,
    ) -> zbus::Result<()>;
}
