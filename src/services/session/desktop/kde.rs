// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! KDE Plasma KScreen display refresh rate integration.

use super::resolve_wayland_env;
use std::process::Command;

/// Creates a std::process::Command for `kscreen-doctor` with injected Wayland environment variables.
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

pub async fn apply_kde_panel_refresh(on_ac: bool) {
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
            matching_modes.last().unwrap()
        } else {
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

pub async fn set_kscreen_refresh_rate(hz: f64) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut cmd = create_kscreen_doctor_cmd()
        .ok_or("Failed to create kscreen-doctor command: Wayland environment unresolved")?;

    let output = cmd.arg("-j").output()?;
    if !output.status.success() {
        return Err("kscreen-doctor query failed".into());
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let outputs = json
        .get("outputs")
        .and_then(|o| o.as_array())
        .ok_or("No outputs in kscreen-doctor data")?;

    for out in outputs {
        let name = out.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let connected = out.get("connected").and_then(|c| c.as_bool()).unwrap_or(true);
        if !name.starts_with("eDP") || !connected {
            continue;
        }

        let modes = out
            .get("modes")
            .and_then(|m| m.as_array())
            .ok_or("No modes for eDP output")?;

        let mut current_res = (0u64, 0u64);
        if let Some(size) = out.get("size") {
            let w = size.get("width").and_then(|w| w.as_u64()).unwrap_or(0);
            let h = size.get("height").and_then(|h| h.as_u64()).unwrap_or(0);
            current_res = (w, h);
        }

        let mut candidate: Option<(&str, f64)> = None;
        let mut min_diff = f64::MAX;

        for m in modes {
            let size = match m.get("size") {
                Some(s) => s,
                None => continue,
            };
            let w = size.get("width").and_then(|w| w.as_u64()).unwrap_or(0);
            let h = size.get("height").and_then(|h| h.as_u64()).unwrap_or(0);

            if current_res.0 > 0 && (w != current_res.0 || h != current_res.1) {
                continue;
            }

            let id = m.get("id").and_then(|i| i.as_str()).unwrap_or("");
            let rate = m.get("refreshRate").and_then(|r| r.as_f64()).unwrap_or(0.0);
            let diff = (rate - hz).abs();
            if diff < min_diff {
                min_diff = diff;
                candidate = Some((id, rate));
            }
        }

        if let Some((mode_id, actual_rate)) = candidate {
            tracing::info!("KDE: Applying {actual_rate:.1}Hz mode {mode_id} to {name}");
            let mut apply_cmd = create_kscreen_doctor_cmd().ok_or("Failed to create kscreen-doctor")?;
            let status = apply_cmd.arg(format!("output.{name}.mode.{mode_id}")).status()?;
            if !status.success() {
                return Err("Failed to apply mode via kscreen-doctor".into());
            }
            return Ok(());
        }
    }

    Err("No matching display mode found".into())
}

pub async fn read_kde_screen_brightness(conn: &zbus::Connection) -> Result<i32, zbus::Error> {
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
    conn: &zbus::Connection,
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
    if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if let Some(idx) = file_name.find("eDP") {
                let conn_name = &file_name[idx..];
                let status_path = entry.path().join("status");
                if let Ok(status) = std::fs::read_to_string(&status_path)
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
        if let Ok(conn) = zbus::Connection::session().await {
            let _ = set_kde_screen_brightness(&conn, dim_value as i32).await;
        }
    }
}

