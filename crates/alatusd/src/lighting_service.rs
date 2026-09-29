//! Server-side D-Bus service implementation for org.alatus.Lighting.

use crate::polkit::check_authorization;
use alatus_core::lighting::{LightingDriver, LightingEffect, LightingMode, RgbColor};
use alatus_ipc::LightingStateMsg;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use zbus::interface;
use zbus::message::Header;
use zbus::object_server::SignalContext;
use zbus::Connection;

pub struct LightingService {
    driver: Arc<dyn LightingDriver>,
    current_mode: Mutex<LightingMode>,
    current_color: Mutex<RgbColor>,
    current_speed: AtomicU8,
}

impl LightingService {
    pub fn new(driver: Arc<dyn LightingDriver>) -> Self {
        Self {
            driver,
            current_mode: Mutex::new(LightingMode::Static),
            current_color: Mutex::new(RgbColor::new(255, 255, 255)),
            current_speed: AtomicU8::new(1),
        }
    }

    async fn current_state(&self) -> Result<LightingStateMsg, zbus::fdo::Error> {
        let brightness = self
            .driver
            .get_brightness()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        let mode = *self.current_mode.lock().unwrap();
        let color = *self.current_color.lock().unwrap();
        let speed = self.current_speed.load(Ordering::SeqCst);

        let mode_str = match mode {
            LightingMode::Static => "Static",
            LightingMode::Breathing => "Breathing",
            LightingMode::Strobe => "Strobe",
            LightingMode::Rainbow => "Rainbow",
            LightingMode::Off => "Off",
        };

        Ok(LightingStateMsg {
            mode: mode_str.to_string(),
            brightness,
            r: color.r,
            g: color.g,
            b: color.b,
            speed,
        })
    }
}

#[interface(name = "org.alatus.Lighting")]
impl LightingService {
    async fn get_brightness(&self) -> zbus::fdo::Result<u8> {
        self.driver
            .get_brightness()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    async fn set_brightness(
        &self,
        #[zbus(signal_context)] ctxt: SignalContext<'_>,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
        level: u8,
    ) -> zbus::fdo::Result<()> {
        check_authorization(conn, &hdr, "org.alatus.manage-lighting").await?;

        self.driver
            .set_brightness(level)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        let state = self.current_state().await?;
        Self::state_changed(&ctxt, state).await?;
        Ok(())
    }

    async fn set_color(
        &self,
        #[zbus(signal_context)] ctxt: SignalContext<'_>,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
        r: u8,
        g: u8,
        b: u8,
    ) -> zbus::fdo::Result<()> {
        check_authorization(conn, &hdr, "org.alatus.manage-lighting").await?;

        let brightness = self
            .driver
            .get_brightness()
            .await
            .unwrap_or(3);
        let speed = self.current_speed.load(Ordering::SeqCst);
        let mode = *self.current_mode.lock().unwrap();

        let effect = LightingEffect {
            mode,
            primary_color: RgbColor::new(r, g, b),
            secondary_color: None,
            speed,
            brightness,
        };

        self.driver
            .apply_effect(&effect)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        *self.current_color.lock().unwrap() = RgbColor::new(r, g, b);

        let state = self.current_state().await?;
        Self::state_changed(&ctxt, state).await?;
        Ok(())
    }

    async fn set_mode(
        &self,
        #[zbus(signal_context)] ctxt: SignalContext<'_>,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
        mode: String,
        speed: u8,
    ) -> zbus::fdo::Result<()> {
        check_authorization(conn, &hdr, "org.alatus.manage-lighting").await?;

        let parsed_mode = match mode.to_lowercase().as_str() {
            "static" => LightingMode::Static,
            "breathing" => LightingMode::Breathing,
            "strobe" => LightingMode::Strobe,
            "rainbow" => LightingMode::Rainbow,
            "off" => LightingMode::Off,
            other => {
                return Err(zbus::fdo::Error::InvalidArgs(format!(
                    "Unknown lighting mode: '{other}'. Expected static, breathing, strobe, rainbow, or off"
                )));
            }
        };

        let brightness = self
            .driver
            .get_brightness()
            .await
            .unwrap_or(3);
        let color = *self.current_color.lock().unwrap();

        let effect = LightingEffect {
            mode: parsed_mode,
            primary_color: color,
            secondary_color: None,
            speed,
            brightness,
        };

        self.driver
            .apply_effect(&effect)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        *self.current_mode.lock().unwrap() = parsed_mode;
        self.current_speed.store(speed, Ordering::SeqCst);

        let state = self.current_state().await?;
        Self::state_changed(&ctxt, state).await?;
        Ok(())
    }

    async fn get_state(&self) -> zbus::fdo::Result<LightingStateMsg> {
        self.current_state().await
    }

    #[zbus(signal)]
    async fn state_changed(signal_ctxt: &SignalContext<'_>, state: LightingStateMsg) -> zbus::Result<()>;
}
