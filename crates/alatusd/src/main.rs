mod battery_service;
mod polkit;

use alatus_drivers::SysfsRoot;
use alatus_ipc::BATTERY_OBJECT_PATH;
use alatus_profile::dmi::{DmiReader, ProfileResolver};
use alatus_registry::DriverRegistry;
use battery_service::BatteryService;
use std::error::Error;
use zbus::connection::Builder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting alatusd root system daemon v0.1.0...");

    // 1. Read DMI
    let dmi_reader = DmiReader::new();
    let dmi_info = dmi_reader.read_dmi()?;
    tracing::info!(
        "Detected DMI: vendor={:?}, product={:?}, board={:?}",
        dmi_info.sys_vendor,
        dmi_info.product_name,
        dmi_info.board_name
    );

    // 2. Resolve Profile
    let resolver = ProfileResolver::default();
    let profile = resolver.resolve(&dmi_info)?;
    tracing::info!("Loaded hardware profile: {}", profile.metadata.name);

    // 3. Build Drivers
    let sysfs_root = SysfsRoot::default();
    let registry = DriverRegistry::with_default_drivers(sysfs_root);
    let drivers = registry.build_from_profile(&profile);

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

    if let Some(thermal_driver) = drivers.thermal {
        tracing::info!(
            "Registering org.alatus.Thermal D-Bus interface at {}",
            alatus_ipc::THERMAL_OBJECT_PATH
        );
        let thermal_svc = alatusd::ThermalService::new(thermal_driver);
        builder = builder.serve_at(alatus_ipc::THERMAL_OBJECT_PATH, thermal_svc)?;
    } else {
        tracing::warn!("No thermal driver available for this machine profile.");
    }

    let _conn = builder.build().await?;
    tracing::info!("alatusd daemon successfully registered on system bus. Listening for requests...");

    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down alatusd...");
    Ok(())
}
