use alatus_drivers::SysfsRoot;
use alatus_ipc::BATTERY_OBJECT_PATH;
use alatus_profile::dmi::{DmiReader, ProfileResolver};
use alatus_registry::DriverRegistry;
use alatusd::BatteryService;
use std::error::Error;
use zbus::connection::Builder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting alatusd root system daemon v{}...", env!("CARGO_PKG_VERSION"));

    // 1. Read DMI
    let dmi_reader = DmiReader::new();
    let dmi_info = dmi_reader.read_dmi()?;
    tracing::info!(
        "Detected DMI: vendor={:?}, product={:?}, board={:?}",
        dmi_info.sys_vendor,
        dmi_info.product_name,
        dmi_info.board_name
    );

    // Clean up any legacy plaintext cached serial file so unprivileged reads are impossible
    let _ = std::fs::remove_file("/run/alatus/serial");

    // Ensure input event nodes are readable by user session for gestures & keyboard idle timeout
    ensure_input_permissions();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            ensure_input_permissions();
        }
    });

    // 2. Resolve Profile
    let resolver = ProfileResolver::default();
    let profile = resolver.resolve(&dmi_info)?;
    tracing::info!("Loaded hardware profile: {}", profile.metadata.name);

    let sysfs_root = SysfsRoot::default();
    let registry = DriverRegistry::with_default_drivers(sysfs_root);
    let drivers = registry.build_from_profile(&profile);

    // Restore daemon state from /var/lib/alatus/state.json if present
    if let Some((saved_limit, saved_profile)) = alatusd::state::load_daemon_state() {
        if let Some(limit) = saved_limit {
            if let Some(ref bat) = drivers.battery {
                let _ = bat.set_charge_limit(limit).await;
                tracing::info!("Restored battery charge limit on daemon start: {}%", limit);
            }
        }
        if let Some(prof_str) = saved_profile {
            if let Some(ref th) = drivers.thermal {
                let parsed_mode = match prof_str.to_lowercase().as_str() {
                    "quiet" => Some(alatus_core::thermal::ThermalProfileMode::Quiet),
                    "balanced" => Some(alatus_core::thermal::ThermalProfileMode::Balanced),
                    "performance" => Some(alatus_core::thermal::ThermalProfileMode::Performance),
                    "fullspeed" => Some(alatus_core::thermal::ThermalProfileMode::FullSpeed),
                    _ => None,
                };
                if let Some(mode) = parsed_mode {
                    let _ = th.set_profile(mode).await;
                    tracing::info!("Restored thermal profile on daemon start: {}", prof_str);
                }
            }
        }
    } else {
        if let Some(ref bat) = drivers.battery {
            if let Ok(info) = bat.get_info().await {
                if let Some(limit) = info.charge_limit {
                    alatusd::state::save_daemon_battery_limit(limit);
                }
            }
        }
        if let Some(ref th) = drivers.thermal {
            if let Ok(prof) = th.get_current_profile().await {
                alatusd::state::save_daemon_thermal_profile(&prof.to_string());
            }
        }
    }

    // 4. Set up D-Bus connection
    let mut builder = Builder::system()?;
    builder = builder.name("org.alatus.Daemon")?;

    if let Some(battery_driver) = drivers.battery {
        tracing::info!("Registering org.alatus.Battery D-Bus interface at {BATTERY_OBJECT_PATH}");
        let battery_svc = BatteryService::new(battery_driver);
        builder = builder.serve_at(BATTERY_OBJECT_PATH, battery_svc)?;
    } else {
        tracing::warn!("No battery driver available for this machine profile.");
    }

    if let Some(ref thermal_driver) = drivers.thermal {
        tracing::info!(
            "Registering org.alatus.Thermal D-Bus interface at {}",
            alatus_ipc::THERMAL_OBJECT_PATH
        );
        let thermal_svc = alatusd::ThermalService::new(thermal_driver.clone());
        builder = builder.serve_at(alatus_ipc::THERMAL_OBJECT_PATH, thermal_svc)?;
    } else {
        tracing::warn!("No thermal driver available for this machine profile.");
    }

    if let Some(lighting_driver) = drivers.lighting {
        tracing::info!(
            "Registering org.alatus.Lighting D-Bus interface at {}",
            alatus_ipc::LIGHTING_OBJECT_PATH
        );
        let lighting_svc = alatusd::LightingService::new(lighting_driver);
        builder = builder.serve_at(alatus_ipc::LIGHTING_OBJECT_PATH, lighting_svc)?;
    } else {
        tracing::warn!("No lighting driver available for this machine profile.");
    }

    tracing::info!(
        "Registering org.alatus.System D-Bus interface at {}",
        alatus_ipc::SYSTEM_SYSTEM_OBJECT_PATH
    );
    let system_svc = alatusd::SystemService::new();
    builder = builder.serve_at(alatus_ipc::SYSTEM_SYSTEM_OBJECT_PATH, system_svc)?;

    let conn = builder.build().await?;
    tracing::info!(
        "alatusd daemon successfully registered on system bus. Listening for requests..."
    );

    // 5. Start Hotkey background listener if available
    if let Some(mut hotkey_driver) = drivers.hotkeys {
        let thermal_opt = drivers.thermal.clone();
        let conn_clone = conn.clone();

        tokio::spawn(async move {
            tracing::info!("Starting background Asus WMI hotkey listener...");
            while let Ok(event) = hotkey_driver.next_event().await {
                tracing::info!("Received hotkey event: {:?}", event.action);
                if let alatus_core::hotkey::HotkeyAction::FanModeToggle = event.action {
                    if let Some(ref thermal) = thermal_opt {
                        if let Ok(available) = thermal.available_profiles().await {
                            if !available.is_empty() {
                                let current_res = thermal.get_current_profile().await;
                                let current = current_res.unwrap_or(available[0]);
                                let next_profile = match available.iter().position(|&p| p == current) {
                                    Some(idx) => available[(idx + 1) % available.len()],
                                    None => available[0],
                                };
                                tracing::info!(
                                    "Advancing thermal profile via hotkey: {} -> {}",
                                    current,
                                    next_profile
                                );
                                if let Err(e) = thermal.set_profile(next_profile).await {
                                    tracing::error!("Failed to update thermal profile: {e}");
                                } else {
                                    let next_str = next_profile.to_string();
                                    alatusd::state::save_daemon_thermal_profile(&next_str);
                                    if let Ok(path) = alatus_ipc::THERMAL_OBJECT_PATH.try_into()
                                    {
                                        let emitter =
                                            zbus::object_server::SignalContext::from_parts(
                                                conn_clone.clone(),
                                                path,
                                            );
                                        let _ = alatusd::ThermalService::profile_changed(
                                            &emitter, &next_str,
                                        )
                                        .await;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down alatusd...");
    Ok(())
}

fn ensure_input_permissions() {
    if let Ok(entries) = std::fs::read_dir("/dev/input") {
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if file_name.starts_with("event") {
                let dev_node = format!("/dev/input/{}", file_name);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(&dev_node, std::fs::Permissions::from_mode(0o666));
                }
            }
        }
    }
}
