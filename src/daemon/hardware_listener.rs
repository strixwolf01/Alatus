// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use super::dbus_interface::{DaemonInterface, DaemonState};
use super::inactivity::InactivityState;
use crate::domain::ThermalMode;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use zbus::Connection;

/// Thermal switch key codes defined across platform ACPI hotkey drivers:
/// - 148: KEY_PROG1 (platform thermal / power switch)
/// - 190: KEY_PROG4 (F20 hotkey)
/// - 202: KEY_PROG2
/// - 203: KEY_PROG3
/// - 482: Platform thermal policy switch key (Fn+F / code 482)
/// - 582: ACPI performance mode key
const THERMAL_HOTKEY_CODES: &[u16] = &[148, 190, 202, 203, 482, 582];

/// Checks whether a Linux input device's capabilities bitmask (`/sys/class/input/eventX/device/capabilities/key`)
/// contains a specific key code.
fn device_has_key(caps_key_str: &str, code: u16) -> bool {
    let words: Vec<&str> = caps_key_str.split_whitespace().collect();
    let word_idx = (code / 64) as usize;
    let bit_idx = (code % 64) as u32;

    if word_idx >= words.len() {
        return false;
    }
    // Words are formatted in big-endian order (highest word first)
    let rev_idx = words.len() - 1 - word_idx;
    if let Ok(val) = u64::from_str_radix(words[rev_idx], 16) {
        (val & (1u64 << bit_idx)) != 0
    } else {
        false
    }
}

/// Discovers keyboards and platform hotkey input devices dynamically via capabilities and sysfs.
pub fn find_keyboard_and_hotkey_devices() -> Vec<(PathBuf, String)> {
    let mut devices = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let mut add_device = |path: PathBuf, name: String| {
        if path.exists() {
            let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
            if seen.insert(canonical.clone()) {
                tracing::debug!(
                    "Discovered input hotkey/keyboard device: {} ({})",
                    canonical.display(),
                    name
                );
                devices.push((canonical, name));
            }
        }
    };

    // 1. Check known by-path symlinks (keyboard and platform event nodes)
    if let Ok(entries) = fs::read_dir("/dev/input/by-path") {
        for entry in entries.flatten() {
            let fname = entry.file_name();
            let name_str = fname.to_string_lossy();
            if name_str.ends_with("-event-kbd")
                || name_str.contains("hotkey")
                || name_str.contains("platform")
                || name_str.contains("wmi")
            {
                add_device(entry.path(), name_str.to_string());
            }
        }
    }

    // 2. Dynamically scan /sys/class/input/event* by device capabilities and names
    if let Ok(entries) = fs::read_dir("/sys/class/input") {
        for entry in entries.flatten() {
            let fname = entry.file_name();
            let name_str = fname.to_string_lossy();
            if name_str.starts_with("event") {
                let dev_node = PathBuf::from("/dev/input").join(&fname);
                let device_dir = entry.path().join("device");
                let sysfs_name_path = device_dir.join("name");
                let dev_name = fs::read_to_string(&sysfs_name_path)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|_| name_str.to_string());

                let caps_key =
                    fs::read_to_string(device_dir.join("capabilities/key")).unwrap_or_default();

                let has_thermal_key = THERMAL_HOTKEY_CODES
                    .iter()
                    .any(|&code| device_has_key(&caps_key, code));

                // Standard keyboard detection (KEY_ENTER=28, KEY_SPACE=57)
                let is_standard_keyboard =
                    device_has_key(&caps_key, 28) && device_has_key(&caps_key, 57);

                let name_lower = dev_name.to_ascii_lowercase();
                let matches_name = name_lower.contains("keyboard")
                    || name_lower.contains("kbd")
                    || name_lower.contains("hotkey")
                    || name_lower.contains("platform");

                if has_thermal_key || is_standard_keyboard || matches_name {
                    add_device(dev_node, dev_name);
                }
            }
        }
    }

    devices
}

pub async fn trigger_osd_notification(mode: u32) {
    let (summary, body, icon) = crate::services::desktop_session::thermal_mode_notification(mode);

    // Scan /run/user for active user D-Bus session sockets
    if let Ok(entries) = fs::read_dir("/run/user") {
        for entry in entries.flatten() {
            let bus_path = entry.path().join("bus");
            if bus_path.exists() {
                let address = format!("unix:path={}", bus_path.display());
                if let Ok(user_conn) = zbus::connection::Builder::address(address.as_str())
                    && let Ok(conn) = user_conn.build().await
                {
                    let _ = crate::services::desktop_session::send_osd_notification(
                        &conn, summary, body, &icon,
                    )
                    .await;
                }
            }
        }
    }
}

pub async fn handle_fn_f_hotkey(conn: &Connection, state: &Arc<Mutex<DaemonState>>) {
    let (cur_u32, device_context) = {
        let s = state.lock().await;
        (
            s.last_firmware_mode.unwrap_or(0),
            Arc::clone(&s.device_context),
        )
    };

    let current_mode = {
        let mut ctx = device_context.lock().await;
        if let Some(ref mut th) = ctx.thermal {
            th.get_mode()
                .unwrap_or_else(|_| ThermalMode::try_from(cur_u32).unwrap_or(ThermalMode::Balanced))
        } else {
            ThermalMode::try_from(cur_u32).unwrap_or(ThermalMode::Balanced)
        }
    };

    // Cycle order: Quiet -> Balanced -> Performance -> FullSpeed -> Quiet
    let next_mode = match current_mode {
        ThermalMode::Quiet => ThermalMode::Balanced,
        ThermalMode::Balanced => ThermalMode::Performance,
        ThermalMode::Performance => ThermalMode::FullSpeed,
        ThermalMode::FullSpeed => ThermalMode::Quiet,
    };

    tracing::info!(
        "Thermal mode switch hotkey cycling: {} -> {}",
        current_mode,
        next_mode
    );

    {
        let mut ctx = device_context.lock().await;
        if let Some(ref mut th) = ctx.thermal {
            match th.set_mode(next_mode) {
                Ok(()) => {}
                Err(e) => {
                    tracing::error!("Failed to write cycled thermal mode {next_mode}: {e}");
                    return;
                }
            }
        }
    }

    let next_u32 = next_mode.as_u32();
    {
        let mut s = state.lock().await;
        s.last_firmware_mode = Some(next_u32);
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
        let _ = DaemonInterface::thermal_mode_changed(emitter, next_u32).await;
        let _ = DaemonInterface::thermal_osd_triggered(emitter, next_u32).await;
    }

    // Direct OSD notification fallback
    trigger_osd_notification(next_u32).await;
}

pub fn listen_to_input_device(
    path: PathBuf,
    name: String,
    conn: Connection,
    state: Arc<Mutex<DaemonState>>,
    rgb_service: Arc<crate::services::rgb::RgbService>,
    inactivity: Arc<InactivityState>,
    tokio_handle: tokio::runtime::Handle,
) {
    use std::io::Read;

    loop {
        let Ok(mut file) = std::fs::File::open(&path) else {
            std::thread::sleep(std::time::Duration::from_secs(2));
            continue;
        };

        tracing::debug!(
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
                        // Key event logged at trace level to avoid journal flooding
                        tracing::trace!("Input event: code={}, val={}", code, value);

                        // Wake RGB immediately if timed out, and update last_activity
                        inactivity.record_activity(&rgb_service);

                        if value == 1 && THERMAL_HOTKEY_CODES.contains(&code) {
                            tracing::info!(
                                "Thermal mode switch hotkey press detected on {} (code {code})",
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
                    tracing::debug!(
                        "Input device {} ({}) disconnected: {e}. Reconnecting in 2s...",
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

pub fn run_hotkey_listener(
    conn: Connection,
    state: Arc<Mutex<DaemonState>>,
    rgb_service: Arc<crate::services::rgb::RgbService>,
    inactivity: Arc<InactivityState>,
    tokio_handle: tokio::runtime::Handle,
) {
    let devices = find_keyboard_and_hotkey_devices();
    if devices.is_empty() {
        tracing::warn!("No keyboard or platform hotkey event devices found");
        return;
    }

    for (path, name) in devices {
        let conn_clone = conn.clone();
        let state_clone = Arc::clone(&state);
        let rgb_clone = Arc::clone(&rgb_service);
        let inactivity_clone = Arc::clone(&inactivity);
        let handle_clone = tokio_handle.clone();
        std::thread::spawn(move || {
            listen_to_input_device(
                path,
                name,
                conn_clone,
                state_clone,
                rgb_clone,
                inactivity_clone,
                handle_clone,
            );
        });
    }
}

pub fn run_asus_hotkey_listener(
    conn: Connection,
    state: Arc<Mutex<DaemonState>>,
    rgb_service: Arc<crate::services::rgb::RgbService>,
    inactivity: Arc<InactivityState>,
    tokio_handle: tokio::runtime::Handle,
) {
    run_hotkey_listener(conn, state, rgb_service, inactivity, tokio_handle);
}
