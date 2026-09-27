// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Root system daemon D-Bus service interface.

pub mod methods;
pub mod properties;

use std::sync::Arc;
use std::time::Duration;
use futures_util::StreamExt;
use tokio::sync::Mutex;
use zbus::{interface, Connection, MatchRule};

use super::inactivity::InactivityState;
use super::policy::{PolicyAction, PolicyEngine, SafetyGate, SystemEvent};
use super::power::check_is_on_ac;
use crate::hardware::DeviceContext;
use crate::services::config::RgbTimeoutPolicy;
use crate::services::rgb::RgbService;

pub struct DaemonState {
    pub device_context: Arc<Mutex<DeviceContext>>,
    pub policy_engine: Arc<Mutex<PolicyEngine>>,
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
        policy_engine: Arc<Mutex<PolicyEngine>>,
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

pub struct DaemonInterface {
    pub device_context: Arc<Mutex<DeviceContext>>,
    pub state: Arc<Mutex<DaemonState>>,
    pub policy_engine: Arc<Mutex<PolicyEngine>>,
    pub rgb_service: Arc<RgbService>,
    pub inactivity: Arc<InactivityState>,
}

impl DaemonInterface {
    pub fn new(
        device_context: Arc<Mutex<DeviceContext>>,
        state: Arc<Mutex<DaemonState>>,
        policy_engine: Arc<Mutex<PolicyEngine>>,
        rgb_service: Arc<RgbService>,
        inactivity: Arc<InactivityState>,
    ) -> Self {
        Self {
            device_context,
            state,
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
        properties::get_firmware_mode(&self.device_context).await
    }

    #[zbus(property)]
    async fn set_firmware_mode(
        &self,
        #[zbus(header)] header: Option<zbus::message::Header<'_>>,
        #[zbus(connection)] conn: &Connection,
        value: u32,
    ) -> zbus::Result<()> {
        properties::set_firmware_mode(
            &self.device_context,
            &self.state,
            &self.policy_engine,
            header,
            conn,
            value,
        )
        .await
    }

    #[zbus(property(emits_changed_signal = "false"))]
    async fn thermal_mode(&self) -> zbus::fdo::Result<u32> {
        self.firmware_mode().await
    }

    #[zbus(property)]
    async fn set_thermal_mode(
        &self,
        #[zbus(header)] header: Option<zbus::message::Header<'_>>,
        #[zbus(connection)] conn: &Connection,
        value: u32,
    ) -> zbus::Result<()> {
        self.set_firmware_mode(header, conn, value).await
    }

    #[zbus(property)]
    async fn charge_limit(&self) -> zbus::fdo::Result<u32> {
        properties::get_charge_limit(&self.device_context).await
    }

    #[zbus(property)]
    async fn set_charge_limit(
        &self,
        #[zbus(header)] header: Option<zbus::message::Header<'_>>,
        #[zbus(connection)] conn: &Connection,
        value: u32,
    ) -> zbus::Result<()> {
        properties::set_charge_limit(&self.device_context, &self.state, header, conn, value).await
    }

    #[zbus(property)]
    async fn deep_sleep_active(&self) -> zbus::fdo::Result<bool> {
        properties::get_deep_sleep_active().await
    }

    #[zbus(property)]
    async fn set_deep_sleep_active(
        &self,
        #[zbus(header)] header: Option<zbus::message::Header<'_>>,
        #[zbus(connection)] conn: &Connection,
        value: bool,
    ) -> zbus::Result<()> {
        properties::set_deep_sleep_active(header, conn, value).await
    }

    #[zbus(property)]
    async fn product_serial(&self) -> zbus::fdo::Result<String> {
        properties::get_product_serial()
    }

    #[zbus(property)]
    async fn has_conflicting_power_daemon(&self) -> zbus::fdo::Result<bool> {
        properties::has_conflicting_power_daemon()
    }

    #[zbus(property)]
    async fn on_ac(&self) -> zbus::fdo::Result<bool> {
        properties::get_on_ac(&self.state).await
    }

    #[zbus(property)]
    async fn auto_thermal_profile(&self) -> zbus::fdo::Result<bool> {
        properties::get_auto_thermal_profile(&self.state).await
    }

    #[zbus(property)]
    async fn set_auto_thermal_profile(
        &self,
        #[zbus(header)] header: Option<zbus::message::Header<'_>>,
        #[zbus(connection)] conn: &Connection,
        value: bool,
    ) -> zbus::Result<()> {
        properties::set_auto_thermal_profile(&self.device_context, &self.state, header, conn, value)
            .await
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
        methods::get_capabilities(&self.device_context).await
    }

    #[zbus(name = "GetRgbStatus")]
    async fn get_rgb_status(&self) -> zbus::fdo::Result<String> {
        methods::get_rgb_status(&self.device_context, &self.rgb_service).await
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
        methods::set_rgb_color(&self.device_context, &self.rgb_service, header, conn, r, g, b).await
    }

    #[zbus(name = "SetRgbBrightness")]
    async fn set_rgb_brightness(
        &self,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &Connection,
        brightness: u32,
    ) -> zbus::fdo::Result<()> {
        methods::set_rgb_brightness(
            &self.device_context,
            &self.rgb_service,
            &self.inactivity,
            header,
            conn,
            brightness,
        )
        .await
    }

    #[zbus(name = "GetRgbTimeout")]
    async fn get_rgb_timeout(&self) -> zbus::fdo::Result<u32> {
        methods::get_rgb_timeout(&self.inactivity)
    }

    #[zbus(name = "SetRgbTimeout")]
    async fn set_rgb_timeout(
        &self,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &Connection,
        seconds: u32,
    ) -> zbus::fdo::Result<()> {
        methods::set_rgb_timeout(&self.rgb_service, &self.inactivity, header, conn, seconds).await
    }

    #[zbus(name = "GetRgbTimeoutPolicy")]
    async fn get_rgb_timeout_policy(&self) -> zbus::fdo::Result<String> {
        methods::get_rgb_timeout_policy(&self.inactivity)
    }

    #[zbus(name = "SetRgbTimeoutPolicy")]
    async fn set_rgb_timeout_policy(
        &self,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &Connection,
        policy_str: String,
    ) -> zbus::fdo::Result<()> {
        methods::set_rgb_timeout_policy(
            &self.state,
            &self.rgb_service,
            &self.inactivity,
            header,
            conn,
            policy_str,
        )
        .await
    }

    #[zbus(name = "WakeRgb")]
    async fn wake_rgb(&self) -> zbus::fdo::Result<()> {
        methods::wake_rgb(&self.rgb_service, &self.inactivity)
    }

    #[zbus(name = "NotifyActivity")]
    async fn notify_activity(&self) -> zbus::fdo::Result<()> {
        methods::notify_activity(&self.rgb_service, &self.inactivity)
    }

    #[zbus(name = "GetHardwareDiagnostics")]
    async fn get_hardware_diagnostics(&self) -> zbus::fdo::Result<(bool, String)> {
        methods::get_hardware_diagnostics()
    }
}

pub async fn run_background_listener(
    conn: Connection,
    state: Arc<Mutex<DaemonState>>,
    rgb_service: Arc<RgbService>,
    inactivity: Arc<InactivityState>,
) -> Result<(), zbus::Error> {
    let rule_login = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.login1.Manager")?
        .member("PrepareForSleep")?
        .path("/org/freedesktop/login1")?
        .build();

    async fn process_power_check(
        state: &Arc<Mutex<DaemonState>>,
        conn: &Connection,
        inactivity: &Arc<InactivityState>,
        rgb_service: &Arc<RgbService>,
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
                    engine.evaluate_event(SystemEvent::AcStateChanged(
                        current_on_ac,
                    ))
                };

                if let Some(decision) = decision {
                    let dispatched = {
                        let mut ctx = device_context.lock().await;
                        SafetyGate::dispatch(&decision, &mut ctx)
                    };

                    if dispatched
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
                        if let PolicyAction::SetThermalMode(mode) =
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
                engine.evaluate_event(SystemEvent::BatteryLevelChanged(
                    level,
                ))
            };

            if let Some(decision) = decision {
                let dispatched = {
                    let mut ctx = device_context.lock().await;
                    SafetyGate::dispatch(&decision, &mut ctx)
                };

                if dispatched
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
                    if let PolicyAction::SetThermalMode(mode) =
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
                {
                    let is_sleeping = msg
                        .body()
                        .deserialize::<(bool,)>()
                        .map(|(b,)| b)
                        .or_else(|_| msg.body().deserialize::<bool>());

                    if let Ok(true) = is_sleeping {
                        tracing::info!("System is preparing for sleep (PrepareForSleep=true).");
                    } else if let Ok(false) = is_sleeping {
                        tracing::info!(
                            "System resumed from sleep (PrepareForSleep=false). Scheduling hardware restoration..."
                        );

                        // Reset inactivity timer immediately to prevent stale timestamps triggering instant dimming
                        inactivity.reset_activity_timer();

                        let rgb_clone = Arc::clone(&rgb_service);
                        let state_clone = Arc::clone(&state);
                        let conn_clone = conn.clone();
                        let inactivity_clone = Arc::clone(&inactivity);
                        tokio::spawn(async move {
                            // Delay 800ms to allow USB host controller and hidraw nodes to settle
                            tokio::time::sleep(Duration::from_millis(800)).await;

                            // Reset inactivity timer again after settling delay
                            inactivity_clone.reset_activity_timer();

                            // 1. Reset fan and thermal telemetry read handles / cache
                            crate::services::telemetry::reset_telemetry_cache();

                            // 2. Hardware re-enumeration on DeviceContext
                            let (limit, policy_engine, device_context) = {
                                let s = state_clone.lock().await;
                                (
                                    s.last_charge_limit,
                                    Arc::clone(&s.policy_engine),
                                    Arc::clone(&s.device_context),
                                )
                            };

                            {
                                let mut ctx = device_context.lock().await;
                                ctx.re_enumerate();
                                tracing::info!(
                                    "Hardware drivers re-enumerated on resume: thermal={:?}, battery={:?}, rgb={:?}",
                                    ctx.capabilities.thermal,
                                    ctx.capabilities.battery,
                                    ctx.capabilities.rgb
                                );
                            }

                            // 3. Delayed & Retried RGB Restoration on Resume
                            let (active_color, active_brightness) = {
                                let (r, g, b) = rgb_clone.get_color();
                                let bri = rgb_clone.get_brightness();
                                (
                                    crate::domain::ColorRgb::new(r, g, b),
                                    crate::domain::BrightnessPercent::new(bri as u8)
                                        .unwrap_or_else(|_| crate::domain::BrightnessPercent::new(80).unwrap()),
                                )
                            };

                            for attempt in 1..=4 {
                                let mut ctx = device_context.lock().await;
                                ctx.re_enumerate_rgb();
                                if let Ok(rgb) = ctx.rgb_mut() {
                                    let _ = rgb.wake();
                                    if rgb.set_color(active_color).is_ok()
                                        && rgb.set_brightness(active_brightness).is_ok()
                                    {
                                        tracing::info!(
                                            "Successfully restored RGB state after resume on attempt {attempt}: color={:?}, brightness={}%",
                                            active_color,
                                            active_brightness.value()
                                        );
                                        let _ = rgb_clone.wake();
                                        let _ = rgb_clone.set_color(active_color.r, active_color.g, active_color.b);
                                        let _ = rgb_clone.set_brightness(active_brightness.value() as u32);
                                        break;
                                    }
                                }
                                drop(ctx);
                                tokio::time::sleep(Duration::from_millis(300)).await;
                            }

                            // Refresh inactivity timer once restoration finishes
                            inactivity_clone.reset_activity_timer();

                            // 4. Re-apply charge limit with retry loop
                            if let Some(limit) = limit {
                                let threshold = if limit == 0 {
                                    crate::domain::ChargeThreshold::new(100).ok()
                                } else {
                                    crate::domain::ChargeThreshold::new(limit as u8).ok()
                                };
                                if let Some(threshold) = threshold {
                                    for attempt in 1..=5 {
                                        let mut ctx = device_context.lock().await;
                                        if let Ok(bat) = ctx.battery_mut()
                                            && bat.set_charge_threshold(threshold).is_ok()
                                        {
                                            tracing::info!(
                                                "Re-applied charge limit of {limit}% on resume (attempt {attempt})"
                                            );
                                            break;
                                        }
                                        drop(ctx);
                                        tokio::time::sleep(Duration::from_millis(400)).await;
                                    }
                                }
                            }

                            // 5. Evaluate resume through PolicyEngine and dispatch via SafetyGate
                            let resume_decision = {
                                let mut engine = policy_engine.lock().await;
                                engine.evaluate_event(SystemEvent::ResumeFromSuspend)
                            };

                            if let Some(decision) = resume_decision {
                                for attempt in 1..=5 {
                                    let dispatched = {
                                        let mut ctx = device_context.lock().await;
                                        SafetyGate::dispatch(&decision, &mut ctx)
                                    };

                                    if dispatched {
                                        tracing::info!(
                                            "Re-applied resume policy decision {:?} (attempt {attempt})",
                                            decision
                                        );
                                        if let PolicyAction::SetThermalMode(mode) =
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
                                    tokio::time::sleep(Duration::from_millis(400)).await;
                                }
                            }
                        });
                    }
                }
            }
        }
    }
}
