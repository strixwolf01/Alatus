use alatus_drivers::{AsusSysfsBatteryDriver, SysfsRoot};
use alatus_ipc::{BatteryProxy, BATTERY_OBJECT_PATH};
use alatusd::BatteryService;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;
use zbus::connection::Builder;
use zbus::Connection;

#[tokio::test]
async fn test_battery_service_and_proxy_over_session_dbus() {
    let dir = tempdir().unwrap();
    let bat_dir = dir.path().join("sys/class/power_supply/BAT0");
    fs::create_dir_all(&bat_dir).unwrap();

    fs::write(bat_dir.join("status"), "Charging\n").unwrap();
    fs::write(bat_dir.join("capacity"), "75\n").unwrap();
    fs::write(bat_dir.join("charge_control_end_threshold"), "80\n").unwrap();
    fs::write(bat_dir.join("energy_now"), "50000000\n").unwrap();
    fs::write(bat_dir.join("energy_full"), "70000000\n").unwrap();
    fs::write(bat_dir.join("energy_full_design"), "75000000\n").unwrap();
    fs::write(bat_dir.join("power_now"), "15000000\n").unwrap();

    let root = SysfsRoot::new(dir.path().join("sys"));
    let driver = Arc::new(AsusSysfsBatteryDriver::new(
        root,
        "/class/power_supply/BAT0",
        vec![60, 80, 100],
    ));

    let service = BatteryService::new(driver);

    // Unique bus name for this test run
    let test_bus_name = "org.alatus.TestDaemonBattery";

    let server_conn = match Builder::session() {
        Ok(b) => match b.name(test_bus_name) {
            Ok(b) => match b.serve_at(BATTERY_OBJECT_PATH, service) {
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

    let proxy = BatteryProxy::builder(&client_conn)
        .destination(test_bus_name)
        .unwrap()
        .path(BATTERY_OBJECT_PATH)
        .unwrap()
        .build()
        .await
        .unwrap();

    // 1. Query info through D-Bus proxy
    let info = proxy.get_info().await.expect("get_info over D-Bus");
    assert_eq!(info.percentage, 75);
    assert_eq!(info.status, "Charging");
    assert_eq!(info.charge_limit, Some(80));
    assert_eq!(info.health_percentage, Some(93)); // 70 / 75 * 100 = 93
    assert_eq!(info.power_now_microwatts, Some(15000000));

    // 2. Query charge limit
    let limit = proxy.get_charge_limit().await.expect("get_charge_limit over D-Bus");
    assert_eq!(limit, 80);

    // 3. Set charge limit over D-Bus
    proxy.set_charge_limit(60).await.expect("set_charge_limit over D-Bus");

    // 4. Verify file on disk
    let updated_limit = fs::read_to_string(bat_dir.join("charge_control_end_threshold")).unwrap();
    assert_eq!(updated_limit.trim(), "60");

    drop(server_conn);
}
