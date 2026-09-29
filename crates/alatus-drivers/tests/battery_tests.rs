use alatus_core::battery::{BatteryDriver, BatteryStatus};
use alatus_drivers::{AsusSysfsBatteryDriver, SysfsRoot};
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn test_battery_driver_read_and_write() {
    let dir = tempdir().unwrap();
    let bat_dir = dir.path().join("sys/class/power_supply/BAT0");
    fs::create_dir_all(&bat_dir).unwrap();

    fs::write(bat_dir.join("status"), "Discharging\n").unwrap();
    fs::write(bat_dir.join("capacity"), "85\n").unwrap();
    fs::write(bat_dir.join("charge_control_end_threshold"), "80\n").unwrap();
    fs::write(bat_dir.join("energy_now"), "60000000\n").unwrap();
    fs::write(bat_dir.join("energy_full"), "72000000\n").unwrap();
    fs::write(bat_dir.join("energy_full_design"), "75000000\n").unwrap();
    fs::write(bat_dir.join("power_now"), "12000000\n").unwrap();

    let root = SysfsRoot::new(dir.path().join("sys"));
    let driver = AsusSysfsBatteryDriver::new(root, "/class/power_supply/BAT0", vec![60, 80, 100]);

    // Check Info
    let info = driver.get_info().await.expect("Failed to get info");
    assert_eq!(info.percentage, 85);
    assert_eq!(info.status, BatteryStatus::Discharging);
    assert_eq!(info.charge_limit, Some(80));
    assert_eq!(info.health_percentage, Some(96)); // 72 / 75 * 100 = 96
    assert_eq!(info.power_now_microwatts, Some(12000000));

    // Test writing threshold: 60%
    driver
        .set_charge_limit(60)
        .await
        .expect("Failed to set 60%");
    let updated = fs::read_to_string(bat_dir.join("charge_control_end_threshold")).unwrap();
    assert_eq!(updated.trim(), "60");

    // Test writing threshold: 100%
    driver
        .set_charge_limit(100)
        .await
        .expect("Failed to set 100%");
    let updated = fs::read_to_string(bat_dir.join("charge_control_end_threshold")).unwrap();
    assert_eq!(updated.trim(), "100");

    // Test invalid limit rejection
    let err = driver.set_charge_limit(75).await.unwrap_err();
    assert!(err.to_string().contains("not supported"));
}

#[tokio::test]
async fn test_battery_driver_fallback_threshold_file() {
    let dir = tempdir().unwrap();
    let bat_dir = dir.path().join("sys/class/power_supply/BAT0");
    fs::create_dir_all(&bat_dir).unwrap();

    // Use charge_control_limit_max instead of charge_control_end_threshold
    fs::write(bat_dir.join("charge_control_limit_max"), "80\n").unwrap();
    fs::write(bat_dir.join("capacity"), "50\n").unwrap();
    fs::write(bat_dir.join("status"), "Charging\n").unwrap();

    let root = SysfsRoot::new(dir.path().join("sys"));
    let driver = AsusSysfsBatteryDriver::new(root, "/class/power_supply/BAT0", vec![60, 80, 100]);

    assert_eq!(driver.get_charge_limit().await.unwrap(), 80);
    assert!(driver.is_charging().await.unwrap());

    driver.set_charge_limit(60).await.unwrap();
    let updated = fs::read_to_string(bat_dir.join("charge_control_limit_max")).unwrap();
    assert_eq!(updated.trim(), "60");
}
