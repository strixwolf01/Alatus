// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use alatus::services::firmware_mode;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use zbus::{Connection, interface};

const SYS_MEM_SLEEP: &str = "/sys/power/mem_sleep";
const SYS_CHARGE_THRESHOLDS: &[&str] = &[
    "/sys/class/power_supply/BAT0/charge_control_end_threshold",
    "/sys/class/power_supply/BAT1/charge_control_end_threshold",
    "/sys/class/power_supply/BATC/charge_control_end_threshold",
    "/sys/class/power_supply/BATT/charge_control_end_threshold",
];

fn find_charge_threshold_path() -> Option<PathBuf> {
    let power_supply_dir = Path::new("/sys/class/power_supply");
    if let Ok(entries) = fs::read_dir(power_supply_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let type_str = fs::read_to_string(path.join("type")).unwrap_or_default();
            if type_str.trim().eq_ignore_ascii_case("Battery") {
                let threshold_path = path.join("charge_control_end_threshold");
                if threshold_path.exists() {
                    return Some(threshold_path);
                }
            }
        }
    }

    // Fallback search across standard paths
    for path in SYS_CHARGE_THRESHOLDS {
        let p = Path::new(path);
        if p.exists() {
            return Some(p.to_path_buf());
        }
    }
    None
}

#[derive(Debug)]
enum DaemonError {
    InvalidArgument(String),
    PermissionDenied(String),
    NotSupported(String),
    IoError(String),
    BackendUnavailable(String),
}

impl std::fmt::Display for DaemonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidArgument(msg) => write!(f, "InvalidArgument: {msg}"),
            Self::PermissionDenied(msg) => write!(f, "PermissionDenied: {msg}"),
            Self::NotSupported(msg) => write!(f, "NotSupported: {msg}"),
            Self::IoError(msg) => write!(f, "IoError: {msg}"),
            Self::BackendUnavailable(msg) => write!(f, "BackendUnavailable: {msg}"),
        }
    }
}

impl std::error::Error for DaemonError {}

impl From<DaemonError> for zbus::fdo::Error {
    fn from(e: DaemonError) -> Self {
        match e {
            DaemonError::InvalidArgument(msg) => zbus::fdo::Error::InvalidArgs(msg),
            DaemonError::PermissionDenied(msg) => zbus::fdo::Error::Failed(msg),
            DaemonError::NotSupported(msg) => zbus::fdo::Error::NotSupported(msg),
            DaemonError::IoError(msg) => zbus::fdo::Error::Failed(msg),
            DaemonError::BackendUnavailable(msg) => zbus::fdo::Error::Failed(msg),
        }
    }
}

impl From<DaemonError> for zbus::Error {
    fn from(e: DaemonError) -> Self {
        zbus::Error::from(zbus::fdo::Error::from(e))
    }
}

fn check_is_on_ac() -> bool {
    let power_supply_dir = Path::new("/sys/class/power_supply");
    if let Ok(entries) = fs::read_dir(power_supply_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let type_str = fs::read_to_string(path.join("type")).unwrap_or_default();
            if type_str.trim().eq_ignore_ascii_case("Battery") {
                continue;
            }
            let online_str = fs::read_to_string(path.join("online")).unwrap_or_default();
            if online_str.trim() == "1" {
                return true;
            }
        }
    }
    false
}

struct DaemonState {
    last_charge_limit: Option<u32>,
    last_firmware_mode: Option<u32>,
    on_ac: bool,
    auto_thermal_profile: bool,
    ac_thermal_mode: u32,
    battery_thermal_mode: u32,
}

fn check_conflicting_power_daemons_sync() -> bool {
    // 1. Check procfs for running daemon processes (e.g. tlp)
    if let Ok(entries) = fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Ok(comm) = fs::read_to_string(path.join("comm")) {
                let comm = comm.trim();
                if comm == "tlp" {
                    return true;
                }
            }
        }
    }

    // 2. Check /run/tlp or active markers
    if Path::new("/run/tlp/run").exists() || Path::new("/run/tlp").exists() {
        return true;
    }

    // 3. Fallback to systemctl check if available
    if let Ok(output) = std::process::Command::new("systemctl")
        .args(["is-active", "tlp.service"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.lines().any(|line| line.trim() == "active") {
            return true;
        }
    }

    false
}

fn read_product_serial() -> String {
    fs::read_to_string("/sys/class/dmi/id/product_serial")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "Unavailable".to_string())
}

#[derive(Clone)]
struct DaemonInterface {
    state: Arc<Mutex<DaemonState>>,
    rgb_service: Arc<alatus::services::rgb::RgbService>,
}

fn read_charge_limit() -> Result<u32, DaemonError> {
    let path = find_charge_threshold_path().ok_or_else(|| {
        DaemonError::NotSupported("Charge control limit end threshold not supported".to_string())
    })?;
    let s = fs::read_to_string(path)
        .map_err(|e| DaemonError::IoError(format!("Read charge limit failed: {e}")))?;
    s.trim()
        .parse::<u32>()
        .map_err(|e| DaemonError::IoError(format!("Parse charge limit failed: {e}")))
}

fn validate_charge_limit(value: u32) -> Result<u32, DaemonError> {
    if value > 100 {
        Err(DaemonError::InvalidArgument(
            "Charge limit must be 0..=100".to_string(),
        ))
    } else {
        Ok(value)
    }
}

fn write_charge_limit(value: u32) -> Result<(), DaemonError> {
    validate_charge_limit(value)?;
    let path = find_charge_threshold_path().ok_or_else(|| {
        DaemonError::NotSupported("Charge control limit end threshold not supported".to_string())
    })?;
    tracing::info!("Writing charge limit: {value}% to {}", path.display());
    fs::write(path, format!("{value}\n"))
        .map_err(|e| DaemonError::IoError(format!("Write charge limit failed: {e}")))
}

fn parse_mem_sleep_active(content: &str) -> bool {
    content.contains("[deep]")
}

fn is_mem_sleep_mode_supported(content: &str, mode: &str) -> bool {
    content.split_whitespace().any(|w| {
        let clean = w.trim_matches(|c| c == '[' || c == ']');
        clean == mode
    })
}

fn read_deep_sleep() -> Result<bool, DaemonError> {
    if !Path::new(SYS_MEM_SLEEP).exists() {
        return Err(DaemonError::NotSupported(
            "Deep sleep configuration not supported".to_string(),
        ));
    }
    let s = fs::read_to_string(SYS_MEM_SLEEP)
        .map_err(|e| DaemonError::IoError(format!("Read mem_sleep failed: {e}")))?;
    Ok(parse_mem_sleep_active(&s))
}

fn write_deep_sleep(active: bool) -> Result<(), DaemonError> {
    if !Path::new(SYS_MEM_SLEEP).exists() {
        return Err(DaemonError::NotSupported(
            "Deep sleep configuration not supported".to_string(),
        ));
    }
    let s = fs::read_to_string(SYS_MEM_SLEEP)
        .map_err(|e| DaemonError::IoError(format!("Read mem_sleep failed: {e}")))?;

    let target = if active { "deep" } else { "s2idle" };
    if !is_mem_sleep_mode_supported(&s, target) {
        return Err(DaemonError::NotSupported(format!(
            "Sleep mode '{target}' is not supported by the kernel"
        )));
    }

    fs::write(SYS_MEM_SLEEP, target)
        .map_err(|e| DaemonError::IoError(format!("Write mem_sleep failed: {e}")))
}

fn map_dbus_value_to_mode(value: u32) -> Result<firmware_mode::FirmwareMode, DaemonError> {
    match value {
        0 => Ok(firmware_mode::FirmwareMode::Balanced),
        1 => Ok(firmware_mode::FirmwareMode::Quiet),
        2 => Ok(firmware_mode::FirmwareMode::High),
        3 => Ok(firmware_mode::FirmwareMode::Full),
        _ => Err(DaemonError::InvalidArgument(
            "Invalid firmware mode value (must be 0..=3)".to_string(),
        )),
    }
}

fn map_mode_to_dbus_value(mode: firmware_mode::FirmwareMode) -> u32 {
    match mode {
        firmware_mode::FirmwareMode::Balanced => 0,
        firmware_mode::FirmwareMode::Quiet => 1,
        firmware_mode::FirmwareMode::High => 2,
        firmware_mode::FirmwareMode::Full => 3,
        firmware_mode::FirmwareMode::Unknown(n) => n as u32,
    }
}

fn read_wmi_firmware_mode() -> Result<u32, DaemonError> {
    match firmware_mode::read_firmware_mode() {
        Ok(mode) => Ok(map_mode_to_dbus_value(mode)),
        Err(e) => Err(DaemonError::IoError(e.to_string())),
    }
}

fn write_wmi_firmware_mode(value: u32) -> Result<(), DaemonError> {
    let mode = map_dbus_value_to_mode(value)?;
    tracing::info!("Writing firmware mode: {mode:?} (dbus value: {value})");
    match firmware_mode::set_firmware_mode_direct(mode) {
        Ok(res) => {
            tracing::info!(
                "Firmware mode updated successfully: requested={:?}, actual={:?}, status={:?}",
                res.requested,
                res.actual,
                res.status
            );
            Ok(())
        }
        Err(e) => {
            tracing::error!("Failed to set firmware mode: {e}");
            Err(DaemonError::IoError(e.to_string()))
        }
    }
}

fn parse_polkit_result(is_authorized: bool) -> Result<(), DaemonError> {
    if is_authorized {
        Ok(())
    } else {
        Err(DaemonError::PermissionDenied(
            "Polkit authorization failed".to_string(),
        ))
    }
}

async fn check_polkit(
    conn: &Connection,
    sender: &zbus::names::UniqueName<'_>,
    action_id: &str,
) -> Result<(), DaemonError> {
    let mut details = HashMap::new();
    details.insert(
        "name".to_string(),
        zbus::zvariant::Value::from(sender.as_str()),
    );

    let subject = ("system-bus-name".to_string(), details);
    let empty_details: HashMap<String, String> = HashMap::new();
    let flags: u32 = 1; // AllowUserInteraction
    let cancellation_id = "";

    let proxy = match zbus::Proxy::new(
        conn,
        "org.freedesktop.PolicyKit1",
        "/org/freedesktop/PolicyKit1/Authority",
        "org.freedesktop.PolicyKit1.Authority",
    )
    .await
    {
        Ok(p) => p,
        Err(e) => {
            return Err(DaemonError::BackendUnavailable(format!(
                "PolicyKit not available: {e}"
            )));
        }
    };

    let response: Result<(bool, bool, HashMap<String, String>), zbus::Error> = proxy
        .call(
            "CheckAuthorization",
            &(subject, action_id, empty_details, flags, cancellation_id),
        )
        .await;

    match response {
        Ok((is_authorized, _, _)) => parse_polkit_result(is_authorized),
        Err(e) => {
            tracing::error!("Polkit check failed with D-Bus error: {e}");
            Err(DaemonError::PermissionDenied(format!(
                "Polkit communication failed: {e}"
            )))
        }
    }
}

#[interface(name = "io.strixwolf.alatus.Daemon")]
impl DaemonInterface {
    #[zbus(property)]
    async fn firmware_mode(&self) -> zbus::fdo::Result<u32> {
        read_wmi_firmware_mode().map_err(Into::into)
    }

    #[zbus(property)]
    async fn set_firmware_mode(
        &self,
        #[zbus(header)] header: Option<zbus::message::Header<'_>>,
        #[zbus(connection)] conn: &Connection,
        value: u32,
    ) -> zbus::Result<()> {
        if value > 3 {
            return Err(zbus::Error::from(DaemonError::InvalidArgument(
                "Firmware mode must be 0..=3".to_string(),
            )));
        }
        let sender = header
            .and_then(|h| h.sender().map(|s| s.to_owned()))
            .ok_or_else(|| {
                zbus::Error::from(DaemonError::PermissionDenied("No sender found".to_string()))
            })?;

        check_polkit(conn, &sender, "io.strixwolf.alatus.set-firmware-mode").await?;
        write_wmi_firmware_mode(value)?;

        {
            let mut s = self.state.lock().await;
            s.last_firmware_mode = Some(value);
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
        read_charge_limit().map_err(Into::into)
    }

    #[zbus(property)]
    async fn set_charge_limit(
        &self,
        #[zbus(header)] header: Option<zbus::message::Header<'_>>,
        #[zbus(connection)] conn: &Connection,
        value: u32,
    ) -> zbus::Result<()> {
        validate_charge_limit(value).map_err(zbus::Error::from)?;
        let sender = header
            .and_then(|h| h.sender().map(|s| s.to_owned()))
            .ok_or_else(|| {
                zbus::Error::from(DaemonError::PermissionDenied("No sender found".to_string()))
            })?;

        check_polkit(conn, &sender, "io.strixwolf.alatus.set-charge-limit").await?;
        write_charge_limit(value)?;

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
            let _ = write_wmi_firmware_mode(target_mode);
            interface_ref
                .get()
                .await
                .firmware_mode_changed(interface_ref.signal_emitter())
                .await?;
        }

        Ok(())
    }

    #[zbus(signal)]
    async fn power_source_changed(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        on_ac: bool,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn thermal_mode_changed(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        mode: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn thermal_osd_triggered(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        mode: u32,
    ) -> zbus::Result<()>;

    #[zbus(name = "GetRgbStatus")]
    async fn get_rgb_status(&self) -> zbus::fdo::Result<String> {
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
        self.rgb_service
            .set_color(r, g, b)
            .map_err(|e| zbus::fdo::Error::from(DaemonError::IoError(e)))?;
        Ok(())
    }

    #[zbus(name = "SetRgbBrightness")]
    async fn set_rgb_brightness(
        &self,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &Connection,
        brightness: u32,
    ) -> zbus::fdo::Result<()> {
        if brightness > 100 {
            return Err(zbus::fdo::Error::from(DaemonError::InvalidArgument(
                "Brightness must be between 0 and 100".to_string(),
            )));
        }
        let sender = header.sender().ok_or_else(|| {
            zbus::fdo::Error::from(DaemonError::PermissionDenied("No sender found".to_string()))
        })?;

        check_polkit(conn, sender, "io.strixwolf.alatus.set-rgb")
            .await
            .map_err(zbus::fdo::Error::from)?;
        self.rgb_service
            .set_brightness(brightness)
            .map_err(|e| zbus::fdo::Error::from(DaemonError::IoError(e)))?;
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

async fn run_background_listener(
    conn: Connection,
    state: Arc<Mutex<DaemonState>>,
    rgb_service: Arc<alatus::services::rgb::RgbService>,
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

    async fn process_power_check(state: &Arc<Mutex<DaemonState>>, conn: &Connection) {
        let current_on_ac = check_is_on_ac();
        let mut s = state.lock().await;
        if s.on_ac != current_on_ac {
            s.on_ac = current_on_ac;
            let auto_thermal = s.auto_thermal_profile;
            let target_mode = if current_on_ac {
                s.ac_thermal_mode
            } else {
                s.battery_thermal_mode
            };
            drop(s);

            tracing::info!("Power source transition detected: OnAC = {current_on_ac}");

            if let Ok(interface_ref) = conn
                .object_server()
                .interface::<_, DaemonInterface>("/io/strixwolf/alatus/Daemon")
                .await
            {
                let emitter = interface_ref.signal_emitter();
                let _ = DaemonInterface::power_source_changed(emitter, current_on_ac).await;
                let _ = interface_ref.get().await.on_ac_changed(emitter).await;

                if auto_thermal {
                    tracing::info!(
                        "Auto-thermal profile: switching to mode {target_mode} (OnAC={current_on_ac})"
                    );
                    let _ = write_wmi_firmware_mode(target_mode);
                    let _ = interface_ref
                        .get()
                        .await
                        .firmware_mode_changed(emitter)
                        .await;
                }
            }
        }
    }

    let mut stream_login = zbus::MessageStream::for_match_rule(rule_login, &conn, Some(16)).await?;
    let mut uevent_rx = alatus::services::power_uevent::spawn_power_uevent_listener();
    let mut ticker = tokio::time::interval(Duration::from_secs(2));

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                process_power_check(&state, &conn).await;
            }
            Some(()) = async {
                match uevent_rx.as_mut() {
                    Some(rx) => rx.recv().await,
                    None => futures_util::future::pending().await,
                }
            } => {
                process_power_check(&state, &conn).await;
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
                        "System resumed from sleep (PrepareForSleep=false). Restoring hardware state..."
                    );

                    // 1. Re-run WMI unlock handshake & re-initialize RGB controller
                    let _ = alatus::services::rgb::wmi_unlock();
                    if let Err(e) = rgb_service.reset_and_reapply() {
                        tracing::warn!("Failed to re-apply RGB state on resume: {e}");
                    }

                    // 2. Restore charge limit and firmware mode
                    let (limit, target_mode) = {
                        let s = state.lock().await;
                        let mode = if s.auto_thermal_profile {
                            if s.on_ac {
                                s.ac_thermal_mode
                            } else {
                                s.battery_thermal_mode
                            }
                        } else {
                            s.last_firmware_mode
                                .or_else(|| read_wmi_firmware_mode().ok())
                                .unwrap_or(0)
                        };
                        (s.last_charge_limit, mode)
                    };

                    if let Some(limit) = limit {
                        tracing::info!(
                            "System resumed. Re-applying cached charge limit of {}%",
                            limit
                        );
                        if let Err(e) = write_charge_limit(limit) {
                            tracing::error!("Failed to re-apply charge limit on resume: {e:?}");
                        }
                    }

                    tracing::info!(
                        "System resumed. Restoring firmware/thermal profile: {target_mode}"
                    );
                    if let Err(e) = write_wmi_firmware_mode(target_mode) {
                        tracing::error!("Failed to re-apply firmware mode on resume: {e}");
                    } else if let Ok(interface_ref) = conn
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
                        let _ = DaemonInterface::thermal_mode_changed(emitter, target_mode).await;
                    }
                }
            }
        }
    }
}

fn find_keyboard_and_hotkey_devices() -> Vec<(PathBuf, String)> {
    let mut devices = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let mut add_device = |path: PathBuf, name: String| {
        if path.exists() {
            let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
            if seen.insert(canonical.clone()) {
                devices.push((canonical, name));
            }
        }
    };

    // 1. Check known by-path symlinks first
    let asus_wmi = PathBuf::from("/dev/input/by-path/platform-asus-nb-wmi-event");
    if asus_wmi.exists() {
        add_device(asus_wmi, "Asus WMI hotkeys".to_string());
    }

    let at_kbd = PathBuf::from("/dev/input/by-path/platform-i8042-serio-0-event-kbd");
    if at_kbd.exists() {
        add_device(at_kbd, "AT Translated Set 2 keyboard".to_string());
    }

    // 2. Scan /sys/class/input/event* for matching devices
    if let Ok(entries) = fs::read_dir("/sys/class/input") {
        for entry in entries.flatten() {
            let fname = entry.file_name();
            let name_str = fname.to_string_lossy();
            if name_str.starts_with("event") {
                let dev_node = PathBuf::from("/dev/input").join(&fname);
                let sysfs_name_path = entry.path().join("device/name");
                if let Ok(dev_name) = fs::read_to_string(&sysfs_name_path) {
                    let trimmed = dev_name.trim().to_string();
                    if trimmed.contains("Asus WMI hotkeys")
                        || trimmed.contains("asus-nb-wmi")
                        || trimmed.contains("keyboard")
                        || trimmed.contains("Keyboard")
                    {
                        add_device(dev_node, trimmed);
                    }
                }
            }
        }
    }

    devices
}

async fn trigger_osd_notification(mode: u32) {
    let (summary, body, icon) = match mode {
        0 => (
            "Balanced Mode",
            "Standard acoustic and power profile applied.",
            "power-profile-balanced-symbolic",
        ),
        1 => (
            "Quiet Mode",
            "Silent fan curves and energy-saving profile applied.",
            "power-profile-power-saver-symbolic",
        ),
        2 => (
            "Performance Mode",
            "High boost clocks and dynamic cooling applied.",
            "power-profile-performance-symbolic",
        ),
        3 => (
            "Full Speed Mode",
            "Maximum cooling and sustained high performance.",
            "power-profile-performance-symbolic",
        ),
        _ => (
            "Thermal Mode",
            "Profile updated.",
            "power-profile-balanced-symbolic",
        ),
    };

    // Scan /run/user for active user D-Bus session sockets
    if let Ok(entries) = fs::read_dir("/run/user") {
        for entry in entries.flatten() {
            let bus_path = entry.path().join("bus");
            if bus_path.exists() {
                let address = format!("unix:path={}", bus_path.display());
                if let Ok(user_conn) = zbus::connection::Builder::address(address.as_str())
                    && let Ok(conn) = user_conn.build().await
                {
                    let _ = alatus::services::desktop_session::send_osd_notification(
                        &conn, summary, body, icon,
                    )
                    .await;
                }
            }
        }
    }
}

async fn handle_fn_f_hotkey(conn: &Connection, state: &Arc<Mutex<DaemonState>>) {
    let cur = read_wmi_firmware_mode().unwrap_or(0);
    // Cycle order: Quiet (1) -> Balanced (0) -> Performance (2) -> Full (3) -> Quiet (1)
    let next_mode = match cur {
        1 => 0, // Quiet -> Balanced
        0 => 2, // Balanced -> Performance
        2 => 3, // Performance -> Full
        3 => 1, // Full -> Quiet
        _ => 0, // Default to Balanced
    };

    tracing::info!("Fn+F hotkey cycling thermal mode: {cur} -> {next_mode}");
    if let Err(e) = write_wmi_firmware_mode(next_mode) {
        tracing::error!("Failed to write cycled firmware mode: {e}");
        return;
    }

    {
        let mut s = state.lock().await;
        s.last_firmware_mode = Some(next_mode);
    }

    // Broadcast D-Bus signals
    if let Ok(interface_ref) = conn
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
        let _ = DaemonInterface::thermal_mode_changed(emitter, next_mode).await;
        let _ = DaemonInterface::thermal_osd_triggered(emitter, next_mode).await;
    }

    // Direct OSD notification fallback
    trigger_osd_notification(next_mode).await;
}

fn listen_to_input_device(
    path: PathBuf,
    name: String,
    conn: Connection,
    state: Arc<Mutex<DaemonState>>,
    tokio_handle: tokio::runtime::Handle,
) {
    use std::io::Read;

    loop {
        let Ok(mut file) = std::fs::File::open(&path) else {
            std::thread::sleep(std::time::Duration::from_secs(2));
            continue;
        };

        tracing::info!(
            "Listening for input events on {} ({})",
            path.display(),
            name
        );

        let mut buf = [0u8; 24];
        loop {
            match file.read_exact(&mut buf) {
                Ok(_) => {
                    let type_ = u16::from_ne_bytes([buf[16], buf[17]]);
                    let code = u16::from_ne_bytes([buf[18], buf[19]]);
                    let value = i32::from_ne_bytes([buf[20], buf[21], buf[22], buf[23]]);

                    if type_ == 1 {
                        // Log every key event at info level as requested
                        tracing::info!("Input event: code={}, val={}", code, value);

                        if value == 1
                            && (code == 148
                                || code == 202
                                || code == 203
                                || code == 482
                                || code == 582
                                || code == 190)
                        {
                            tracing::info!(
                                "ASUS Fn+F key press detected on {} (code {code})",
                                name
                            );
                            let conn_clone = conn.clone();
                            let state_clone = Arc::clone(&state);
                            tokio_handle.spawn(async move {
                                handle_fn_f_hotkey(&conn_clone, &state_clone).await;
                            });
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Input device {} ({}) disconnected or returned error: {e}. Rescanning in 2s...",
                        path.display(),
                        name
                    );
                    drop(file);
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    break;
                }
            }
        }
    }
}

fn run_asus_hotkey_listener(
    conn: Connection,
    state: Arc<Mutex<DaemonState>>,
    tokio_handle: tokio::runtime::Handle,
) {
    let devices = find_keyboard_and_hotkey_devices();
    if devices.is_empty() {
        tracing::warn!("No keyboard or ASUS hotkey event devices found for Fn+F listener");
        return;
    }

    for (path, name) in devices {
        let conn_clone = conn.clone();
        let state_clone = Arc::clone(&state);
        let handle_clone = tokio_handle.clone();
        std::thread::spawn(move || {
            listen_to_input_device(path, name, conn_clone, state_clone, handle_clone);
        });
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting alatusd");

    let connection = Connection::system().await?;

    let initial_charge_limit = read_charge_limit().ok();
    let initial_firmware_mode = read_wmi_firmware_mode().ok();
    let initial_on_ac = check_is_on_ac();

    let state = Arc::new(Mutex::new(DaemonState {
        last_charge_limit: initial_charge_limit,
        last_firmware_mode: initial_firmware_mode,
        on_ac: initial_on_ac,
        auto_thermal_profile: false,
        ac_thermal_mode: 0,
        battery_thermal_mode: 1,
    }));

    let rgb_service = Arc::new(alatus::services::rgb::RgbService::new());

    let interface = DaemonInterface {
        state: Arc::clone(&state),
        rgb_service: Arc::clone(&rgb_service),
    };

    connection
        .object_server()
        .at("/io/strixwolf/alatus/Daemon", interface)
        .await?;

    connection
        .request_name("io.strixwolf.alatus.Daemon")
        .await?;

    tracing::info!("Registered D-Bus interface and requested name io.strixwolf.alatus.Daemon");

    let conn_clone = connection.clone();
    let state_clone = Arc::clone(&state);
    let rgb_service_clone = Arc::clone(&rgb_service);
    tokio::spawn(async move {
        if let Err(e) = run_background_listener(conn_clone, state_clone, rgb_service_clone).await {
            tracing::error!("Background listener ended with error: {e}");
        }
    });

    let conn_hotkey = connection.clone();
    let state_hotkey = Arc::clone(&state);
    let handle = tokio::runtime::Handle::current();
    std::thread::spawn(move || {
        run_asus_hotkey_listener(conn_hotkey, state_hotkey, handle);
    });

    // Run forever
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_dbus_value_to_mode() {
        assert!(map_dbus_value_to_mode(0).is_ok());
        assert!(map_dbus_value_to_mode(1).is_ok());
        assert!(map_dbus_value_to_mode(2).is_ok());
        assert!(map_dbus_value_to_mode(3).is_ok());
        assert!(map_dbus_value_to_mode(4).is_err());
        assert!(map_dbus_value_to_mode(u32::MAX).is_err());
    }

    #[test]
    fn test_charge_limit_validation() {
        assert_eq!(validate_charge_limit(0).unwrap(), 0);
        assert_eq!(validate_charge_limit(60).unwrap(), 60);
        assert_eq!(validate_charge_limit(80).unwrap(), 80);
        assert_eq!(validate_charge_limit(100).unwrap(), 100);
        assert!(matches!(
            validate_charge_limit(101),
            Err(DaemonError::InvalidArgument(_))
        ));
        assert!(matches!(
            validate_charge_limit(u32::MAX),
            Err(DaemonError::InvalidArgument(_))
        ));
    }

    #[test]
    fn test_deep_sleep_parsing() {
        assert!(parse_mem_sleep_active("s2idle [deep]"));
        assert!(!parse_mem_sleep_active("[s2idle] deep"));
        assert!(!parse_mem_sleep_active("s2idle"));

        assert!(is_mem_sleep_mode_supported("s2idle [deep]", "deep"));
        assert!(is_mem_sleep_mode_supported("s2idle [deep]", "s2idle"));
        assert!(is_mem_sleep_mode_supported("[s2idle] deep", "deep"));
        assert!(is_mem_sleep_mode_supported("[s2idle] deep", "s2idle"));
        assert!(!is_mem_sleep_mode_supported("s2idle", "deep"));
    }

    #[test]
    fn test_polkit_authorization_denied() {
        let res = parse_polkit_result(false);
        assert!(matches!(res, Err(DaemonError::PermissionDenied(_))));
        let fdo: zbus::fdo::Error = res.unwrap_err().into();
        assert!(matches!(fdo, zbus::fdo::Error::Failed(msg) if msg.contains("authorization")));
    }

    #[test]
    fn test_polkit_authorization_allowed() {
        let res = parse_polkit_result(true);
        assert!(res.is_ok());
    }
}
