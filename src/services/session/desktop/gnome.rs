// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! GNOME Mutter DisplayConfig D-Bus display refresh rate integration.

use zbus::Connection;

pub type MutterMonitorSpec = (String, String, String, String);
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

pub type MutterGetCurrentStateReturn = (
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

pub async fn apply_gnome_panel_refresh(on_ac: bool) {
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

pub async fn set_gnome_refresh_rate(hz: f64) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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

    let edp_monitor = monitors
        .iter()
        .find(|m| m.0.0.starts_with("eDP"))
        .ok_or("No internal eDP display found")?;

    let edp_connector = &edp_monitor.0.0;
    let modes = &edp_monitor.1;

    let current_mode = modes.iter().find(|m| {
        m.6.get("is-current")
            .and_then(|v| bool::try_from(v).ok())
            .unwrap_or(false)
    });

    let (target_w, target_h) = if let Some(cur) = current_mode {
        (cur.1, cur.2)
    } else {
        modes
            .iter()
            .map(|m| (m.1, m.2))
            .max_by_key(|&(w, h)| (w as i64) * (h as i64))
            .unwrap_or((0, 0))
    };

    let target_mode = pick_mutter_mode_for_rate(modes, (target_w, target_h), hz)
        .ok_or("No matching mode found for specified rate")?;

    let target_mode_id = &target_mode.0;
    let mut apply_logical_monitors: Vec<ApplyLogicalMonitor> = Vec::new();

    for lm in &logical_monitors {
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
        apply_logical_monitors.push((lm.0, lm.1, lm.2, lm.3, lm.4, assignments));
    }

    let empty_props = std::collections::HashMap::<String, zbus::zvariant::Value<'static>>::new();
    proxy
        .call::<_, _, ()>(
            "ApplyMonitorsConfig",
            &(serial, 1u32, apply_logical_monitors, empty_props),
        )
        .await?;

    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

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
}

