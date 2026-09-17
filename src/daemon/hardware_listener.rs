// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use super::dbus_interface::{DaemonInterface, DaemonState};
use super::inactivity::InactivityState;
use super::power::{read_wmi_firmware_mode, write_wmi_firmware_mode};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use zbus::Connection;

pub fn find_keyboard_and_hotkey_devices() -> Vec<(PathBuf, String)> {
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

pub async fn trigger_osd_notification(mode: u32) {
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
                    let _ = crate::services::desktop_session::send_osd_notification(
                        &conn, summary, body, icon,
                    )
                    .await;
                }
            }
        }
    }
}

pub async fn handle_fn_f_hotkey(conn: &Connection, state: &Arc<Mutex<DaemonState>>) {
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
                        // Key event logged at trace level to avoid journal flooding
                        tracing::trace!("Input event: code={}, val={}", code, value);

                        // Wake RGB immediately if timed out, and update last_activity
                        inactivity.record_activity(&rgb_service);

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

pub fn run_asus_hotkey_listener(
    conn: Connection,
    state: Arc<Mutex<DaemonState>>,
    rgb_service: Arc<crate::services::rgb::RgbService>,
    inactivity: Arc<InactivityState>,
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
