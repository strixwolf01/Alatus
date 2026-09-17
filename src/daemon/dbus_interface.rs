// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use super::inactivity::InactivityState;
use super::power::{
    DaemonError, check_conflicting_power_daemons_sync, check_is_on_ac, check_polkit,
    read_deep_sleep, read_product_serial, write_deep_sleep,
};
use crate::hardware::DeviceContext;
use crate::hardware::capabilities::CapabilityState;
use crate::services::config::RgbTimeoutPolicy;
use crate::services::firmware_mode;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tokio::sync::Mutex;
use zbus::{Connection, interface};

pub struct DaemonState {
    pub device_context: Arc<Mutex<DeviceContext>>,
    pub policy_engine: Arc<Mutex<crate::daemon::policy::PolicyEngine>>,
    pub last_charge_limit: Option<u32>,
    pub last_firmware_mode: Option<u32>,
    pub on_ac: bool,
    pub auto_thermal_profile: bool,
    pub ac_thermal_mode: u32,
    pub battery_thermal_mode: u32,
}

impl DaemonState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        device_context: Arc<Mutex<DeviceContext>>,
        policy_engine: Arc<Mutex<crate::daemon::policy::PolicyEngine>>,
        charge_limit: Option<u32>,
        firmware_mode: Option<u32>,
        on_ac: bool,
        auto_thermal_profile: bool,
        ac_thermal_mode: u32,
        battery_thermal_mode: u32,
    ) -> Self {
        Self {
            device_context,
            policy_engine,
            last_charge_limit: charge_limit,
            last_firmware_mode: firmware_mode,
            on_ac,
            auto_thermal_profile,
            ac_thermal_mode,
            battery_thermal_mode,
        }
    }
}

#[derive(Clone)]
pub struct DaemonInterface {
    pub state: Arc<Mutex<DaemonState>>,
    pub device_context: Arc<Mutex<DeviceContext>>,
    pub policy_engine: Arc<Mutex<crate::daemon::policy::PolicyEngine>>,
    pub rgb_service: Arc<crate::services::rgb::RgbService>,
    pub inactivity: Arc<InactivityState>,
}

impl DaemonInterface {
    pub fn new(
        state: Arc<Mutex<DaemonState>>,
        device_context: Arc<Mutex<DeviceContext>>,
        policy_engine: Arc<Mutex<crate::daemon::policy::PolicyEngine>>,
        rgb_service: Arc<crate::services::rgb::RgbService>,
        inactivity: Arc<InactivityState>,
    ) -> Self {
        Self {
            state,
            device_context,
            policy_engine,
            rgb_service,
            inactivity,
        }
    }
}

#[interface(name = "io.strixwolf.alatus.Daemon")]
impl DaemonInterface {
    #[zbus(property)]
    async fn firmware_mode(&self) -> zbus::fdo::Result<u32> {
        let ctx = self.device_context.lock().await;
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
                    Err(
                        DaemonError::CapabilityUnavailable("Thermal driver missing".to_string())
                            .into(),
                    )
                }
            }
        }
    }

    #[zbus(property)]
    async fn set_firmware_mode(
        &self,
        #[zbus(header)] header: Option<zbus::message::Header<'_>>,
        #[zbus(connection)] conn: &Connection,
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
            let mut ctx = self.device_context.lock().await;
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
            let mut s = self.state.lock().await;
            s.last_firmware_mode = Some(value);
        }

        {
            let mut engine = self.policy_engine.lock().await;
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

    #[zbus(property)]
    async fn charge_limit(&self) -> zbus::fdo::Result<u32> {
        let ctx = self.device_context.lock().await;
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
                    Err(
                        DaemonError::CapabilityUnavailable("Battery driver missing".to_string())
                            .into(),
                    )
                }
            }
        }
    }

    #[zbus(property)]
    async fn set_charge_limit(
        &self,
        #[zbus(header)] header: Option<zbus::message::Header<'_>>,
        #[zbus(connection)] conn: &Connection,
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
            let mut ctx = self.device_context.lock().await;
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

        let mut state = self.state.lock().await;
        state.last_charge_limit = Some(value);

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

    #[zbus(property)]
    async fn deep_sleep_active(&self) -> zbus::fdo::Result<bool> {
        read_deep_sleep().map_err(Into::into)
    }

    #[zbus(property)]
    async fn set_deep_sleep_active(
        &self,
        #[zbus(header)] header: Option<zbus::message::Header<'_>>,
        #[zbus(connection)] conn: &Connection,
        value: bool,
    ) -> zbus::Result<()> {
        let sender = header
            .and_then(|h| h.sender().map(|s| s.to_owned()))
            .ok_or_else(|| {
                zbus::Error::from(DaemonError::PermissionDenied("No sender found".to_string()))
            })?;

        check_polkit(conn, &sender, "io.strixwolf.alatus.set-deep-sleep").await?;
        write_deep_sleep(value)?;

        // Emit PropertiesChanged signal
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

    #[zbus(property)]
    async fn product_serial(&self) -> zbus::fdo::Result<String> {
        Ok(read_product_serial())
    }

    #[zbus(property)]
    async fn has_conflicting_power_daemon(&self) -> zbus::fdo::Result<bool> {
        Ok(check_conflicting_power_daemons_sync())
    }

    #[zbus(property)]
    async fn on_ac(&self) -> zbus::fdo::Result<bool> {
        let state = self.state.lock().await;
        Ok(state.on_ac)
    }

    #[zbus(property)]
    async fn auto_thermal_profile(&self) -> zbus::fdo::Result<bool> {
        let state = self.state.lock().await;
        Ok(state.auto_thermal_profile)
    }

    #[zbus(property)]
    async fn set_auto_thermal_profile(
        &self,
        #[zbus(header)] header: Option<zbus::message::Header<'_>>,
        #[zbus(connection)] conn: &Connection,
        value: bool,
    ) -> zbus::Result<()> {
        let sender = header
            .and_then(|h| h.sender().map(|s| s.to_owned()))
            .ok_or_else(|| {
                zbus::Error::from(DaemonError::PermissionDenied("No sender found".to_string()))
            })?;

        check_polkit(conn, &sender, "io.strixwolf.alatus.set-firmware-mode").await?;

        let (on_ac, target_mode) = {
            let mut state = self.state.lock().await;
            state.auto_thermal_profile = value;
            let target = if state.on_ac {
                state.ac_thermal_mode
            } else {
                state.battery_thermal_mode
            };
            (state.on_ac, target)
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
                let mut ctx = self.device_context.lock().await;
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

    #[zbus(signal)]
    pub async fn power_source_changed(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        on_ac: bool,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn thermal_mode_changed(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        mode: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn thermal_osd_triggered(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        mode: u32,
    ) -> zbus::Result<()>;

    #[zbus(name = "GetCapabilities")]
    async fn get_capabilities(&self) -> zbus::fdo::Result<String> {
        let ctx = self.device_context.lock().await;
        serde_json::to_string(&ctx.capabilities)
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    #[zbus(name = "GetRgbStatus")]
    async fn get_rgb_status(&self) -> zbus::fdo::Result<String> {
        let ctx = self.device_context.lock().await;
        if ctx.capabilities.rgb.is_unsupported() {
            return Err(zbus::fdo::Error::from(DaemonError::CapabilityUnsupported(
                "RGB backlighting unsupported on this device".to_string(),
            )));
        }
        let status = self.rgb_service.get_status();
        serde_json::to_string(&status).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    #[zbus(name = "SetRgbColor")]
    async fn set_rgb_color(
        &self,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &Connection,
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

        let mut ctx = self.device_context.lock().await;
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
                    let _ = self.rgb_service.set_color(r, g, b);
                    Ok(())
                } else {
                    Err(DaemonError::CapabilityUnavailable("RGB driver missing".to_string()).into())
                }
            }
        }
    }

    #[zbus(name = "SetRgbBrightness")]
    async fn set_rgb_brightness(
        &self,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &Connection,
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

        let mut ctx = self.device_context.lock().await;
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
                        self.inactivity.wake_if_timed_out(&self.rgb_service);
                    }
                    rgb.set_brightness(brightness_pct)
                        .map_err(|e| DaemonError::IoError(e.to_string()))?;
                    let _ = self.rgb_service.set_brightness(brightness);
                    self.inactivity.record_activity(&self.rgb_service);
                    Ok(())
                } else {
                    Err(DaemonError::CapabilityUnavailable("RGB driver missing".to_string()).into())
                }
            }
        }
    }

    #[zbus(name = "GetRgbTimeout")]
    async fn get_rgb_timeout(&self) -> zbus::fdo::Result<u32> {
        Ok(self.inactivity.timeout_seconds.load(Ordering::Relaxed))
    }

    #[zbus(name = "SetRgbTimeout")]
    async fn set_rgb_timeout(
        &self,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &Connection,
        seconds: u32,
    ) -> zbus::fdo::Result<()> {
        let sender = header.sender().ok_or_else(|| {
            zbus::fdo::Error::from(DaemonError::PermissionDenied("No sender found".to_string()))
        })?;

        check_polkit(conn, sender, "io.strixwolf.alatus.set-rgb")
            .await
            .map_err(zbus::fdo::Error::from)?;

        tracing::info!("D-Bus: Setting RGB inactivity timeout to {seconds} seconds");
        self.inactivity
            .timeout_seconds
            .store(seconds, Ordering::Relaxed);
        self.inactivity.record_activity(&self.rgb_service);
        Ok(())
    }

    #[zbus(name = "GetRgbTimeoutPolicy")]
    async fn get_rgb_timeout_policy(&self) -> zbus::fdo::Result<String> {
        Ok(self.inactivity.get_policy().to_string())
    }

    #[zbus(name = "SetRgbTimeoutPolicy")]
    async fn set_rgb_timeout_policy(
        &self,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &Connection,
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
            let state = self.state.lock().await;
            state.on_ac
        };

        tracing::info!("D-Bus: Setting RGB inactivity timeout policy to '{policy}'");
        self.inactivity.set_policy(policy, &self.rgb_service, on_ac);
        Ok(())
    }

    #[zbus(name = "WakeRgb")]
    async fn wake_rgb(&self) -> zbus::fdo::Result<()> {
        self.inactivity.wake(&self.rgb_service);
        Ok(())
    }

    #[zbus(name = "NotifyActivity")]
    async fn notify_activity(&self) -> zbus::fdo::Result<()> {
        self.inactivity.record_activity(&self.rgb_service);
        Ok(())
    }

    #[zbus(name = "GetHardwareDiagnostics")]
    async fn get_hardware_diagnostics(&self) -> zbus::fdo::Result<(bool, String)> {
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
}

pub async fn run_background_listener(
    conn: Connection,
    state: Arc<Mutex<DaemonState>>,
    rgb_service: Arc<crate::services::rgb::RgbService>,
    inactivity: Arc<InactivityState>,
) -> Result<(), zbus::Error> {
    use futures_util::StreamExt;
    use std::time::Duration;
    use zbus::MatchRule;

    let rule_login = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender("org.freedesktop.login1")?
        .interface("org.freedesktop.login1.Manager")?
        .member("PrepareForSleep")?
        .path("/org/freedesktop/login1")?
        .build();

    async fn process_power_check(
        state: &Arc<Mutex<DaemonState>>,
        conn: &Connection,
        inactivity: &Arc<InactivityState>,
        rgb_service: &Arc<crate::services::rgb::RgbService>,
    ) {
        let current_on_ac = check_is_on_ac();
        let (policy_engine, device_context, auto_thermal, changed) = {
            let mut s = state.lock().await;
            let changed = s.on_ac != current_on_ac;
            if changed {
                s.on_ac = current_on_ac;
            }
            (
                Arc::clone(&s.policy_engine),
                Arc::clone(&s.device_context),
                s.auto_thermal_profile,
                changed,
            )
        };

        if changed {
            tracing::info!("Power source transition detected: OnAC = {current_on_ac}");

            // If AC connected and policy is BatteryOnly, immediately wake backlight
            if current_on_ac && inactivity.get_policy() == RgbTimeoutPolicy::BatteryOnly {
                inactivity.wake_if_timed_out(rgb_service);
            }

            if let Ok(interface_ref) = conn
                .object_server()
                .interface::<_, DaemonInterface>("/io/strixwolf/alatus/Daemon")
                .await
            {
                let emitter = interface_ref.signal_emitter();
                let _ = DaemonInterface::power_source_changed(emitter, current_on_ac).await;
                let _ = interface_ref.get().await.on_ac_changed(emitter).await;
            }

            if auto_thermal {
                let decision = {
                    let mut engine = policy_engine.lock().await;
                    engine.evaluate_event(crate::daemon::policy::SystemEvent::AcStateChanged(
                        current_on_ac,
                    ))
                };

                if let Some(decision) = decision {
                    let mut ctx = device_context.lock().await;
                    if crate::daemon::policy::SafetyGate::dispatch(&decision, &mut ctx)
                        && let Ok(interface_ref) = conn
                            .object_server()
                            .interface::<_, DaemonInterface>("/io/strixwolf/alatus/Daemon")
                            .await
                    {
                        let emitter = interface_ref.signal_emitter();
                        let _ = interface_ref
                            .get()
                            .await
                            .firmware_mode_changed(emitter)
                            .await;
                        if let crate::daemon::policy::PolicyAction::SetThermalMode(mode) =
                            decision.action
                        {
                            let _ =
                                DaemonInterface::thermal_mode_changed(emitter, mode.as_u32()).await;
                        }
                    }
                }
            }
        }

        // Battery level monitoring for hysteresis
        let current_battery_level =
            crate::services::telemetry::read_battery_telemetry().map(|t| t.capacity as u8);
        if let Some(level) = current_battery_level {
            let decision = {
                let mut engine = policy_engine.lock().await;
                engine.evaluate_event(crate::daemon::policy::SystemEvent::BatteryLevelChanged(
                    level,
                ))
            };

            if let Some(decision) = decision {
                let mut ctx = device_context.lock().await;
                if crate::daemon::policy::SafetyGate::dispatch(&decision, &mut ctx)
                    && let Ok(interface_ref) = conn
                        .object_server()
                        .interface::<_, DaemonInterface>("/io/strixwolf/alatus/Daemon")
                        .await
                {
                    let emitter = interface_ref.signal_emitter();
                    let _ = interface_ref
                        .get()
                        .await
                        .firmware_mode_changed(emitter)
                        .await;
                    if let crate::daemon::policy::PolicyAction::SetThermalMode(mode) =
                        decision.action
                    {
                        let _ = DaemonInterface::thermal_mode_changed(emitter, mode.as_u32()).await;
                    }
                }
            }
        }
    }

    let mut stream_login = zbus::MessageStream::for_match_rule(rule_login, &conn, Some(16)).await?;
    let mut uevent_rx = crate::services::power_uevent::spawn_power_uevent_listener();
    let mut ticker = tokio::time::interval(Duration::from_secs(1));

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let current_on_ac = { state.lock().await.on_ac };
                process_power_check(&state, &conn, &inactivity, &rgb_service).await;
                inactivity.check_timeout(&rgb_service, current_on_ac);
            }
            Some(()) = async {
                match uevent_rx.as_mut() {
                    Some(rx) => rx.recv().await,
                    None => futures_util::future::pending().await,
                }
            } => {
                process_power_check(&state, &conn, &inactivity, &rgb_service).await;
            }
            Some(msg_res) = stream_login.next() => {
                let Ok(msg) = msg_res else { continue };
                let header = msg.header();
                let (Some(interface), Some(member)) = (header.interface(), header.member()) else {
                    continue;
                };

                if interface == "org.freedesktop.login1.Manager"
                    && member == "PrepareForSleep"
                    && let Ok(false) = msg.body().deserialize::<bool>()
                {
                    tracing::info!(
                        "System resumed from sleep (PrepareForSleep=false). Scheduling hardware restoration..."
                    );

                    let rgb_clone = Arc::clone(&rgb_service);
                    let state_clone = Arc::clone(&state);
                    let conn_clone = conn.clone();
                    let inactivity_clone = Arc::clone(&inactivity);
                    tokio::spawn(async move {
                        // Delay 1s to allow USB host controller and hidraw nodes to re-enumerate
                        tokio::time::sleep(Duration::from_millis(1000)).await;

                        inactivity_clone.record_activity(&rgb_clone);
                        let _ = crate::services::rgb::wmi_unlock();
                        if let Err(e) = rgb_clone.reset_and_reapply() {
                            tracing::warn!("Failed to re-apply RGB state on resume: {e}");
                        }

                        let (limit, policy_engine, device_context) = {
                            let s = state_clone.lock().await;
                            (
                                s.last_charge_limit,
                                Arc::clone(&s.policy_engine),
                                Arc::clone(&s.device_context),
                            )
                        };

                        if let Some(limit) = limit {
                            let threshold = if limit == 0 {
                                crate::domain::ChargeThreshold::new(100).ok()
                            } else {
                                crate::domain::ChargeThreshold::new(limit as u8).ok()
                            };
                            if let Some(threshold) = threshold {
                                for attempt in 1..=5 {
                                    let mut ctx = device_context.lock().await;
                                    if let Some(ref mut bat) = ctx.battery {
                                        if bat.set_charge_threshold(threshold).is_ok() {
                                            tracing::info!(
                                                "Re-applied charge limit of {limit}% on resume (attempt {attempt})"
                                            );
                                            break;
                                        }
                                    } else {
                                        break;
                                    }
                                    drop(ctx);
                                    tokio::time::sleep(Duration::from_millis(400)).await;
                                }
                            }
                        }

                        // Evaluate resume through PolicyEngine and dispatch via SafetyGate
                        let resume_decision = {
                            let mut engine = policy_engine.lock().await;
                            engine.evaluate_event(crate::daemon::policy::SystemEvent::ResumeFromSuspend)
                        };

                        if let Some(decision) = resume_decision {
                            for attempt in 1..=5 {
                                let mut ctx = device_context.lock().await;
                                if crate::daemon::policy::SafetyGate::dispatch(&decision, &mut ctx) {
                                    tracing::info!(
                                        "Re-applied resume policy decision {:?} (attempt {attempt})",
                                        decision
                                    );
                                    if let crate::daemon::policy::PolicyAction::SetThermalMode(mode) =
                                        decision.action
                                        && let Ok(interface_ref) = conn_clone
                                            .object_server()
                                            .interface::<_, DaemonInterface>("/io/strixwolf/alatus/Daemon")
                                            .await
                                    {
                                        let emitter = interface_ref.signal_emitter();
                                        let _ = interface_ref
                                            .get()
                                            .await
                                            .firmware_mode_changed(emitter)
                                            .await;
                                        let _ = DaemonInterface::thermal_mode_changed(
                                            emitter,
                                            mode.as_u32(),
                                        )
                                        .await;
                                    }
                                    break;
                                }
                                drop(ctx);
                                tokio::time::sleep(Duration::from_millis(400)).await;
                            }
                        }
                    });
                }
            }
        }
    }
}
