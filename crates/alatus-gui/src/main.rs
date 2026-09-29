slint::include_modules!();

use alatus_ipc::{BatteryProxy, LightingProxy, ThermalProxy};
use futures_util::StreamExt;
use std::error::Error;
use zbus::Connection;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting Alatus GUI...");

    let main_window = MainWindow::new()?;
    let handle = main_window.as_weak();

    let conn = Connection::system().await.map_err(|e| {
        format!("Failed to connect to system D-Bus. Is alatusd daemon running? Error: {e}")
    })?;

    // 1. Initial Data Query: Battery
    let battery_proxy = BatteryProxy::new(&conn).await.ok();
    if let Some(ref bp) = battery_proxy {
        if let Ok(info) = bp.get_info().await {
            let handle_clone = handle.clone();
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = handle_clone.upgrade() {
                    ui.set_battery_percentage(info.percentage as i32);
                    ui.set_battery_status(info.status.into());
                    if let Some(limit) = info.charge_limit {
                        ui.set_battery_limit(limit as i32);
                    }
                    if let Some(health) = info.health_percentage {
                        ui.set_battery_health(health as i32);
                    }
                    if let Some(microwatts) = info.power_now_microwatts {
                        let watts = microwatts as f64 / 1_000_000.0;
                        ui.set_battery_power(format!("{:.1} W", watts).into());
                    }
                }
            })?;
        }
    }

    // 2. Initial Data Query: Thermal
    let thermal_proxy = ThermalProxy::new(&conn).await.ok();
    if let Some(ref tp) = thermal_proxy {
        let is_cpu_only = tp.is_cpu_only().await.unwrap_or(true);
        let handle_clone = handle.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = handle_clone.upgrade() {
                ui.set_is_cpu_only(is_cpu_only);
            }
        });

        if let Ok(current) = tp.get_current_profile().await {
            let handle_clone = handle.clone();
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = handle_clone.upgrade() {
                    ui.set_current_profile(current.into());
                }
            })?;
        }
        if let Ok(fans) = tp.get_fans().await {
            let (cpu_only_text, rpm_subtext, cpu_rpm, gpu_rpm) = format_fan_telemetry(&fans);
            let handle_clone = handle.clone();
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = handle_clone.upgrade() {
                    ui.set_cpu_fan_rpm(cpu_rpm);
                    ui.set_gpu_fan_rpm(gpu_rpm);
                    ui.set_fan_cpu_only_text(cpu_only_text.into());
                    ui.set_fan_rpm_subtext(rpm_subtext.into());
                }
            })?;
        }
    }

    // 3. Initial Data Query: Lighting
    let lighting_proxy = LightingProxy::new(&conn).await.ok();
    if let Some(ref lp) = lighting_proxy {
        if let Ok(state) = lp.get_state().await {
            let handle_clone = handle.clone();
            let has_multiple_modes = state.supported_modes.len() > 1;
            let supports_breathing = state
                .supported_modes
                .iter()
                .any(|m| m.eq_ignore_ascii_case("breathing"));
            let supports_rainbow = state
                .supported_modes
                .iter()
                .any(|m| m.eq_ignore_ascii_case("rainbow"));
            let supports_strobe = state
                .supported_modes
                .iter()
                .any(|m| m.eq_ignore_ascii_case("strobe"));
            let hex = format!("#{:02X}{:02X}{:02X}", state.r, state.g, state.b);
            let r = state.r;
            let g = state.g;
            let b = state.b;

            slint::invoke_from_event_loop(move || {
                if let Some(ui) = handle_clone.upgrade() {
                    ui.set_lighting_brightness(state.brightness as i32);
                    ui.set_lighting_mode(state.mode.into());
                    ui.set_has_multiple_modes(has_multiple_modes);
                    ui.set_supports_mode_breathing(supports_breathing);
                    ui.set_supports_mode_rainbow(supports_rainbow);
                    ui.set_supports_mode_strobe(supports_strobe);
                    ui.set_lighting_hex(hex.into());
                    ui.set_current_rgb_color(slint::Color::from_rgb_u8(r, g, b));
                }
            })?;
        }
    }

    // 4. Wire UI Callbacks: Battery Limit
    if let Some(bp) = battery_proxy.clone() {
        let handle_clone = handle.clone();
        main_window.on_set_battery_limit(move |limit| {
            let bp_clone = bp.clone();
            let handle_inner = handle_clone.clone();
            tokio::spawn(async move {
                if let Ok(()) = bp_clone.set_charge_limit(limit as u8).await {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = handle_inner.upgrade() {
                            ui.set_battery_limit(limit);
                        }
                    });
                }
            });
        });
    }

    // 5. Wire UI Callbacks: Thermal Profile
    if let Some(tp) = thermal_proxy.clone() {
        let handle_clone = handle.clone();
        main_window.on_set_thermal_profile(move |mode| {
            let tp_clone = tp.clone();
            let mode_str = mode.to_string();
            let handle_inner = handle_clone.clone();
            tokio::spawn(async move {
                if let Ok(()) = tp_clone.set_profile(mode_str.clone()).await {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = handle_inner.upgrade() {
                            ui.set_current_profile(mode_str.into());
                        }
                    });
                }
            });
        });
    }

    // 6. Wire UI Callbacks: Lighting
    if let Some(lp) = lighting_proxy.clone() {
        let lp_brightness = lp.clone();
        let handle_b = handle.clone();
        main_window.on_set_lighting_brightness(move |level| {
            let lp_clone = lp_brightness.clone();
            let handle_inner = handle_b.clone();
            tokio::spawn(async move {
                if let Ok(()) = lp_clone.set_brightness(level as u8).await {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = handle_inner.upgrade() {
                            ui.set_lighting_brightness(level);
                        }
                    });
                }
            });
        });

        let lp_color = lp.clone();
        let handle_c = handle.clone();
        main_window.on_set_lighting_color(move |r, g, b| {
            let lp_clone = lp_color.clone();
            let handle_inner = handle_c.clone();
            let hex = format!("#{r:02X}{g:02X}{b:02X}");
            tokio::spawn(async move {
                let _ = lp_clone.set_color(r as u8, g as u8, b as u8).await;
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = handle_inner.upgrade() {
                        ui.set_lighting_hex(hex.into());
                        ui.set_current_rgb_color(slint::Color::from_rgb_u8(
                            r as u8, g as u8, b as u8,
                        ));
                    }
                });
            });
        });

        let lp_picker = lp.clone();
        let handle_p = handle.clone();
        main_window.on_open_de_color_picker(move || {
            let lp_clone = lp_picker.clone();
            let handle_inner = handle_p.clone();
            tokio::spawn(async move {
                if let Some((r, g, b)) = pick_color_from_de().await {
                    let _ = lp_clone.set_color(r, g, b).await;
                    let hex = format!("#{r:02X}{g:02X}{b:02X}");
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = handle_inner.upgrade() {
                            ui.set_lighting_hex(hex.into());
                            ui.set_current_rgb_color(slint::Color::from_rgb_u8(r, g, b));
                        }
                    });
                }
            });
        });

        let lp_mode = lp.clone();
        let handle_m = handle.clone();
        main_window.on_set_lighting_mode(move |mode, speed| {
            let lp_clone = lp_mode.clone();
            let mode_str = mode.to_string();
            let handle_inner = handle_m.clone();
            tokio::spawn(async move {
                if let Ok(()) = lp_clone.set_mode(mode_str.clone(), speed as u8).await {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = handle_inner.upgrade() {
                            ui.set_lighting_mode(mode_str.into());
                        }
                    });
                }
            });
        });
    }

    // 7. Background Signal Monitors for Live Hardware Updates
    if let Some(tp) = thermal_proxy.clone() {
        let handle_t = handle.clone();
        tokio::spawn(async move {
            if let Ok(mut stream) = tp.receive_profile_changed().await {
                while let Some(signal) = stream.next().await {
                    if let Ok(args) = signal.args() {
                        let new_profile = args.new_profile;
                        let handle_inner = handle_t.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = handle_inner.upgrade() {
                                ui.set_current_profile(new_profile.into());
                            }
                        });
                    }
                }
            }
        });
    }

    if let Some(lp) = lighting_proxy {
        let handle_l = handle.clone();
        tokio::spawn(async move {
            if let Ok(mut stream) = lp.receive_state_changed().await {
                while let Some(signal) = stream.next().await {
                    if let Ok(args) = signal.args() {
                        let state = args.state;
                        let handle_inner = handle_l.clone();
                        let hex = format!("#{:02X}{:02X}{:02X}", state.r, state.g, state.b);
                        let r = state.r;
                        let g = state.g;
                        let b = state.b;
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = handle_inner.upgrade() {
                                ui.set_lighting_brightness(state.brightness as i32);
                                ui.set_lighting_mode(state.mode.into());
                                ui.set_lighting_hex(hex.into());
                                ui.set_current_rgb_color(slint::Color::from_rgb_u8(r, g, b));
                            }
                        });
                    }
                }
            }
        });
    }

    // 8. Periodic Polling for Live Telemetry (Battery & Fans)
    let poller_timer = slint::Timer::default();
    {
        let bp_poll = battery_proxy.clone();
        let tp_poll = thermal_proxy.clone();
        let handle_poll = handle.clone();

        poller_timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(1500),
            move || {
                let bp = bp_poll.clone();
                let tp = tp_poll.clone();
                let handle_inner = handle_poll.clone();

                tokio::spawn(async move {
                    if let Some(ref bp) = bp {
                        if let Ok(info) = bp.get_info().await {
                            let h = handle_inner.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = h.upgrade() {
                                    ui.set_battery_percentage(info.percentage as i32);
                                    ui.set_battery_status(info.status.into());
                                    if let Some(limit) = info.charge_limit {
                                        ui.set_battery_limit(limit as i32);
                                    }
                                    if let Some(health) = info.health_percentage {
                                        ui.set_battery_health(health as i32);
                                    }
                                    if let Some(microwatts) = info.power_now_microwatts {
                                        let watts = microwatts as f64 / 1_000_000.0;
                                        ui.set_battery_power(format!("{:.1} W", watts).into());
                                    }
                                }
                            });
                        }
                    }

                    if let Some(ref tp) = tp {
                        if let Ok(fans) = tp.get_fans().await {
                            let (cpu_only_text, rpm_subtext, cpu_rpm, gpu_rpm) =
                                format_fan_telemetry(&fans);
                            let h = handle_inner.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = h.upgrade() {
                                    ui.set_cpu_fan_rpm(cpu_rpm);
                                    ui.set_gpu_fan_rpm(gpu_rpm);
                                    ui.set_fan_cpu_only_text(cpu_only_text.into());
                                    ui.set_fan_rpm_subtext(rpm_subtext.into());
                                }
                            });
                        }

                        if let Ok(current) = tp.get_current_profile().await {
                            let h = handle_inner.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = h.upgrade() {
                                    if ui.get_current_profile() != current.as_str() {
                                        ui.set_current_profile(current.into());
                                    }
                                }
                            });
                        }
                    }
                });
            },
        );
    }

    main_window.run()?;
    Ok(())
}

fn format_fan_telemetry(fans: &[alatus_ipc::FanStatusMsg]) -> (String, String, i32, i32) {
    let cpu_rpm = fans.first().map(|f| f.current_rpm as i32).unwrap_or(0);
    let gpu_rpm = fans.get(1).map(|f| f.current_rpm as i32).unwrap_or(0);

    let default_max = 8100;
    let max_rpm_1 = fans.first().and_then(|f| f.max_rpm).unwrap_or(default_max) as u64;
    let max_rpm_2 = fans.get(1).and_then(|f| f.max_rpm).unwrap_or(default_max) as u64;

    let pct1 = (cpu_rpm.max(0) as u64 * 100)
        .checked_div(max_rpm_1)
        .unwrap_or(0)
        .min(100);
    let pct2 = (gpu_rpm.max(0) as u64 * 100)
        .checked_div(max_rpm_2)
        .unwrap_or(0)
        .min(100);

    let (cpu_only_text, rpm_subtext) = if fans.len() >= 2 {
        (
            format!(" CPU : {}%/{}% ", pct1, pct2),
            format!("{} RPM / {} RPM", cpu_rpm, gpu_rpm),
        )
    } else if let Some(fan) = fans.first() {
        (
            format!(" CPU : {}% ", pct1),
            format!("{} RPM", fan.current_rpm),
        )
    } else {
        (" CPU : 0%/0% ".to_string(), "".to_string())
    };

    (cpu_only_text, rpm_subtext, cpu_rpm, gpu_rpm)
}

async fn pick_color_from_de() -> Option<(u8, u8, u8)> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_lowercase();
    let is_kde = desktop.contains("kde");

    let output = if is_kde && std::path::Path::new("/usr/bin/kdialog").exists() {
        tokio::process::Command::new("kdialog")
            .arg("--getcolor")
            .output()
            .await
            .ok()
    } else if std::path::Path::new("/usr/bin/zenity").exists() {
        tokio::process::Command::new("zenity")
            .arg("--color-selection")
            .arg("--show-palette")
            .output()
            .await
            .ok()
    } else if std::path::Path::new("/usr/bin/kdialog").exists() {
        tokio::process::Command::new("kdialog")
            .arg("--getcolor")
            .output()
            .await
            .ok()
    } else {
        None
    };

    if let Some(out) = output {
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            return parse_color_string(&stdout);
        }
    }
    None
}

fn parse_color_string(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Format 1: #RRGGBB or #RRGGBBAA or RRGGBB
    let hex_part = s.trim_start_matches('#');
    if hex_part.len() >= 6 && hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
        let r = u8::from_str_radix(&hex_part[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex_part[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex_part[4..6], 16).ok()?;
        return Some((r, g, b));
    }

    // Format 2: rgb(r, g, b) or rgba(r, g, b, a)
    if let Some(inner) = s
        .strip_prefix("rgb(")
        .and_then(|t| t.strip_suffix(')'))
        .or_else(|| s.strip_prefix("rgba(").and_then(|t| t.strip_suffix(')')))
    {
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() >= 3 {
            let r = parts[0].trim().parse::<u8>().ok()?;
            let g = parts[1].trim().parse::<u8>().ok()?;
            let b = parts[2].trim().parse::<u8>().ok()?;
            return Some((r, g, b));
        }
    }

    None
}
