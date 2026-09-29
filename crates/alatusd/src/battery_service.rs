//! Server-side D-Bus service implementation for org.alatus.Battery.

use crate::polkit::check_authorization;
use alatus_core::battery::BatteryDriver;
use alatus_ipc::BatteryInfoMsg;
use std::sync::Arc;
use zbus::interface;
use zbus::object_server::SignalContext;

pub struct BatteryService {
    driver: Arc<dyn BatteryDriver>,
}

impl BatteryService {
    pub fn new(driver: Arc<dyn BatteryDriver>) -> Self {
        Self { driver }
    }
}

#[interface(name = "org.alatus.Battery")]
impl BatteryService {
    async fn get_info(&self) -> zbus::fdo::Result<BatteryInfoMsg> {
        self.driver
            .get_info()
            .await
            .map(Into::into)
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    async fn get_charge_limit(&self) -> zbus::fdo::Result<u8> {
        self.driver
            .get_charge_limit()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    async fn set_charge_limit(
        &self,
        #[zbus(signal_context)] ctxt: SignalContext<'_>,
        #[zbus(header)] hdr: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &zbus::Connection,
        limit: u8,
    ) -> zbus::fdo::Result<()> {
        check_authorization(conn, &hdr, "org.alatus.manage-battery").await?;

        self.driver
            .set_charge_limit(limit)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        Self::limit_changed(&ctxt, limit).await?;
        Ok(())
    }

    async fn get_health_percentage(&self) -> zbus::fdo::Result<u8> {
        self.driver
            .get_health_percentage()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    async fn is_charging(&self) -> zbus::fdo::Result<bool> {
        self.driver
            .is_charging()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    #[zbus(signal)]
    pub async fn limit_changed(signal_ctxt: &SignalContext<'_>, new_limit: u8) -> zbus::Result<()>;
}
