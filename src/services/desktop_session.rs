// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! User-space desktop session integration agent.
//!
//! Provides event-driven synchronization with periodic reconciliation between the active desktop
//! environment (KDE Plasma & GNOME) and Alatus hardware controllers:
//! 1. XDG Desktop Portal Accent Color -> Keyboard RGB Backlight.
//! 2. Power Source (AC vs Battery) -> Display Panel Refresh Rate (e.g. 120Hz vs 60Hz).

use crate::services::daemon_client::DaemonClient;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering};
use zbus::zvariant::Value;
use zbus::{Connection, MatchRule};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopEnv {
    Kde,
    Gnome,
    Wlroots,
    Other,
}

pub fn detect_desktop() -> DesktopEnv {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_uppercase();
    if desktop.contains("KDE") {
        DesktopEnv::Kde
    } else if desktop.contains("GNOME") || desktop.contains("UBUNTU") {
        DesktopEnv::Gnome
    } else if desktop.contains("HYPRLAND")
        || desktop.contains("SWAY")
        || desktop.contains("WLROOTS")
        || desktop.contains("WAYFIRE")
        || desktop.contains("RIVER")
    {
        DesktopEnv::Wlroots
    } else {
        DesktopEnv::Other
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaylandEnv {
    pub wayland_display: String,
    pub xdg_runtime_dir: String,
    pub xdg_current_desktop: Option<String>,
}

/// Dynamically discovers the active Wayland display environment.
///
/// In systemd user sessions (e.g., alatus-session.service), environment variables like
/// WAYLAND_DISPLAY may not yet be imported into the activation environment.
/// This function falls back to scanning /run/user/<UID>/ for active wayland-* sockets.
pub fn resolve_wayland_env() -> Option<WaylandEnv> {
    let env_wayland = std::env::var("WAYLAND_DISPLAY").ok();
    let xdg_runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| {
        let uid = unsafe { libc::getuid() };
        format!("/run/user/{uid}")
    });
    let env_desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .ok()
        .or_else(|| std::env::var("XDG_SESSION_DESKTOP").ok());

    resolve_wayland_env_from(
        env_wayland.as_deref(),
        Path::new(&xdg_runtime_dir),
        env_desktop.as_deref(),
    )
}

/// Pure helper for resolving Wayland environment parameters from explicit inputs or socket scans.
pub fn resolve_wayland_env_from(
    env_wayland: Option<&str>,
    runtime_dir: &Path,
    env_desktop: Option<&str>,
) -> Option<WaylandEnv> {
    let desktop = env_desktop
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| Some("KDE".to_string()));

    let runtime_dir_str = runtime_dir.to_string_lossy().to_string();

    let wayland_display = if let Some(w) = env_wayland.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        w.to_string()
    } else {
        if !runtime_dir.is_dir() {
            return None;
        }

        let mut candidate_sockets = Vec::new();
        if let Ok(entries) = fs::read_dir(runtime_dir) {
            for entry in entries.flatten() {
                let file_name = entry.file_name().to_string_lossy().to_string();
                if file_name.starts_with("wayland-") && !file_name.ends_with(".lock") {
                    use std::os::unix::fs::FileTypeExt;
                    if let Ok(meta) = entry.metadata()
                        && (meta.file_type().is_socket() || meta.is_file())
                    {
                        candidate_sockets.push(file_name);
                    }
                }
            }
        }

        candidate_sockets.sort();
        candidate_sockets.into_iter().next()?
    };

    Some(WaylandEnv {
        wayland_display,
        xdg_runtime_dir: runtime_dir_str,
        xdg_current_desktop: desktop,
    })
}

/// Creates a std::process::Command for `kscreen-doctor` with injected Wayland environment variables.
///
/// Returns None if Wayland display resolution fails, preventing SIGABRT Qt initialization crashes.
pub fn create_kscreen_doctor_cmd() -> Option<Command> {
    let env = match resolve_wayland_env() {
        Some(env) => env,
        None => {
            tracing::warn!(
                "kscreen-doctor invocation skipped: active Wayland environment unresolved"
            );
            return None;
        }
    };

    let mut cmd = Command::new("kscreen-doctor");
    cmd.env("WAYLAND_DISPLAY", &env.wayland_display);
    cmd.env("XDG_RUNTIME_DIR", &env.xdg_runtime_dir);
    cmd.env("QT_QPA_PLATFORM", "wayland");
    if let Some(desktop) = &env.xdg_current_desktop {
        cmd.env("XDG_CURRENT_DESKTOP", desktop);
    }
    Some(cmd)
}

/// Robustly extracts (r, g, b) in [0, 255] range from XDG Desktop Portal accent-color value.
///
/// Handles both D-Bus (ddd) tuples/structs and arrays of doubles/numbers, unwrapping nested variants.
pub fn parse_accent_color(val: &Value<'_>) -> Option<(u8, u8, u8)> {
    match val {
        Value::Value(inner) => parse_accent_color(inner),
        Value::Structure(s) => {
            let fields = s.fields();
            if fields.len() >= 3 {
                let r = extract_float(&fields[0])?;
                let g = extract_float(&fields[1])?;
                let b = extract_float(&fields[2])?;
                Some((to_u8_color(r), to_u8_color(g), to_u8_color(b)))
            } else {
                None
            }
        }
        Value::Array(arr) => {
            let elements = arr.inner();
            if elements.len() >= 3 {
                let r = extract_float(&elements[0])?;
                let g = extract_float(&elements[1])?;
                let b = extract_float(&elements[2])?;
                Some((to_u8_color(r), to_u8_color(g), to_u8_color(b)))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn extract_float(v: &Value<'_>) -> Option<f64> {
    match v {
        Value::Value(inner) => extract_float(inner),
        Value::F64(f) => Some(*f),
        Value::U8(u) => Some(*u as f64 / 255.0),
        Value::U16(u) => Some(*u as f64 / 65535.0),
        Value::U32(u) => Some(*u as f64 / 255.0),
        _ => None,
    }
}

fn to_u8_color(f: f64) -> u8 {
    (f.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Automatically adjusts display panel refresh rate for internal eDP panels based on power state.
pub async fn apply_panel_refresh_rate(on_ac: bool) {
    let desktop = detect_desktop();
    match desktop {
        DesktopEnv::Kde => {
            apply_kde_panel_refresh(on_ac).await;
        }
        DesktopEnv::Gnome => {
            apply_gnome_panel_refresh(on_ac).await;
        }
        DesktopEnv::Wlroots => {
            tracing::info!(
                "Wlroots desktop detected (Hyprland/Sway). Panel refresh rate is managed via compositor config or wlr-randr (OnAC={on_ac})"
            );
        }
        DesktopEnv::Other => {
            tracing::info!(
                "Non-KDE/GNOME desktop detected. Skipping auto-panel refresh (OnAC={on_ac})"
            );
        }
    }
}

async fn apply_kde_panel_refresh(on_ac: bool) {
    let mut cmd = match create_kscreen_doctor_cmd() {
        Some(cmd) => cmd,
        None => return,
    };
    let output = match cmd.arg("-j").output() {
        Ok(out) => out,
        Err(e) => {
            tracing::warn!("Failed to query kscreen-doctor: {e}");
            return;
        }
    };

    if !output.status.success() {
        tracing::warn!("kscreen-doctor -j returned non-zero exit code");
        return;
    }

    let json_str = match std::str::from_utf8(&output.stdout) {
        Ok(s) => s,
        Err(_) => return,
    };

    let Ok(json): Result<serde_json::Value, _> = serde_json::from_str(json_str) else {
        tracing::warn!("Failed to parse kscreen-doctor JSON output");
        return;
    };

    let Some(outputs) = json.get("outputs").and_then(|o| o.as_array()) else {
        return;
    };

    for out in outputs {
        let name = out.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let connected = out
            .get("connected")
            .and_then(|c| c.as_bool())
            .unwrap_or(true);
        if !name.starts_with("eDP") || !connected {
            continue;
        }

        let modes = match out.get("modes").and_then(|m| m.as_array()) {
            Some(m) => m,
            None => continue,
        };

        // Determine target resolution from:
        // 1. Current size (if active)
        // 2. Preferred mode (if flagged)
        // 3. Maximum resolution available among all modes on this connector
        let mut max_res: (u64, u64) = (0, 0);
        let mut pref_res: Option<(u64, u64)> = None;

        for m in modes {
            let is_pref = m
                .get("preferred")
                .and_then(|p| p.as_bool())
                .unwrap_or(false)
                || m.get("isPreferred")
                    .and_then(|p| p.as_bool())
                    .unwrap_or(false);
            let size = m.get("size");
            let w = size
                .and_then(|s| s.get("width"))
                .and_then(|w| w.as_u64())
                .unwrap_or(0);
            let h = size
                .and_then(|s| s.get("height"))
                .and_then(|h| h.as_u64())
                .unwrap_or(0);

            if is_pref && pref_res.is_none() && w > 0 && h > 0 {
                pref_res = Some((w, h));
            }
            if w.saturating_mul(h) > max_res.0.saturating_mul(max_res.1) {
                max_res = (w, h);
            }
        }

        let current_size = out.get("size");
        let cur_w = current_size
            .and_then(|s| s.get("width"))
            .and_then(|w| w.as_u64())
            .unwrap_or(0);
        let cur_h = current_size
            .and_then(|s| s.get("height"))
            .and_then(|h| h.as_u64())
            .unwrap_or(0);

        let (target_w, target_h) = if cur_w > 0 && cur_h > 0 {
            (cur_w, cur_h)
        } else if let Some(pref) = pref_res {
            pref
        } else {
            max_res
        };

        if target_w == 0 || target_h == 0 {
            tracing::warn!("KDE: no valid resolution found for display {name}");
            continue;
        }

        // Find candidate modes for target resolution
        let mut matching_modes: Vec<(&str, &str, f64)> = Vec::new();
        for m in modes {
            let mode_id = m.get("id").and_then(|i| i.as_str()).unwrap_or("");
            let mode_name = m.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let refresh = m.get("refreshRate").and_then(|r| r.as_f64()).unwrap_or(0.0);
            let size = m.get("size");
            let w = size
                .and_then(|s| s.get("width"))
                .and_then(|w| w.as_u64())
                .unwrap_or(0);
            let h = size
                .and_then(|s| s.get("height"))
                .and_then(|h| h.as_u64())
                .unwrap_or(0);

            if w == target_w && h == target_h {
                matching_modes.push((mode_id, mode_name, refresh));
            }
        }

        if matching_modes.is_empty() {
            continue;
        }

        matching_modes.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

        let target_mode = if on_ac {
            // Highest refresh rate for AC
            matching_modes.last().unwrap()
        } else {
            // Power saver (closest mode to 60Hz or lowest refresh rate) for battery
            matching_modes
                .iter()
                .min_by(|a, b| {
                    let diff_a = (a.2 - 60.0).abs();
                    let diff_b = (b.2 - 60.0).abs();
                    diff_a
                        .partial_cmp(&diff_b)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap_or(&matching_modes[0])
        };

        tracing::info!(
            "KDE eDP Panel: Switching {name} to mode {} ({}, {:.1}Hz) for OnAC={on_ac}",
            target_mode.0,
            target_mode.1,
            target_mode.2
        );

        let res = match create_kscreen_doctor_cmd() {
            Some(mut cmd) => cmd
                .arg(format!("output.{name}.mode.{}", target_mode.0))
                .status(),
            None => {
                tracing::warn!(
                    "Failed to switch kscreen-doctor mode: Wayland environment unresolved"
                );
                return;
            }
        };
        if let Err(e) = res {
            tracing::warn!("Failed to switch kscreen-doctor mode: {e}");
        }
    }
}

type MutterMonitorSpec = (String, String, String, String);
pub type MutterModeSpec = (
    String,                                                        // id
    i32,                                                           // width
    i32,                                                           // height
    f64,                                                           // refresh rate
    f64,                                                           // preferred scale
    Vec<f64>,                                                      // supported scales
    std::collections::HashMap<String, zbus::zvariant::OwnedValue>, // properties
);
type MutterMonitorItem = (
    MutterMonitorSpec,
    Vec<MutterModeSpec>,
    std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
);
type MutterLogicalMonitorItem = (
    i32,  // x
    i32,  // y
    f64,  // scale
    u32,  // transform
    bool, // primary
    Vec<MutterMonitorSpec>,
    std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
);

type MutterGetCurrentStateReturn = (
    u32,
    Vec<MutterMonitorItem>,
    Vec<MutterLogicalMonitorItem>,
    std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
);

type ApplyMonitorAssignment = (
    String,
    String,
    std::collections::HashMap<String, zbus::zvariant::Value<'static>>,
);

type ApplyLogicalMonitor = (i32, i32, f64, u32, bool, Vec<ApplyMonitorAssignment>);

pub fn pick_mutter_mode(
    modes: &[MutterModeSpec],
    target_res: (i32, i32),
    on_ac: bool,
) -> Option<&MutterModeSpec> {
    let mut matching: Vec<&MutterModeSpec> = modes
        .iter()
        .filter(|m| m.1 == target_res.0 && m.2 == target_res.1)
        .collect();

    if matching.is_empty() {
        return None;
    }

    matching.sort_by(|a, b| a.3.partial_cmp(&b.3).unwrap_or(std::cmp::Ordering::Equal));

    if on_ac {
        matching.last().copied()
    } else {
        // Battery: pick mode closest to 60.0Hz (or lowest supported refresh rate)
        matching
            .iter()
            .min_by(|a, b| {
                let diff_a = (a.3 - 60.0).abs();
                let diff_b = (b.3 - 60.0).abs();
                diff_a
                    .partial_cmp(&diff_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .copied()
    }
}

pub fn pick_mutter_mode_for_rate<'a>(
    modes: &'a [MutterModeSpec],
    target_res: (i32, i32),
    target_hz: f64,
) -> Option<&'a MutterModeSpec> {
    let mut matching: Vec<&'a MutterModeSpec> = modes
        .iter()
        .filter(|m| m.1 == target_res.0 && m.2 == target_res.1)
        .collect();

    if matching.is_empty() {
        return None;
    }

    matching.sort_by(|a, b| {
        (a.3 - target_hz)
            .abs()
            .partial_cmp(&(b.3 - target_hz).abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    matching.first().copied()
}

async fn apply_gnome_panel_refresh(on_ac: bool) {
    let Ok(conn) = Connection::session().await else {
        tracing::warn!("GNOME Mutter: failed to connect to user session bus");
        return;
    };

    let proxy = match zbus::Proxy::new(
        &conn,
        "org.gnome.Mutter.DisplayConfig",
        "/org/gnome/Mutter/DisplayConfig",
        "org.gnome.Mutter.DisplayConfig",
    )
    .await
    {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("GNOME Mutter: failed to create DisplayConfig proxy: {e}");
            return;
        }
    };

    let state: MutterGetCurrentStateReturn = match proxy.call("GetCurrentState", &()).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("GNOME Mutter DisplayConfig: GetCurrentState failed: {e}");
            return;
        }
    };

    let (serial, monitors, logical_monitors, _properties) = state;

    let edp_monitor = monitors.iter().find(|m| m.0.0.starts_with("eDP"));
    let Some(edp_monitor) = edp_monitor else {
        tracing::warn!("GNOME Mutter: no internal eDP display found in connected monitors");
        return;
    };

    let edp_connector = &edp_monitor.0.0;
    let modes = &edp_monitor.1;

    let current_mode = modes.iter().find(|m| {
        m.6.get("is-current")
            .and_then(|v| bool::try_from(v).ok())
            .unwrap_or(false)
    });
    let preferred_mode = modes.iter().find(|m| {
        m.6.get("is-preferred")
            .and_then(|v| bool::try_from(v).ok())
            .unwrap_or(false)
    });

    // Dynamically resolve target resolution from current -> preferred -> max available
    let (target_w, target_h) = if let Some(cur) = current_mode {
        (cur.1, cur.2)
    } else if let Some(pref) = preferred_mode {
        (pref.1, pref.2)
    } else {
        modes
            .iter()
            .map(|m| (m.1, m.2))
            .max_by_key(|&(w, h)| (w as i64) * (h as i64))
            .unwrap_or((0, 0))
    };

    if target_w == 0 || target_h == 0 {
        tracing::warn!(
            "GNOME Mutter: no valid resolution found for internal display {edp_connector}"
        );
        return;
    }

    let Some(target_mode) = pick_mutter_mode(modes, (target_w, target_h), on_ac) else {
        tracing::warn!("GNOME Mutter: no modes found matching resolution {target_w}x{target_h}");
        return;
    };

    let target_mode_id = &target_mode.0;
    let target_refresh = target_mode.3;

    if let Some(cur) = current_mode
        && &cur.0 == target_mode_id
    {
        tracing::info!(
            "GNOME Mutter: {edp_connector} already running target mode {target_mode_id} ({target_w}x{target_h}@{target_refresh:.1}Hz)"
        );
        return;
    }

    tracing::info!(
        "GNOME Mutter: Switching {edp_connector} to mode {target_mode_id} ({target_w}x{target_h}@{target_refresh:.1}Hz) for OnAC={on_ac}"
    );

    let mut apply_logical_monitors: Vec<ApplyLogicalMonitor> = Vec::new();

    for lm in &logical_monitors {
        let x = lm.0;
        let y = lm.1;
        let scale = lm.2;
        let transform = lm.3;
        let primary = lm.4;
        let mut assignments: Vec<ApplyMonitorAssignment> = Vec::new();

        for m_spec in &lm.5 {
            let conn_name = &m_spec.0;
            if conn_name == edp_connector {
                assignments.push((
                    conn_name.clone(),
                    target_mode_id.clone(),
                    std::collections::HashMap::new(),
                ));
            } else {
                let ext_current_mode_id = monitors
                    .iter()
                    .find(|m| &m.0.0 == conn_name)
                    .and_then(|m| {
                        m.1.iter().find(|mode| {
                            mode.6
                                .get("is-current")
                                .and_then(|v| bool::try_from(v).ok())
                                .unwrap_or(false)
                        })
                    })
                    .map(|mode| mode.0.clone())
                    .unwrap_or_default();

                if !ext_current_mode_id.is_empty() {
                    assignments.push((
                        conn_name.clone(),
                        ext_current_mode_id,
                        std::collections::HashMap::new(),
                    ));
                }
            }
        }

        if !assignments.is_empty() {
            apply_logical_monitors.push((x, y, scale, transform, primary, assignments));
        }
    }

    if apply_logical_monitors.is_empty() {
        tracing::warn!("GNOME Mutter: no logical monitors constructed for ApplyMonitorsConfig");
        return;
    }

    let empty_props = std::collections::HashMap::<String, zbus::zvariant::Value<'static>>::new();
    let res: Result<(), _> = proxy
        .call(
            "ApplyMonitorsConfig",
            &(serial, 1u32, apply_logical_monitors, empty_props),
        )
        .await;

    match res {
        Ok(()) => {
            tracing::info!(
                "GNOME Mutter: successfully switched {edp_connector} to {target_mode_id} ({target_w}x{target_h}@{target_refresh:.1}Hz)"
            );
        }
        Err(e) => {
            tracing::warn!("GNOME Mutter DisplayConfig: ApplyMonitorsConfig failed: {e}");
        }
    }
}

pub async fn read_gnome_screen_brightness(conn: &Connection) -> Result<i32, zbus::Error> {
    let proxy = zbus::Proxy::new(
        conn,
        "org.gnome.SettingsDaemon.Power",
        "/org/gnome/SettingsDaemon/Power/Screen",
        "org.gnome.SettingsDaemon.Power.Screen",
    )
    .await?;
    let val: i32 = proxy.get_property("Brightness").await?;
    Ok(val)
}

pub async fn set_gnome_screen_brightness(
    conn: &Connection,
    brightness: i32,
) -> Result<(), zbus::Error> {
    let proxy = zbus::Proxy::new(
        conn,
        "org.gnome.SettingsDaemon.Power",
        "/org/gnome/SettingsDaemon/Power/Screen",
        "org.gnome.SettingsDaemon.Power.Screen",
    )
    .await?;
    proxy.set_property("Brightness", brightness).await?;
    Ok(())
}

pub async fn read_kde_screen_brightness(conn: &Connection) -> Result<i32, zbus::Error> {
    let proxy = zbus::Proxy::new(
        conn,
        "org.kde.Solid.PowerManagement",
        "/org/kde/Solid/PowerManagement/Actions/BrightnessControl",
        "org.kde.Solid.PowerManagement.Actions.BrightnessControl",
    )
    .await?;
    let cur: i32 = proxy.call("brightness", &()).await?;
    let max: i32 = proxy.call("brightnessMax", &()).await.unwrap_or(100);
    if max > 0 {
        Ok(((cur as f64 * 100.0) / max as f64).round() as i32)
    } else {
        Ok(cur)
    }
}

pub async fn set_kde_screen_brightness(
    conn: &Connection,
    brightness: i32,
) -> Result<(), zbus::Error> {
    let proxy = zbus::Proxy::new(
        conn,
        "org.kde.Solid.PowerManagement",
        "/org/kde/Solid/PowerManagement/Actions/BrightnessControl",
        "org.kde.Solid.PowerManagement.Actions.BrightnessControl",
    )
    .await?;
    let max: i32 = proxy.call("brightnessMax", &()).await.unwrap_or(100);
    let target = if max > 0 {
        ((max as f64 * brightness.clamp(0, 100) as f64) / 100.0).round() as i32
    } else {
        brightness.clamp(0, 100)
    };
    if proxy
        .call::<_, _, ()>("setBrightnessSilent", &(target,))
        .await
        .is_err()
    {
        let _ = proxy.call::<_, _, ()>("setBrightness", &(target,)).await;
    }
    Ok(())
}

pub async fn find_kde_internal_connector() -> Option<String> {
    // 1. Try kscreen-doctor -j (JSON)
    if let Some(mut cmd) = create_kscreen_doctor_cmd()
        && let Ok(output) = cmd.arg("-j").output()
        && let Ok(json_str) = String::from_utf8(output.stdout)
        && let Ok(json) = serde_json::from_str::<serde_json::Value>(&json_str)
        && let Some(outputs) = json.get("outputs").and_then(|o| o.as_array())
    {
        for out in outputs {
            if let Some(name) = out.get("name").and_then(|n| n.as_str()) {
                let connected = out
                    .get("connected")
                    .and_then(|c| c.as_bool())
                    .unwrap_or(true);
                if name.starts_with("eDP") && connected {
                    return Some(name.to_string());
                }
            }
        }
    }

    // 2. Try kscreen-doctor -o (human-readable lines)
    if let Some(mut cmd) = create_kscreen_doctor_cmd()
        && let Ok(output) = cmd.arg("-o").output()
        && let Ok(text) = String::from_utf8(output.stdout)
    {
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("Output:") {
                for token in trimmed.split_whitespace() {
                    if token.starts_with("eDP") {
                        return Some(token.to_string());
                    }
                }
            }
        }
    }

    // 3. Fallback: DRM sysfs scan for connected eDP connector
    if let Ok(entries) = fs::read_dir("/sys/class/drm") {
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if let Some(idx) = file_name.find("eDP") {
                let conn_name = &file_name[idx..];
                let status_path = entry.path().join("status");
                if let Ok(status) = fs::read_to_string(&status_path)
                    && status.trim() == "connected"
                {
                    return Some(conn_name.to_string());
                }
            }
        }
    }

    None
}

pub async fn apply_kde_panel_dimming(connector: &str, dim: bool, dim_level: Option<u32>) {
    let dim_value = if dim {
        dim_level.unwrap_or(20).clamp(10, 100)
    } else {
        100
    };
    let arg = format!("output.{connector}.brightness.{dim_value}");
    let mut success = false;

    if let Some(mut cmd) = create_kscreen_doctor_cmd() {
        let res = cmd.arg(&arg).output();
        if let Ok(out) = res {
            if out.status.success() {
                success = true;
            } else {
                // Fallback for HDR mode: use sdr-brightness (range 100-1000)
                let sdr_val = (dim_value as f64 * 10.0).clamp(100.0, 1000.0) as u32;
                let sdr_arg = format!("output.{connector}.sdr-brightness.{sdr_val}");
                if let Some(mut sdr_cmd) = create_kscreen_doctor_cmd()
                    && let Ok(sdr_out) = sdr_cmd.arg(&sdr_arg).output()
                    && sdr_out.status.success()
                {
                    success = true;
                }
            }
        }
    }

    if !success {
        tracing::warn!(
            "kscreen-doctor dimming failed or unavailable; falling back to D-Bus BrightnessControl"
        );
        if let Ok(conn) = Connection::session().await {
            let _ = set_kde_screen_brightness(&conn, dim_value as i32).await;
        }
    }
}

/// Instant compositor-level software dimming without touching hardware backlight.
pub async fn apply_flicker_free_dimming(connector: Option<&str>, level_pct: u32) {
    let clamped = level_pct.min(100);
    let desktop = detect_desktop();
    match desktop {
        DesktopEnv::Kde => {
            let conn = match connector {
                Some(c) => c.to_string(),
                None => find_kde_internal_connector()
                    .await
                    .unwrap_or_else(|| "eDP-1".to_string()),
            };
            let arg = format!("output.{conn}.brightness.{clamped}");
            tracing::info!("KDE Plasma Flicker-Free Dimming: executing kscreen-doctor {arg}");
            let mut success = false;

            if let Some(mut cmd) = create_kscreen_doctor_cmd() {
                let res = cmd.arg(&arg).output();
                if let Ok(out) = res {
                    if out.status.success() {
                        success = true;
                    } else {
                        // Fallback for HDR mode: use sdr-brightness (range 100-1000)
                        let sdr_val = ((clamped as f64) * 10.0).clamp(100.0, 1000.0) as u32;
                        let sdr_arg = format!("output.{conn}.sdr-brightness.{sdr_val}");
                        if let Some(mut sdr_cmd) = create_kscreen_doctor_cmd()
                            && let Ok(sdr_out) = sdr_cmd.arg(&sdr_arg).output()
                            && sdr_out.status.success()
                        {
                            success = true;
                        }
                    }
                }
            }

            if !success {
                tracing::warn!(
                    "kscreen-doctor flicker-free dimming failed or unavailable; falling back to D-Bus BrightnessControl"
                );
                if let Ok(session_conn) = Connection::session().await {
                    let _ = set_kde_screen_brightness(&session_conn, clamped as i32).await;
                }
            }
        }
        DesktopEnv::Wlroots => {
            let factor = (clamped as f64) / 100.0;
            tracing::info!(
                "Wlroots Compositor Flicker-Free Dimming software luminance factor: {factor:.2} (Hyprland/Sway)"
            );
        }
        DesktopEnv::Gnome | DesktopEnv::Other => {
            let factor = (clamped as f64) / 100.0;
            tracing::info!(
                "Compositor Flicker-Free Dimming software luminance factor: {factor:.2}"
            );
        }
    }
}

pub fn default_oled_dim_level() -> u32 {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OledMetrics {
    pub active_screen_seconds: u64,
    pub refresh_count: u32,
    pub last_refresh_timestamp: u64,
    #[serde(default = "default_oled_dim_level")]
    pub oled_dim_level: u32,
}

impl Default for OledMetrics {
    fn default() -> Self {
        Self {
            active_screen_seconds: 0,
            refresh_count: 0,
            last_refresh_timestamp: 0,
            oled_dim_level: default_oled_dim_level(),
        }
    }
}

pub fn get_metrics_path() -> Option<PathBuf> {
    let base = if let Ok(dir) = std::env::var("XDG_STATE_HOME") {
        PathBuf::from(dir).join("alatus")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home)
            .join(".local")
            .join("state")
            .join("alatus")
    } else {
        return None;
    };

    let metrics_file = base.join("metrics.json");
    let legacy_file = base.join("session_metrics.json");
    if metrics_file.exists() {
        Some(metrics_file)
    } else if legacy_file.exists() {
        Some(legacy_file)
    } else {
        // Check legacy ascend location
        let legacy_ascend = base.parent().map(|p| p.join("ascend").join("metrics.json"));
        if let Some(ref l) = legacy_ascend
            && l.exists()
        {
            legacy_ascend
        } else {
            Some(metrics_file)
        }
    }
}

pub fn get_primary_metrics_path() -> Option<PathBuf> {
    let base = if let Ok(dir) = std::env::var("XDG_STATE_HOME") {
        PathBuf::from(dir).join("alatus")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home)
            .join(".local")
            .join("state")
            .join("alatus")
    } else {
        return None;
    };
    Some(base.join("metrics.json"))
}

pub fn load_oled_metrics() -> OledMetrics {
    load_oled_metrics_from(get_metrics_path().as_deref())
}

pub fn load_oled_metrics_from(path: Option<&Path>) -> OledMetrics {
    let Some(path) = path else {
        return OledMetrics::default();
    };
    if let Ok(content) = fs::read_to_string(path)
        && let Ok(metrics) = serde_json::from_str::<OledMetrics>(&content)
    {
        return metrics;
    }
    // Fallback: if path was metrics.json, check session_metrics.json in parent dir
    if let Some(parent) = path.parent() {
        let legacy = parent.join("session_metrics.json");
        if legacy != path
            && legacy.exists()
            && let Ok(content) = fs::read_to_string(&legacy)
            && let Ok(metrics) = serde_json::from_str::<OledMetrics>(&content)
        {
            return metrics;
        }
    }
    OledMetrics::default()
}

pub fn save_oled_metrics(metrics: &OledMetrics) {
    save_oled_metrics_to(get_primary_metrics_path().as_deref(), metrics);
}

pub fn save_oled_metrics_to(path: Option<&Path>, metrics: &OledMetrics) {
    let Some(path) = path else { return };
    if let Some(parent) = path.parent() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            builder.mode(0o700);
            let _ = builder.create(parent);
        }
        #[cfg(not(unix))]
        {
            let _ = fs::create_dir_all(parent);
        }
    }
    if let Ok(json) = serde_json::to_string_pretty(metrics) {
        let _ = fs::write(path, json);
    }
}

pub fn get_epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub async fn send_desktop_notification(
    session_conn: &Connection,
    summary: &str,
    body: &str,
) -> Result<u32, zbus::Error> {
    let proxy = zbus::Proxy::new(
        session_conn,
        "org.freedesktop.Notifications",
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await?;

    let actions: Vec<&str> = Vec::new();
    let hints: std::collections::HashMap<&str, zbus::zvariant::Value<'_>> =
        std::collections::HashMap::new();

    let id: u32 = proxy
        .call(
            "Notify",
            &(
                "Alatus",
                0u32,
                "preferences-desktop-display",
                summary,
                body,
                actions,
                hints,
                5000i32,
            ),
        )
        .await?;

    Ok(id)
}

pub async fn send_osd_notification(
    session_conn: &Connection,
    summary: &str,
    body: &str,
    icon: &str,
) -> Result<u32, zbus::Error> {
    let proxy = zbus::Proxy::new(
        session_conn,
        "org.freedesktop.Notifications",
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await?;

    let actions: Vec<&str> = Vec::new();
    let mut hints: std::collections::HashMap<&str, zbus::zvariant::Value<'_>> =
        std::collections::HashMap::new();
    hints.insert("urgency", zbus::zvariant::Value::U8(1));
    hints.insert("transient", zbus::zvariant::Value::Bool(true));
    hints.insert(
        "category",
        zbus::zvariant::Value::from("x-kde.display-profile"),
    );
    hints.insert(
        "x-canonical-private-synchronous",
        zbus::zvariant::Value::from("thermal-profile-osd"),
    );

    let id: u32 = proxy
        .call(
            "Notify",
            &("Alatus", 0u32, icon, summary, body, actions, hints, 2000i32),
        )
        .await?;

    Ok(id)
}

pub async fn execute_pixel_refresh(
    session_conn: &Connection,
    state: Arc<SessionState>,
    _desktop: DesktopEnv,
    internal_connector: Option<String>,
) -> bool {
    // Atomic test-and-set: prevent concurrent conditioning executions
    if state
        .pixel_refresh_in_progress
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        tracing::warn!("Pixel refresh already in progress. Ignoring duplicate trigger.");
        return false;
    }

    let _ = send_desktop_notification(
        session_conn,
        "OLED Care: Pixel Refresh Started",
        "Conditioning OLED panel to relieve subpixel stress...",
    )
    .await;

    // Reset usage counter and increment cycle count
    state.active_screen_seconds.store(0, Ordering::Relaxed);
    let count = state.pixel_refresh_count.fetch_add(1, Ordering::Relaxed) + 1;
    save_oled_metrics(&OledMetrics {
        active_screen_seconds: 0,
        refresh_count: count,
        last_refresh_timestamp: get_epoch_seconds(),
        oled_dim_level: state.oled_dim_level.load(Ordering::Relaxed),
    });

    let conn_clone = session_conn.clone();
    let state_clone = Arc::clone(&state);
    tokio::spawn(async move {
        struct RefreshGuard(Arc<SessionState>);
        impl Drop for RefreshGuard {
            fn drop(&mut self) {
                self.0
                    .pixel_refresh_in_progress
                    .store(false, Ordering::SeqCst);
            }
        }
        let _guard = RefreshGuard(Arc::clone(&state_clone));

        let resolved_connector = match internal_connector {
            Some(c) => Some(c),
            None => find_kde_internal_connector().await,
        };

        // Deep conditioning blackout pulse at compositor level (0% total black, never touch /sys/class/backlight)
        apply_flicker_free_dimming(resolved_connector.as_deref(), 0).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
        let restored_level = state_clone
            .oled_dim_level
            .load(Ordering::Relaxed)
            .clamp(10, 100);
        apply_flicker_free_dimming(resolved_connector.as_deref(), restored_level).await;

        let _ = send_desktop_notification(
            &conn_clone,
            "OLED Care: Pixel Refresh Completed",
            "Panel conditioning cycle finished successfully.",
        )
        .await;
    });

    true
}

pub async fn apply_oled_care_dimming(
    _session_conn: &Connection,
    daemon_client: &DaemonClient,
    state: &SessionState,
    _desktop: DesktopEnv,
    internal_connector: Option<&str>,
    dim: bool,
) {
    if !state.oled_care.load(Ordering::Relaxed) {
        return;
    }

    if dim {
        // Prevent cache clobbering: never overwrite cached values if already dimmed
        if !state.try_enter_dimmed() {
            return;
        }

        tracing::info!("OLED Care: User idle detected. Applying software compositor dimming.");

        // Cache pre-dim keyboard RGB brightness
        if let Ok(rgb_status) = daemon_client.get_rgb_status().await
            && rgb_status.available
        {
            state
                .cached_rgb_brightness
                .store(rgb_status.brightness, Ordering::Relaxed);
        }

        // Apply software dimming at compositor level (never touches hardware backlight)
        apply_flicker_free_dimming(internal_connector, 20).await;

        // Dim keyboard RGB to 0%
        let _ = daemon_client.set_rgb_brightness(0).await;
    } else {
        // Wake-up event
        if !state.try_exit_dimmed() {
            return;
        }

        tracing::info!("OLED Care: User return detected. Restoring software luminance.");
        let restored_level = state.oled_dim_level.load(Ordering::Relaxed).clamp(10, 100);
        apply_flicker_free_dimming(internal_connector, restored_level).await;

        // Restore keyboard RGB brightness
        let cached_rgb = state.cached_rgb_brightness.load(Ordering::Relaxed);
        let _ = daemon_client.set_rgb_brightness(cached_rgb).await;
    }
}

#[derive(Debug)]
pub struct SessionState {
    pub sync_accent: AtomicBool,
    pub auto_refresh: AtomicBool,
    pub oled_care: AtomicBool,
    pub power_monitor_active: AtomicBool,
    pub is_dimmed: AtomicBool,
    pub cached_panel_brightness: AtomicI32,
    pub cached_rgb_brightness: AtomicU32,
    pub active_screen_seconds: AtomicU64,
    pub pixel_refresh_count: AtomicU32,
    pub pixel_refresh_in_progress: AtomicBool,
    pub oled_dim_level: AtomicU32,
}

impl SessionState {
    pub fn new(sync_accent: bool, auto_refresh: bool, oled_care: bool) -> Self {
        Self::with_metrics(sync_accent, auto_refresh, oled_care, load_oled_metrics())
    }

    pub fn with_metrics(
        sync_accent: bool,
        auto_refresh: bool,
        oled_care: bool,
        metrics: OledMetrics,
    ) -> Self {
        Self {
            sync_accent: AtomicBool::new(sync_accent),
            auto_refresh: AtomicBool::new(auto_refresh),
            oled_care: AtomicBool::new(oled_care),
            power_monitor_active: AtomicBool::new(false),
            is_dimmed: AtomicBool::new(false),
            cached_panel_brightness: AtomicI32::new(-1),
            cached_rgb_brightness: AtomicU32::new(100),
            active_screen_seconds: AtomicU64::new(metrics.active_screen_seconds),
            pixel_refresh_count: AtomicU32::new(metrics.refresh_count),
            pixel_refresh_in_progress: AtomicBool::new(false),
            oled_dim_level: AtomicU32::new(metrics.oled_dim_level),
        }
    }

    /// Attempts to enter the dimmed state atomically.
    /// Returns true if successfully transitioned from not-dimmed to dimmed,
    /// or false if already dimmed (preventing pre-dim cache clobbering).
    pub fn try_enter_dimmed(&self) -> bool {
        self.is_dimmed
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// Attempts to exit the dimmed state atomically.
    /// Returns true if successfully transitioned from dimmed to not-dimmed.
    pub fn try_exit_dimmed(&self) -> bool {
        self.is_dimmed
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }
}

pub struct SessionDbusInterface {
    pub state: Arc<SessionState>,
    pub session_conn: Connection,
    pub desktop: DesktopEnv,
    pub internal_connector: Option<String>,
}

#[zbus::interface(name = "io.strixwolf.alatus.Session")]
impl SessionDbusInterface {
    #[zbus(property)]
    fn sync_accent(&self) -> bool {
        self.state.sync_accent.load(Ordering::Relaxed)
    }

    #[zbus(name = "SetSyncAccent")]
    async fn set_sync_accent(&self, value: bool) {
        self.state.sync_accent.store(value, Ordering::Relaxed);
        tracing::info!("Session: Desktop Accent Sync dynamically set to {value}");
    }

    #[zbus(property)]
    fn auto_refresh(&self) -> bool {
        self.state.auto_refresh.load(Ordering::Relaxed)
    }

    #[zbus(name = "SetAutoRefresh")]
    async fn set_auto_refresh(&self, value: bool) {
        self.state.auto_refresh.store(value, Ordering::Relaxed);
        tracing::info!("Session: Panel Refresh Switching dynamically set to {value}");
    }

    #[zbus(property)]
    fn oled_care(&self) -> bool {
        self.state.oled_care.load(Ordering::Relaxed)
    }

    #[zbus(property)]
    async fn set_oled_care(&self, value: bool) {
        self.state.oled_care.store(value, Ordering::Relaxed);
        tracing::info!("Session: OLED Care dynamically set to {value}");
    }

    #[zbus(name = "SetOledCare")]
    async fn set_oled_care_method(&self, value: bool) {
        self.set_oled_care(value).await;
    }

    #[zbus(property)]
    fn pixel_refresh_hours(&self) -> f64 {
        (self.state.active_screen_seconds.load(Ordering::Relaxed) as f64) / 3600.0
    }

    #[zbus(property)]
    fn pixel_refresh_count(&self) -> u32 {
        self.state.pixel_refresh_count.load(Ordering::Relaxed)
    }

    #[zbus(property)]
    fn pixel_refresh_in_progress(&self) -> bool {
        self.state.pixel_refresh_in_progress.load(Ordering::Relaxed)
    }

    #[zbus(name = "TriggerPixelRefresh")]
    async fn trigger_pixel_refresh_method(&self) -> bool {
        execute_pixel_refresh(
            &self.session_conn,
            Arc::clone(&self.state),
            self.desktop,
            self.internal_connector.clone(),
        )
        .await
    }

    #[zbus(property)]
    fn power_monitor_active(&self) -> bool {
        self.state.power_monitor_active.load(Ordering::Relaxed)
    }

    #[zbus(property)]
    fn oled_dim_level(&self) -> u32 {
        self.state.oled_dim_level.load(Ordering::Relaxed)
    }

    #[zbus(name = "SetOledDimLevel")]
    async fn set_oled_dim_level(&self, value: u32) {
        let clamped = value.clamp(10, 100);
        self.state.oled_dim_level.store(clamped, Ordering::Relaxed);
        let mut metrics = load_oled_metrics();
        metrics.oled_dim_level = clamped;
        save_oled_metrics(&metrics);
        tracing::info!("Session: Flicker-Free OLED Dim Level dynamically set to {clamped}%");

        // Instant Compositor-level flicker-free dimming (never touches /sys/class/backlight)
        apply_flicker_free_dimming(self.internal_connector.as_deref(), clamped).await;
    }

    #[zbus(name = "SetFlickerFreeDimming")]
    async fn set_flicker_free_dimming(&self, value: u32) {
        self.set_oled_dim_level(value).await;
    }

    #[zbus(name = "GetStatus")]
    async fn get_status(&self) -> (bool, bool, bool, bool, f64, u32) {
        (
            self.state.sync_accent.load(Ordering::Relaxed),
            self.state.auto_refresh.load(Ordering::Relaxed),
            self.state.power_monitor_active.load(Ordering::Relaxed),
            self.state.oled_care.load(Ordering::Relaxed),
            (self.state.active_screen_seconds.load(Ordering::Relaxed) as f64) / 3600.0,
            self.state.pixel_refresh_count.load(Ordering::Relaxed),
        )
    }

    #[zbus(name = "ShowThermalOsd")]
    async fn show_thermal_osd(&self, mode: u32) {
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
        let _ = send_osd_notification(&self.session_conn, summary, body, icon).await;
    }
}

pub const SESSION_BUS_NAME: &str = "io.strixwolf.alatus.Session";
pub const SESSION_OBJECT_PATH: &str = "/io/strixwolf/alatus/Session";

/// Checks whether the desktop session daemon is currently running and owns its D-Bus name.
pub async fn is_session_daemon_running() -> bool {
    if let Ok(conn) = Connection::session().await
        && let Ok(proxy) = zbus::fdo::DBusProxy::new(&conn).await
        && let Ok(name) = zbus::names::WellKnownName::try_from(SESSION_BUS_NAME)
    {
        return proxy.name_has_owner(name.into()).await.unwrap_or(false);
    }
    false
}

#[derive(Debug, Clone)]
pub struct SessionStatusInfo {
    pub running: bool,
    pub desktop: DesktopEnv,
    pub sync_accent: bool,
    pub auto_refresh: bool,
    pub power_monitor_active: bool,
    pub oled_care: bool,
    pub pixel_refresh_hours: f64,
    pub pixel_refresh_count: u32,
    pub oled_dim_level: u32,
}

pub async fn query_session_status() -> SessionStatusInfo {
    let desktop = detect_desktop();
    let metrics = load_oled_metrics();

    let Ok(conn) = Connection::session().await else {
        return SessionStatusInfo {
            running: false,
            desktop,
            sync_accent: true,
            auto_refresh: true,
            power_monitor_active: false,
            oled_care: true,
            pixel_refresh_hours: (metrics.active_screen_seconds as f64) / 3600.0,
            pixel_refresh_count: metrics.refresh_count,
            oled_dim_level: metrics.oled_dim_level,
        };
    };

    // Check if the well-known name has an active owner on the session bus
    let has_owner = if let Ok(dbus_proxy) = zbus::fdo::DBusProxy::new(&conn).await
        && let Ok(name) = zbus::names::WellKnownName::try_from(SESSION_BUS_NAME)
    {
        dbus_proxy
            .name_has_owner(name.into())
            .await
            .unwrap_or(false)
    } else {
        false
    };

    let mut sync_accent = true;
    let mut auto_refresh = true;
    let mut power_monitor_active = false;
    let mut oled_care = true;
    let mut pixel_refresh_hours = (metrics.active_screen_seconds as f64) / 3600.0;
    let mut pixel_refresh_count = metrics.refresh_count;
    let mut oled_dim_level = metrics.oled_dim_level;
    let mut call_succeeded = false;

    if let Ok(proxy) = zbus::Proxy::new(
        &conn,
        SESSION_BUS_NAME,
        SESSION_OBJECT_PATH,
        SESSION_BUS_NAME,
    )
    .await
    {
        // 1. Try 6-tuple GetStatus (v0.3.2+)
        if let Ok((sa, ar, pa, oc, rh, rc)) =
            proxy.call("GetStatus", &()).await as Result<(bool, bool, bool, bool, f64, u32), _>
        {
            sync_accent = sa;
            auto_refresh = ar;
            power_monitor_active = pa;
            oled_care = oc;
            pixel_refresh_hours = rh;
            pixel_refresh_count = rc;
            call_succeeded = true;
        } else if let Ok((sa, ar, pa)) =
            proxy.call("GetStatus", &()).await as Result<(bool, bool, bool), _>
        {
            // 2. Fallback to 3-tuple GetStatus (earlier versions)
            sync_accent = sa;
            auto_refresh = ar;
            power_monitor_active = pa;
            call_succeeded = true;

            if let Ok(oc) = proxy.get_property::<bool>("OledCare").await {
                oled_care = oc;
            }
            if let Ok(rh) = proxy.get_property::<f64>("PixelRefreshHours").await {
                pixel_refresh_hours = rh;
            }
            if let Ok(rc) = proxy.get_property::<u32>("PixelRefreshCount").await {
                pixel_refresh_count = rc;
            }
        } else if has_owner {
            // 3. If GetStatus call failed or timed out but daemon is running, query properties directly
            if let Ok(sa) = proxy.get_property::<bool>("SyncAccent").await {
                sync_accent = sa;
            }
            if let Ok(ar) = proxy.get_property::<bool>("AutoRefresh").await {
                auto_refresh = ar;
            }
            if let Ok(pa) = proxy.get_property::<bool>("PowerMonitorActive").await {
                power_monitor_active = pa;
            }
            if let Ok(oc) = proxy.get_property::<bool>("OledCare").await {
                oled_care = oc;
            }
            if let Ok(rh) = proxy.get_property::<f64>("PixelRefreshHours").await {
                pixel_refresh_hours = rh;
            }
            if let Ok(rc) = proxy.get_property::<u32>("PixelRefreshCount").await {
                pixel_refresh_count = rc;
            }
        }

        if let Ok(odl) = proxy.get_property::<u32>("OledDimLevel").await {
            oled_dim_level = odl;
        }
    }

    SessionStatusInfo {
        running: has_owner || call_succeeded,
        desktop,
        sync_accent,
        auto_refresh,
        power_monitor_active,
        oled_care,
        pixel_refresh_hours,
        pixel_refresh_count,
        oled_dim_level,
    }
}

pub async fn set_session_oled_dim_level(level: u32) -> Result<(), Box<dyn std::error::Error>> {
    let level = level.clamp(10, 100);
    let mut metrics = load_oled_metrics();
    metrics.oled_dim_level = level;
    save_oled_metrics(&metrics);

    if let Ok(conn) = Connection::session().await {
        let proxy = zbus::Proxy::new(
            &conn,
            SESSION_BUS_NAME,
            SESSION_OBJECT_PATH,
            SESSION_BUS_NAME,
        )
        .await?;
        let () = proxy.call("SetOledDimLevel", &level).await?;
    }
    Ok(())
}

pub async fn trigger_session_pixel_refresh() -> Result<bool, Box<dyn std::error::Error>> {
    let conn = Connection::session().await?;
    let proxy = zbus::Proxy::new(
        &conn,
        SESSION_BUS_NAME,
        SESSION_OBJECT_PATH,
        SESSION_BUS_NAME,
    )
    .await?;
    let res: bool = proxy.call("TriggerPixelRefresh", &()).await?;
    Ok(res)
}

pub async fn set_session_accent_sync(enable: bool) -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::session().await?;
    let proxy = zbus::Proxy::new(
        &conn,
        SESSION_BUS_NAME,
        SESSION_OBJECT_PATH,
        SESSION_BUS_NAME,
    )
    .await?;
    let () = proxy.call("SetSyncAccent", &enable).await?;
    Ok(())
}

pub async fn set_session_auto_refresh(enable: bool) -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::session().await?;
    let proxy = zbus::Proxy::new(
        &conn,
        SESSION_BUS_NAME,
        SESSION_OBJECT_PATH,
        SESSION_BUS_NAME,
    )
    .await?;
    let () = proxy.call("SetAutoRefresh", &enable).await?;
    Ok(())
}

pub async fn set_session_oled_care(enable: bool) -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::session().await?;
    let proxy = zbus::Proxy::new(
        &conn,
        SESSION_BUS_NAME,
        SESSION_OBJECT_PATH,
        SESSION_BUS_NAME,
    )
    .await?;
    let () = proxy.call("SetOledCare", &enable).await?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisplayInfo {
    pub connector: String,
    pub width: u32,
    pub height: u32,
    pub refresh_rate: f64,
    pub auto_refresh: bool,
}

pub async fn query_display_info() -> DisplayInfo {
    let desktop = detect_desktop();
    let status = query_session_status().await;

    // 1. Try KDE via kscreen-doctor
    if desktop == DesktopEnv::Kde
        && let Some(mut cmd) = create_kscreen_doctor_cmd()
        && let Ok(output) = cmd.arg("-j").output()
        && output.status.success()
        && let Ok(json_str) = std::str::from_utf8(&output.stdout)
        && let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str)
        && let Some(outputs) = json.get("outputs").and_then(|o| o.as_array())
    {
        for out in outputs {
            let name = out.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let connected = out
                .get("connected")
                .and_then(|c| c.as_bool())
                .unwrap_or(true);
            if name.starts_with("eDP") && connected {
                let mut width = out
                    .get("size")
                    .and_then(|s| s.get("width"))
                    .and_then(|w| w.as_u64())
                    .unwrap_or(0) as u32;
                let mut height = out
                    .get("size")
                    .and_then(|s| s.get("height"))
                    .and_then(|h| h.as_u64())
                    .unwrap_or(0) as u32;
                let mut refresh = 60.0;

                if let Some(modes) = out.get("modes").and_then(|m| m.as_array()) {
                    for m in modes {
                        let is_cur = m.get("current").and_then(|c| c.as_bool()).unwrap_or(false)
                            || m.get("isCurrent")
                                .and_then(|c| c.as_bool())
                                .unwrap_or(false);
                        if is_cur {
                            if let Some(size) = m.get("size") {
                                if let Some(w) = size.get("width").and_then(|v| v.as_u64()) {
                                    width = w as u32;
                                }
                                if let Some(h) = size.get("height").and_then(|v| v.as_u64()) {
                                    height = h as u32;
                                }
                            }
                            if let Some(r) = m.get("refreshRate").and_then(|v| v.as_f64()) {
                                refresh = r;
                            }
                            break;
                        }
                    }
                }

                return DisplayInfo {
                    connector: name.to_string(),
                    width,
                    height,
                    refresh_rate: refresh,
                    auto_refresh: status.auto_refresh,
                };
            }
        }
    }

    // 2. Try GNOME Mutter DisplayConfig
    if desktop == DesktopEnv::Gnome
        && let Ok(conn) = Connection::session().await
        && let Ok(proxy) = zbus::Proxy::new(
            &conn,
            "org.gnome.Mutter.DisplayConfig",
            "/org/gnome/Mutter/DisplayConfig",
            "org.gnome.Mutter.DisplayConfig",
        )
        .await
        && let Ok(state) =
            proxy.call("GetCurrentState", &()).await as Result<MutterGetCurrentStateReturn, _>
    {
        let (_serial, monitors, _logical, _props) = state;
        if let Some(edp) = monitors.iter().find(|m| m.0.0.starts_with("eDP")) {
            let conn_name = edp.0.0.clone();
            let mut width = 0;
            let mut height = 0;
            let mut refresh = 60.0;

            for mode in &edp.1 {
                let is_cur = mode
                    .6
                    .get("is-current")
                    .and_then(|v| bool::try_from(v).ok())
                    .unwrap_or(false);
                if is_cur {
                    width = mode.1 as u32;
                    height = mode.2 as u32;
                    refresh = mode.3;
                    break;
                }
            }

            return DisplayInfo {
                connector: conn_name,
                width,
                height,
                refresh_rate: refresh,
                auto_refresh: status.auto_refresh,
            };
        }
    }

    // 3. Fallback: DRM sysfs or generic detection
    let mut fallback_connector = "eDP-1".to_string();
    let mut fallback_width = 2880;
    let mut fallback_height = 1800;

    if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.contains("eDP") {
                if let Some(idx) = name.find("eDP") {
                    fallback_connector = name[idx..].to_string();
                }
                if let Ok(modes) = std::fs::read_to_string(entry.path().join("modes"))
                    && let Some(first_mode) = modes.lines().next()
                {
                    let parts: Vec<&str> = first_mode.split('x').collect();
                    if parts.len() == 2
                        && let (Ok(w), Ok(h)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>())
                    {
                        fallback_width = w;
                        fallback_height = h;
                    }
                }
                break;
            }
        }
    }

    DisplayInfo {
        connector: fallback_connector,
        width: fallback_width,
        height: fallback_height,
        refresh_rate: 120.0,
        auto_refresh: status.auto_refresh,
    }
}

pub async fn set_panel_refresh_rate(target_hz: f64) -> Result<(), Box<dyn std::error::Error>> {
    let desktop = detect_desktop();
    if desktop == DesktopEnv::Kde {
        apply_kde_refresh_rate_manual(target_hz).await?;
    } else if desktop == DesktopEnv::Gnome {
        apply_gnome_refresh_rate_manual(target_hz).await?;
    } else {
        tracing::warn!("Manual refresh rate switching not supported on desktop {desktop:?}");
    }
    Ok(())
}

async fn apply_kde_refresh_rate_manual(target_hz: f64) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = create_kscreen_doctor_cmd().ok_or_else(|| {
        Box::<dyn std::error::Error>::from(
            "kscreen-doctor unavailable: Wayland environment unresolved",
        )
    })?;
    let output = cmd.arg("-j").output()?;
    if !output.status.success() {
        return Err("kscreen-doctor returned non-zero exit code".into());
    }

    let json_str = std::str::from_utf8(&output.stdout)?;
    let json: serde_json::Value = serde_json::from_str(json_str)?;

    let Some(outputs) = json.get("outputs").and_then(|o| o.as_array()) else {
        return Err("No outputs found in kscreen-doctor output".into());
    };

    for out in outputs {
        let name = out.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let connected = out
            .get("connected")
            .and_then(|c| c.as_bool())
            .unwrap_or(true);
        if !name.starts_with("eDP") || !connected {
            continue;
        }

        let Some(modes) = out.get("modes").and_then(|m| m.as_array()) else {
            continue;
        };

        let cur_size = out.get("size");
        let cur_w = cur_size
            .and_then(|s| s.get("width"))
            .and_then(|w| w.as_u64())
            .unwrap_or(0);
        let cur_h = cur_size
            .and_then(|s| s.get("height"))
            .and_then(|h| h.as_u64())
            .unwrap_or(0);

        let mut matching_modes: Vec<(&str, f64)> = Vec::new();
        for m in modes {
            let mode_id = m.get("id").and_then(|i| i.as_str()).unwrap_or("");
            let refresh = m.get("refreshRate").and_then(|r| r.as_f64()).unwrap_or(0.0);
            let size = m.get("size");
            let w = size
                .and_then(|s| s.get("width"))
                .and_then(|w| w.as_u64())
                .unwrap_or(0);
            let h = size
                .and_then(|s| s.get("height"))
                .and_then(|h| h.as_u64())
                .unwrap_or(0);

            if (cur_w == 0 || cur_h == 0) || (w == cur_w && h == cur_h) {
                matching_modes.push((mode_id, refresh));
            }
        }

        if matching_modes.is_empty() {
            continue;
        }

        matching_modes.sort_by(|a, b| {
            (a.1 - target_hz)
                .abs()
                .partial_cmp(&(b.1 - target_hz).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let target_mode = matching_modes[0];
        let mut switch_cmd = create_kscreen_doctor_cmd().ok_or_else(|| {
            Box::<dyn std::error::Error>::from(
                "kscreen-doctor unavailable: Wayland environment unresolved",
            )
        })?;
        let status = switch_cmd
            .arg(format!("output.{name}.mode.{}", target_mode.0))
            .status()?;
        if !status.success() {
            return Err(
                format!("Failed to switch kscreen-doctor mode to {}", target_mode.0).into(),
            );
        }
        return Ok(());
    }

    Err("No internal eDP panel found on KDE".into())
}

async fn apply_gnome_refresh_rate_manual(target_hz: f64) -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::session().await?;
    let proxy = zbus::Proxy::new(
        &conn,
        "org.gnome.Mutter.DisplayConfig",
        "/org/gnome/Mutter/DisplayConfig",
        "org.gnome.Mutter.DisplayConfig",
    )
    .await?;

    let state: MutterGetCurrentStateReturn = proxy.call("GetCurrentState", &()).await?;
    let (serial, monitors, logical_monitors, _properties) = state;

    let edp_monitor = monitors.iter().find(|m| m.0.0.starts_with("eDP"));
    let Some(edp_monitor) = edp_monitor else {
        return Err("GNOME Mutter: no internal eDP display found".into());
    };

    let edp_connector = &edp_monitor.0.0;
    let modes = &edp_monitor.1;

    let current_mode = modes.iter().find(|m| {
        m.6.get("is-current")
            .and_then(|v| bool::try_from(v).ok())
            .unwrap_or(false)
    });
    let preferred_mode = modes.iter().find(|m| {
        m.6.get("is-preferred")
            .and_then(|v| bool::try_from(v).ok())
            .unwrap_or(false)
    });

    let (target_w, target_h) = if let Some(cur) = current_mode {
        (cur.1, cur.2)
    } else if let Some(pref) = preferred_mode {
        (pref.1, pref.2)
    } else {
        modes
            .iter()
            .map(|m| (m.1, m.2))
            .max_by_key(|(w, h)| (*w as u64) * (*h as u64))
            .unwrap_or((2880, 1800))
    };

    let Some(target_mode) = pick_mutter_mode_for_rate(modes, (target_w, target_h), target_hz)
    else {
        return Err("GNOME Mutter: no matching mode for requested resolution".into());
    };

    let target_mode_id = &target_mode.0;
    let target_refresh = target_mode.3;

    let mut apply_logical_monitors: Vec<ApplyLogicalMonitor> = Vec::new();
    for log_mon in &logical_monitors {
        let x = log_mon.0;
        let y = log_mon.1;
        let scale = log_mon.2;
        let transform = log_mon.3;
        let primary = log_mon.4;
        let monitored_connectors = &log_mon.5;

        let mut assignments: Vec<ApplyMonitorAssignment> = Vec::new();
        for mc in monitored_connectors {
            let conn_name = &mc.0;
            if conn_name == edp_connector {
                assignments.push((
                    conn_name.clone(),
                    target_mode_id.clone(),
                    std::collections::HashMap::new(),
                ));
            } else {
                for other_mon in &monitors {
                    if &other_mon.0.0 == conn_name {
                        if let Some(cur) = other_mon.1.iter().find(|m| {
                            m.6.get("is-current")
                                .and_then(|v| bool::try_from(v).ok())
                                .unwrap_or(false)
                        }) {
                            assignments.push((
                                conn_name.clone(),
                                cur.0.clone(),
                                std::collections::HashMap::new(),
                            ));
                        }
                        break;
                    }
                }
            }
        }

        if !assignments.is_empty() {
            apply_logical_monitors.push((x, y, scale, transform, primary, assignments));
        }
    }

    let empty_props = std::collections::HashMap::<String, zbus::zvariant::Value<'static>>::new();
    let () = proxy
        .call(
            "ApplyMonitorsConfig",
            &(serial, 1u32, apply_logical_monitors, empty_props),
        )
        .await?;

    tracing::info!(
        "GNOME Mutter: successfully switched {edp_connector} to {target_mode_id} ({target_w}x{target_h}@{target_refresh:.1}Hz)"
    );
    Ok(())
}

#[derive(Debug, Clone)]
pub struct DesktopSessionOptions {
    pub sync_accent: bool,
    pub auto_refresh: bool,
    pub oled_care: bool,
}

pub async fn run_desktop_session(
    options: DesktopSessionOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = Arc::new(SessionState::new(
        options.sync_accent,
        options.auto_refresh,
        options.oled_care,
    ));

    let daemon_client =
        DaemonClient::connect_with_retry(30, std::time::Duration::from_millis(500)).await?;

    // Synchronize configured RGB inactivity timeout and policy to daemon
    let config = crate::services::config::load_config();
    let _ = daemon_client
        .set_rgb_timeout(config.rgb_timeout_seconds)
        .await;
    let _ = daemon_client
        .set_rgb_timeout_policy(&config.rgb_timeout_policy.to_string())
        .await;
    let session_conn = Connection::session().await?;
    let system_conn = daemon_client.connection();

    let desktop = detect_desktop();
    let kde_internal_connector = if desktop == DesktopEnv::Kde {
        find_kde_internal_connector().await
    } else {
        None
    };

    // Register Session D-Bus interface for live status, dynamic toggling, and pixel refresh
    let iface = SessionDbusInterface {
        state: Arc::clone(&state),
        session_conn: session_conn.clone(),
        desktop,
        internal_connector: kde_internal_connector.clone(),
    };
    let _ = session_conn
        .object_server()
        .at(SESSION_OBJECT_PATH, iface)
        .await;

    let _ = session_conn.request_name(SESSION_BUS_NAME).await;

    // Spawn kernel netlink uevent watcher for instant power changes
    let mut uevent_rx = crate::services::power_uevent::spawn_power_uevent_listener();
    let power_monitor_active = uevent_rx.is_some();
    state
        .power_monitor_active
        .store(power_monitor_active, Ordering::Relaxed);

    tracing::info!("Starting Alatus desktop session agent");
    tracing::info!("[Power Monitor: ACTIVE (Live uevent + D-Bus)]");
    tracing::info!(
        "[Panel Refresh Switching: {}]",
        if state.auto_refresh.load(Ordering::Relaxed) {
            "ACTIVE"
        } else {
            "DISABLED"
        }
    );
    tracing::info!(
        "[Desktop Accent Sync: {}]",
        if state.sync_accent.load(Ordering::Relaxed) {
            "ACTIVE"
        } else {
            "DISABLED"
        }
    );
    tracing::info!(
        "[OLED Care: {}]",
        if state.oled_care.load(Ordering::Relaxed) {
            "ACTIVE"
        } else {
            "DISABLED"
        }
    );

    // 1. Initial Accent Color Sync
    if state.sync_accent.load(Ordering::Relaxed) {
        let (r, g, b) = match read_portal_accent_color(&session_conn).await {
            Ok(Some((r, g, b))) => {
                tracing::info!("Initial XDG accent color: #{r:02X}{g:02X}{b:02X}");
                (r, g, b)
            }
            Ok(None) => {
                tracing::info!(
                    "No accent color returned by XDG Desktop Portal; falling back to ASUS Cyan (#00D2FF)"
                );
                (0x00, 0xD2, 0xFF)
            }
            Err(e) => {
                tracing::warn!(
                    "XDG Desktop Portal Settings read failed ({e}); falling back to ASUS Cyan (#00D2FF)"
                );
                (0x00, 0xD2, 0xFF)
            }
        };
        if let Err(e) = daemon_client.set_rgb_color(r, g, b).await {
            tracing::warn!("Failed to set initial RGB color via daemon: {e}");
        }
    }

    // 2. Initial Panel Refresh Rate Sync
    let mut last_known_on_ac = daemon_client.get_on_ac().await.unwrap_or(true);
    if state.auto_refresh.load(Ordering::Relaxed) {
        tracing::info!("Initial power state: OnAC={last_known_on_ac}");
        apply_panel_refresh_rate(last_known_on_ac).await;
    }

    // 3. Setup Match Rules
    let rule_portal = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.portal.Settings")?
        .member("SettingChanged")?
        .build();

    let rule_power = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("io.strixwolf.alatus.Daemon")?
        .member("PowerSourceChanged")?
        .build();

    let rule_thermal_osd = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("io.strixwolf.alatus.Daemon")?
        .member("ThermalOsdTriggered")?
        .build();

    let rule_screensaver_fdo = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.ScreenSaver")?
        .member("ActiveChanged")?
        .build();

    let rule_screensaver_gnome = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.gnome.ScreenSaver")?
        .member("ActiveChanged")?
        .build();

    let mut stream_portal =
        zbus::MessageStream::for_match_rule(rule_portal, &session_conn, Some(16)).await?;
    let mut stream_power =
        zbus::MessageStream::for_match_rule(rule_power, system_conn, Some(16)).await?;
    let mut stream_thermal_osd =
        zbus::MessageStream::for_match_rule(rule_thermal_osd, system_conn, Some(16)).await?;
    let mut stream_screensaver_fdo =
        zbus::MessageStream::for_match_rule(rule_screensaver_fdo, &session_conn, Some(16)).await?;
    let mut stream_screensaver_gnome =
        zbus::MessageStream::for_match_rule(rule_screensaver_gnome, &session_conn, Some(16))
            .await?;

    tracing::info!("Desktop session agent initialized and listening for events");

    let mut usage_timer = tokio::time::interval(tokio::time::Duration::from_secs(60));
    usage_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut last_notified_threshold_hours: u32 = 0;

    loop {
        tokio::select! {
            _ = usage_timer.tick() => {
                if !state.is_dimmed.load(Ordering::SeqCst) {
                    let current_secs = state.active_screen_seconds.fetch_add(60, Ordering::Relaxed) + 60;
                    let hours = (current_secs / 3600) as u32;

                    // Persist usage metrics periodically every 10 minutes
                    if current_secs.is_multiple_of(600) {
                        save_oled_metrics(&OledMetrics {
                            active_screen_seconds: current_secs,
                            refresh_count: state.pixel_refresh_count.load(Ordering::Relaxed),
                            last_refresh_timestamp: get_epoch_seconds(),
                            oled_dim_level: state.oled_dim_level.load(Ordering::Relaxed),
                        });
                    }

                    // Alert user after 4+ hours of active screen time
                    if hours >= 4 && hours > last_notified_threshold_hours && state.oled_care.load(Ordering::Relaxed) {
                        last_notified_threshold_hours = hours;
                        let summary = "ASUS OLED Care: Pixel Refresh Recommended";
                        let body = format!(
                            "Your display has been active for {hours} hours. Run 'alatus session pixel-refresh' or take a break to relieve OLED pixel stress."
                        );
                        let _ = send_desktop_notification(&session_conn, summary, &body).await;
                    }
                }
            }
            Some(msg_res) = stream_portal.next() => {
                if !state.sync_accent.load(Ordering::Relaxed) {
                    continue;
                }
                let Ok(msg) = msg_res else { continue };
                let Ok((namespace, key, val)) = msg.body().deserialize::<(String, String, zbus::zvariant::OwnedValue)>() else {
                    continue;
                };

                if namespace == "org.freedesktop.appearance"
                    && key == "accent-color"
                    && let Some((r, g, b)) = parse_accent_color(&val)
                {
                    tracing::info!("Accent color updated: #{r:02X}{g:02X}{b:02X}");
                    if let Err(e) = daemon_client.set_rgb_color(r, g, b).await {
                        tracing::warn!("Failed to set RGB color via daemon: {e}");
                    }
                }
            }
            Some(msg_res) = stream_power.next() => {
                let Ok(msg) = msg_res else { continue };
                if let Ok(on_ac) = msg.body().deserialize::<bool>() {
                    last_known_on_ac = on_ac;
                    if state.auto_refresh.load(Ordering::Relaxed) {
                        tracing::info!("Received PowerSourceChanged D-Bus signal: OnAC={on_ac}");
                        apply_panel_refresh_rate(on_ac).await;
                    }
                }
            }
            Some(msg_res) = stream_thermal_osd.next() => {
                let Ok(msg) = msg_res else { continue };
                let Ok((mode,)) = msg.body().deserialize::<(u32,)>() else {
                    continue;
                };
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
                let _ = send_osd_notification(&session_conn, summary, body, icon).await;
            }
            Some(msg_res) = stream_screensaver_fdo.next() => {
                let Ok(msg) = msg_res else { continue };
                if let Ok(is_idle) = msg.body().deserialize::<bool>() {
                    tracing::info!("ScreenSaver ActiveChanged (FDO) signal received: is_idle={is_idle}");
                    apply_oled_care_dimming(
                        &session_conn,
                        &daemon_client,
                        &state,
                        desktop,
                        kde_internal_connector.as_deref(),
                        is_idle,
                    ).await;
                    if !is_idle {
                        let _ = daemon_client.wake_rgb().await;
                    }
                }
            }
            Some(msg_res) = stream_screensaver_gnome.next() => {
                let Ok(msg) = msg_res else { continue };
                if let Ok(is_idle) = msg.body().deserialize::<bool>() {
                    tracing::info!("ScreenSaver ActiveChanged (GNOME) signal received: is_idle={is_idle}");
                    apply_oled_care_dimming(
                        &session_conn,
                        &daemon_client,
                        &state,
                        desktop,
                        kde_internal_connector.as_deref(),
                        is_idle,
                    ).await;
                    if !is_idle {
                        let _ = daemon_client.wake_rgb().await;
                    }
                }
            }
            Some(()) = async {
                match uevent_rx.as_mut() {
                    Some(rx) => rx.recv().await,
                    None => futures_util::future::pending().await,
                }
            } => {
                if let Ok(current_on_ac) = daemon_client.get_on_ac().await
                    && current_on_ac != last_known_on_ac
                {
                    last_known_on_ac = current_on_ac;
                    if state.auto_refresh.load(Ordering::Relaxed) {
                        tracing::info!(
                            "Kernel uevent power transition detected: OnAC={current_on_ac}"
                        );
                        apply_panel_refresh_rate(current_on_ac).await;
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Received exit signal. Terminating desktop session agent.");
                break;
            }
        }
    }

    Ok(())
}

pub async fn read_portal_accent_color(
    conn: &Connection,
) -> Result<Option<(u8, u8, u8)>, zbus::Error> {
    let proxy = zbus::Proxy::new(
        conn,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Settings",
    )
    .await?;

    let res: Result<zbus::zvariant::OwnedValue, _> = proxy
        .call("Read", &("org.freedesktop.appearance", "accent-color"))
        .await;

    match res {
        Ok(val) => Ok(parse_accent_color(&val)),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::{StructureBuilder, Value};

    #[test]
    fn test_parse_accent_color_from_struct() {
        // Build (0.2, 0.5, 1.0)
        let mut builder = StructureBuilder::new();
        builder.push_field(Value::F64(0.2));
        builder.push_field(Value::F64(0.5));
        builder.push_field(Value::F64(1.0));
        let structure = builder.build().expect("valid structure");
        let val = Value::Structure(structure);

        let rgb = parse_accent_color(&val).expect("should parse");
        assert_eq!(rgb.0, 51); // 0.2 * 255 = 51
        assert_eq!(rgb.1, 128); // 0.5 * 255 = 127.5 -> 128
        assert_eq!(rgb.2, 255); // 1.0 * 255 = 255
    }

    #[test]
    fn test_parse_accent_color_from_array() {
        let arr = vec![Value::F64(1.0), Value::F64(0.0), Value::F64(0.0)];
        let val = Value::from(arr);

        let rgb = parse_accent_color(&val).expect("should parse array");
        assert_eq!(rgb, (255, 0, 0));
    }

    #[test]
    fn test_parse_accent_color_nested_value() {
        let inner = Value::F64(0.5);
        let arr = vec![inner.clone(), inner.clone(), inner];
        let val = Value::Value(Box::new(Value::from(arr)));

        let rgb = parse_accent_color(&val).expect("should parse nested");
        assert_eq!(rgb, (128, 128, 128));
    }

    #[test]
    fn test_pick_mutter_mode_ac_and_battery() {
        let empty_props = std::collections::HashMap::new();
        let modes: Vec<MutterModeSpec> = vec![
            (
                "mode-60".to_string(),
                2880,
                1620,
                60.001,
                1.0,
                vec![1.0, 2.0],
                empty_props.clone(),
            ),
            (
                "mode-120".to_string(),
                2880,
                1620,
                120.003,
                1.0,
                vec![1.0, 2.0],
                empty_props.clone(),
            ),
            (
                "mode-lower-res".to_string(),
                1920,
                1080,
                120.0,
                1.0,
                vec![1.0],
                empty_props,
            ),
        ];

        // On AC: highest refresh rate for target resolution
        let ac_mode = pick_mutter_mode(&modes, (2880, 1620), true);
        assert!(ac_mode.is_some());
        assert_eq!(ac_mode.unwrap().0, "mode-120");
        assert_eq!(ac_mode.unwrap().1, 2880);
        assert_eq!(ac_mode.unwrap().2, 1620);
        assert!((ac_mode.unwrap().3 - 120.0).abs() < 0.1);

        // On Battery: ~60Hz for target resolution
        let bat_mode = pick_mutter_mode(&modes, (2880, 1620), false);
        assert!(bat_mode.is_some());
        assert_eq!(bat_mode.unwrap().0, "mode-60");
        assert_eq!(bat_mode.unwrap().1, 2880);
        assert_eq!(bat_mode.unwrap().2, 1620);
        assert!((bat_mode.unwrap().3 - 60.0).abs() < 0.1);

        // Resolution not found: None
        let missing_mode = pick_mutter_mode(&modes, (3840, 2160), true);
        assert!(missing_mode.is_none());
    }

    #[test]
    fn test_pick_mutter_mode_unusual_refresh_rates() {
        let empty_props = std::collections::HashMap::new();
        let modes: Vec<MutterModeSpec> = vec![
            (
                "mode-90".to_string(),
                2560,
                1440,
                90.0,
                1.0,
                vec![1.0],
                empty_props.clone(),
            ),
            (
                "mode-144".to_string(),
                2560,
                1440,
                144.0,
                1.0,
                vec![1.0],
                empty_props,
            ),
        ];

        // On AC: highest refresh rate (144Hz)
        let ac_mode = pick_mutter_mode(&modes, (2560, 1440), true);
        assert_eq!(ac_mode.unwrap().0, "mode-144");

        // On Battery: closest to 60Hz (90Hz)
        let bat_mode = pick_mutter_mode(&modes, (2560, 1440), false);
        assert_eq!(bat_mode.unwrap().0, "mode-90");

        // Explicit target rate tests
        let mode_90 = pick_mutter_mode_for_rate(&modes, (2560, 1440), 90.0);
        assert_eq!(mode_90.unwrap().0, "mode-90");

        let mode_144 = pick_mutter_mode_for_rate(&modes, (2560, 1440), 144.0);
        assert_eq!(mode_144.unwrap().0, "mode-144");

        let mode_closest = pick_mutter_mode_for_rate(&modes, (2560, 1440), 120.0);
        // 120 is closer to 144 (diff 24) than to 90 (diff 30)
        assert_eq!(mode_closest.unwrap().0, "mode-144");
    }

    #[test]
    fn test_load_save_oled_metrics_fallback() {
        let temp_dir = std::env::temp_dir().join(format!("alatus_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);

        let metrics_file = temp_dir.join("metrics.json");
        let legacy_file = temp_dir.join("session_metrics.json");

        // When legacy exists and metrics does not:
        let initial_metrics = OledMetrics {
            active_screen_seconds: 3600,
            refresh_count: 5,
            last_refresh_timestamp: 123456,
            oled_dim_level: 50,
        };
        save_oled_metrics_to(Some(&legacy_file), &initial_metrics);

        // Loading from metrics_file should fall back to legacy_file
        let loaded = load_oled_metrics_from(Some(&metrics_file));
        assert_eq!(loaded.active_screen_seconds, 3600);
        assert_eq!(loaded.refresh_count, 5);
        assert_eq!(loaded.oled_dim_level, 50);

        // Now save to metrics_file
        let updated_metrics = OledMetrics {
            active_screen_seconds: 7200,
            refresh_count: 6,
            last_refresh_timestamp: 123457,
            oled_dim_level: 70,
        };
        save_oled_metrics_to(Some(&metrics_file), &updated_metrics);

        // Loading should now load updated_metrics from metrics_file directly
        let loaded_updated = load_oled_metrics_from(Some(&metrics_file));
        assert_eq!(loaded_updated.active_screen_seconds, 7200);
        assert_eq!(loaded_updated.refresh_count, 6);
        assert_eq!(loaded_updated.oled_dim_level, 70);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_resolve_wayland_env_from_explicit_env() {
        let dummy_runtime = Path::new("/run/user/1000");
        let res = resolve_wayland_env_from(Some("wayland-1"), dummy_runtime, Some("KDE"));
        assert!(res.is_some());
        let env = res.unwrap();
        assert_eq!(env.wayland_display, "wayland-1");
        assert_eq!(env.xdg_runtime_dir, "/run/user/1000");
        assert_eq!(env.xdg_current_desktop.as_deref(), Some("KDE"));
    }

    #[test]
    fn test_resolve_wayland_env_from_socket_scan() {
        let temp_dir =
            std::env::temp_dir().join(format!("alatus_wayland_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);

        let socket_path = temp_dir.join("wayland-0");
        let lock_path = temp_dir.join("wayland-0.lock");
        let _ = fs::write(&socket_path, b"");
        let _ = fs::write(&lock_path, b"");

        let res = resolve_wayland_env_from(None, &temp_dir, None);
        assert!(res.is_some());
        let env = res.unwrap();
        assert_eq!(env.wayland_display, "wayland-0");
        assert_eq!(env.xdg_current_desktop.as_deref(), Some("KDE"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_resolve_wayland_env_from_no_sockets() {
        let temp_dir =
            std::env::temp_dir().join(format!("alatus_wayland_empty_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);

        let lock_path = temp_dir.join("wayland-0.lock");
        let _ = fs::write(&lock_path, b"");

        let res = resolve_wayland_env_from(None, &temp_dir, None);
        assert!(res.is_none());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_resolve_wayland_env_from_nonexistent_dir() {
        let dummy = Path::new("/nonexistent/runtime/dir/test_123");
        let res = resolve_wayland_env_from(None, dummy, None);
        assert!(res.is_none());
    }
}
