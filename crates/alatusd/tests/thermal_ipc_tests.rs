use alatus_drivers::{AsusHybridThermalDriver, SysfsRoot};
use alatus_ipc::{ThermalProxy, THERMAL_OBJECT_PATH};
use alatusd::ThermalService;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;
use zbus::connection::Builder;
use zbus::Connection;

#[tokio::test]
async fn test_thermal_service_and_proxy_over_session_dbus() {
    let dir = tempdir().unwrap();
    let acpi_dir = dir.path().join("sys/firmware/acpi");
    let debugfs_dir = dir.path().join("sys/kernel/debug/asus-nb-wmi");
    let hwmon_dir = dir
        .path()
        .join("sys/devices/platform/asus-nb-wmi/hwmon/hwmon1");
    fs::create_dir_all(&acpi_dir).unwrap();
    fs::create_dir_all(&debugfs_dir).unwrap();
    fs::create_dir_all(&hwmon_dir).unwrap();

    fs::write(acpi_dir.join("platform_profile"), "balanced\n").unwrap();
    fs::write(
        acpi_dir.join("platform_profile_choices"),
        "quiet balanced performance\n",
    )
    .unwrap();
    fs::write(debugfs_dir.join("devs"), "").unwrap();
    fs::write(hwmon_dir.join("fan1_input"), "4200\n").unwrap();
    fs::write(hwmon_dir.join("fan1_label"), "cpu_fan\n").unwrap();
    fs::write(hwmon_dir.join("fan2_input"), "4100\n").unwrap();
    fs::write(hwmon_dir.join("fan2_label"), "gpu_fan\n").unwrap();

    let root = SysfsRoot::new(dir.path().join("sys"));
    let driver = Arc::new(AsusHybridThermalDriver::new(
        root,
        "/firmware/acpi/platform_profile",
        Some("/kernel/debug/asus-nb-wmi/devs"),
        Some(0x00110013),
        Some(hwmon_dir.to_str().unwrap()),
        true,
    ));

    let service = ThermalService::new(driver);
    let test_bus_name = "org.alatus.TestDaemonThermal";

    let server_conn = match Builder::session() {
        Ok(b) => match b.name(test_bus_name) {
            Ok(b) => match b.serve_at(THERMAL_OBJECT_PATH, service) {
                Ok(b) => match b.build().await {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Session bus build failed (skipping headless test): {e}");
                        return;
                    }
                },
                Err(e) => {
                    eprintln!("serve_at failed: {e}");
                    return;
                }
            },
            Err(e) => {
                eprintln!("name failed: {e}");
                return;
            }
        },
        Err(e) => {
            eprintln!("Session bus connection failed: {e}");
            return;
        }
    };

    let client_conn = Connection::session().await.unwrap();

    let proxy = ThermalProxy::builder(&client_conn)
        .destination(test_bus_name)
        .unwrap()
        .path(THERMAL_OBJECT_PATH)
        .unwrap()
        .build()
        .await
        .unwrap();

    // 1. Check current profile
    let current = proxy
        .get_current_profile()
        .await
        .expect("get_current_profile");
    assert_eq!(current, "Balanced");

    // 2. Check available profiles
    let profiles = proxy.list_profiles().await.expect("list_profiles");
    assert!(profiles.contains(&"Quiet".to_string()));
    assert!(profiles.contains(&"Balanced".to_string()));
    assert!(profiles.contains(&"Performance".to_string()));
    assert!(profiles.contains(&"FullSpeed".to_string()));

    // 3. Check fans telemetry
    let fans = proxy.get_fans().await.expect("get_fans");
    assert_eq!(fans.len(), 2);
    assert_eq!(fans[0].label, "cpu_fan");
    assert_eq!(fans[0].current_rpm, 4200);
    assert_eq!(fans[1].label, "gpu_fan");
    assert_eq!(fans[1].current_rpm, 4100);

    // 4. Switch to Full Speed
    proxy
        .set_profile("FullSpeed".into())
        .await
        .expect("set FullSpeed");
    assert_eq!(proxy.get_current_profile().await.unwrap(), "FullSpeed");
    let devs_content = fs::read_to_string(debugfs_dir.join("devs")).unwrap();
    assert_eq!(devs_content.trim(), "0x00110013 0x1");

    // 5. Switch to Quiet
    proxy.set_profile("Quiet".into()).await.expect("set Quiet");
    assert_eq!(proxy.get_current_profile().await.unwrap(), "Quiet");
    let devs_reverted = fs::read_to_string(debugfs_dir.join("devs")).unwrap();
    assert_eq!(devs_reverted.trim(), "0x00110013 0x0");
    let profile_content = fs::read_to_string(acpi_dir.join("platform_profile")).unwrap();
    assert_eq!(profile_content.trim(), "quiet");

    drop(server_conn);
}
