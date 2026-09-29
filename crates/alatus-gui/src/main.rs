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
        if let Ok(current) = tp.get_current_profile().await {
            let handle_clone = handle.clone();
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = handle_clone.upgrade() {
                    ui.set_current_profile(current.into());
                }
            })?;
        }
        if let Ok(fans) = tp.get_fans().await {
            let handle_clone = handle.clone();
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = handle_clone.upgrade() {
                    if let Some(fan1) = fans.first() {
                        ui.set_cpu_fan_rpm(fan1.current_rpm as i32);
                    }
                    if let Some(fan2) = fans.get(1) {
                        ui.set_gpu_fan_rpm(fan2.current_rpm as i32);
                    }
                }
            })?;
        }
    }

    // 3. Initial Data Query: Lighting
    let lighting_proxy = LightingProxy::new(&conn).await.ok();
    if let Some(ref lp) = lighting_proxy {
        if let Ok(state) = lp.get_state().await {
            let handle_clone = handle.clone();
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = handle_clone.upgrade() {
                    ui.set_lighting_brightness(state.brightness as i32);
                    ui.set_lighting_mode(state.mode.into());
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
        main_window.on_set_lighting_color(move |r, g, b| {
            let lp_clone = lp_color.clone();
            tokio::spawn(async move {
                let _ = lp_clone.set_color(r as u8, g as u8, b as u8).await;
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

    // 7. Background Signal Monitor for Live Hardware Updates
    if let Some(tp) = thermal_proxy {
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

    main_window.run()?;
    Ok(())
}
