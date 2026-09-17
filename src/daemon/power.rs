// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::services::firmware_mode;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use zbus::Connection;

pub const SYS_MEM_SLEEP: &str = "/sys/power/mem_sleep";
pub const SYS_CHARGE_THRESHOLDS: &[&str] = &[
    "/sys/class/power_supply/BAT0/charge_control_end_threshold",
    "/sys/class/power_supply/BAT1/charge_control_end_threshold",
    "/sys/class/power_supply/BATC/charge_control_end_threshold",
    "/sys/class/power_supply/BATT/charge_control_end_threshold",
];

#[derive(Debug)]
pub enum DaemonError {
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

pub fn find_charge_threshold_path() -> Option<PathBuf> {
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

pub fn check_is_on_ac() -> bool {
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

pub fn check_conflicting_power_daemons_sync() -> bool {
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

pub fn read_product_serial() -> String {
    fs::read_to_string("/sys/class/dmi/id/product_serial")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "Unavailable".to_string())
}

pub fn read_charge_limit() -> Result<u32, DaemonError> {
    let path = find_charge_threshold_path().ok_or_else(|| {
        DaemonError::NotSupported("Charge control limit end threshold not supported".to_string())
    })?;
    let s = fs::read_to_string(path)
        .map_err(|e| DaemonError::IoError(format!("Read charge limit failed: {e}")))?;
    s.trim()
        .parse::<u32>()
        .map_err(|e| DaemonError::IoError(format!("Parse charge limit failed: {e}")))
}

pub fn validate_charge_limit(value: u32) -> Result<u32, DaemonError> {
    if value > 100 {
        Err(DaemonError::InvalidArgument(
            "Charge limit must be 0..=100".to_string(),
        ))
    } else {
        Ok(value)
    }
}

pub fn write_charge_limit(value: u32) -> Result<(), DaemonError> {
    validate_charge_limit(value)?;
    let path = find_charge_threshold_path().ok_or_else(|| {
        DaemonError::NotSupported("Charge control limit end threshold not supported".to_string())
    })?;
    tracing::info!("Writing charge limit: {value}% to {}", path.display());
    fs::write(path, format!("{value}\n"))
        .map_err(|e| DaemonError::IoError(format!("Write charge limit failed: {e}")))
}

pub fn parse_mem_sleep_active(content: &str) -> bool {
    content.contains("[deep]")
}

pub fn is_mem_sleep_mode_supported(content: &str, mode: &str) -> bool {
    content.split_whitespace().any(|w| {
        let clean = w.trim_matches(|c| c == '[' || c == ']');
        clean == mode
    })
}

pub fn read_deep_sleep() -> Result<bool, DaemonError> {
    if !Path::new(SYS_MEM_SLEEP).exists() {
        return Err(DaemonError::NotSupported(
            "Deep sleep configuration not supported".to_string(),
        ));
    }
    let s = fs::read_to_string(SYS_MEM_SLEEP)
        .map_err(|e| DaemonError::IoError(format!("Read mem_sleep failed: {e}")))?;
    Ok(parse_mem_sleep_active(&s))
}

pub fn write_deep_sleep(active: bool) -> Result<(), DaemonError> {
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

pub fn map_dbus_value_to_mode(value: u32) -> Result<firmware_mode::FirmwareMode, DaemonError> {
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

pub fn map_mode_to_dbus_value(mode: firmware_mode::FirmwareMode) -> u32 {
    match mode {
        firmware_mode::FirmwareMode::Balanced => 0,
        firmware_mode::FirmwareMode::Quiet => 1,
        firmware_mode::FirmwareMode::High => 2,
        firmware_mode::FirmwareMode::Full => 3,
        firmware_mode::FirmwareMode::Unknown(n) => n as u32,
    }
}

pub fn read_wmi_firmware_mode() -> Result<u32, DaemonError> {
    match firmware_mode::read_firmware_mode() {
        Ok(mode) => Ok(map_mode_to_dbus_value(mode)),
        Err(e) => Err(DaemonError::IoError(e.to_string())),
    }
}

pub fn write_wmi_firmware_mode(value: u32) -> Result<(), DaemonError> {
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

pub fn parse_polkit_result(is_authorized: bool) -> Result<(), DaemonError> {
    if is_authorized {
        Ok(())
    } else {
        Err(DaemonError::PermissionDenied(
            "Polkit authorization failed".to_string(),
        ))
    }
}

pub async fn check_polkit(
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
