use alatus_core::capabilities::ThermalCapabilities;
use alatus_core::thermal::{ThermalDriver, ThermalProfileMode};
use alatus_drivers::{AsusHybridThermalDriver, SysfsRoot};
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn test_thermal_standard_profiles() {
    let dir = tempdir().unwrap();
    let acpi_dir = dir.path().join("sys/firmware/acpi");
    fs::create_dir_all(&acpi_dir).unwrap();

    fs::write(acpi_dir.join("platform_profile"), "balanced\n").unwrap();
    fs::write(
        acpi_dir.join("platform_profile_choices"),
        "quiet balanced performance\n",
    )
    .unwrap();

    let root = SysfsRoot::new(dir.path().join("sys"));
    let driver = AsusHybridThermalDriver::new(
        root,
        "/firmware/acpi/platform_profile",
        None::<&str>,
        None,
        None::<&str>,
        false,
    );

    let caps = driver.capabilities();
    assert!(caps.contains(ThermalCapabilities::PLATFORM_PROFILE_STANDARD));
    assert!(!caps.contains(ThermalCapabilities::FULL_SPEED_FAN));

    let profiles = driver.available_profiles().await.unwrap();
    assert_eq!(
        profiles,
        vec![
            ThermalProfileMode::Quiet,
            ThermalProfileMode::Balanced,
            ThermalProfileMode::Performance,
        ]
    );

    assert_eq!(
        driver.get_current_profile().await.unwrap(),
        ThermalProfileMode::Balanced
    );

    // Switch to performance
    driver.set_profile(ThermalProfileMode::Performance).await.unwrap();
    assert_eq!(
        driver.get_current_profile().await.unwrap(),
        ThermalProfileMode::Performance
    );
    assert_eq!(
        fs::read_to_string(acpi_dir.join("platform_profile")).unwrap().trim(),
        "performance"
    );

    // Switch to quiet
    driver.set_profile(ThermalProfileMode::Quiet).await.unwrap();
    assert_eq!(
        driver.get_current_profile().await.unwrap(),
        ThermalProfileMode::Quiet
    );
}

#[tokio::test]
async fn test_thermal_full_speed_with_debugfs() {
    let dir = tempdir().unwrap();
    let acpi_dir = dir.path().join("sys/firmware/acpi");
    let debugfs_dir = dir.path().join("sys/kernel/debug/asus-nb-wmi");
    fs::create_dir_all(&acpi_dir).unwrap();
    fs::create_dir_all(&debugfs_dir).unwrap();

    fs::write(acpi_dir.join("platform_profile"), "balanced\n").unwrap();
    fs::write(
        acpi_dir.join("platform_profile_choices"),
        "quiet balanced performance\n",
    )
    .unwrap();
    fs::write(debugfs_dir.join("devs"), "").unwrap();

    let root = SysfsRoot::new(dir.path().join("sys"));
    let driver = AsusHybridThermalDriver::new(
        root,
        "/firmware/acpi/platform_profile",
        Some("/kernel/debug/asus-nb-wmi/devs"),
        Some(0x00110013),
        None::<&str>,
        true,
    );

    assert!(driver.capabilities().contains(ThermalCapabilities::FULL_SPEED_FAN));

    // Activate Full Speed
    driver.set_profile(ThermalProfileMode::FullSpeed).await.unwrap();
    assert_eq!(
        driver.get_current_profile().await.unwrap(),
        ThermalProfileMode::FullSpeed
    );

    let devs_content = fs::read_to_string(debugfs_dir.join("devs")).unwrap();
    assert_eq!(devs_content.trim(), "0x00110013 0x1");

    // Revert to Quiet
    driver.set_profile(ThermalProfileMode::Quiet).await.unwrap();
    assert_eq!(
        driver.get_current_profile().await.unwrap(),
        ThermalProfileMode::Quiet
    );

    let devs_content_reverted = fs::read_to_string(debugfs_dir.join("devs")).unwrap();
    assert_eq!(devs_content_reverted.trim(), "0x00110013 0x0");
}

#[tokio::test]
async fn test_thermal_hwmon_fan_readback() {
    let dir = tempdir().unwrap();
    let acpi_dir = dir.path().join("sys/firmware/acpi");
    let hwmon_dir = dir.path().join("sys/devices/platform/asus-nb-wmi/hwmon/hwmon1");
    fs::create_dir_all(&acpi_dir).unwrap();
    fs::create_dir_all(&hwmon_dir).unwrap();

    fs::write(acpi_dir.join("platform_profile"), "balanced\n").unwrap();
    fs::write(hwmon_dir.join("fan1_input"), "4200\n").unwrap();
    fs::write(hwmon_dir.join("fan1_label"), "cpu_fan\n").unwrap();
    fs::write(hwmon_dir.join("fan2_input"), "4100\n").unwrap();
    fs::write(hwmon_dir.join("fan2_label"), "gpu_fan\n").unwrap();

    let root = SysfsRoot::new(dir.path().join("sys"));
    let driver = AsusHybridThermalDriver::new(
        root,
        "/firmware/acpi/platform_profile",
        None::<&str>,
        None,
        Some(hwmon_dir.to_str().unwrap()),
        false,
    );

    let fans = driver.get_fans().await.unwrap();
    assert_eq!(fans.len(), 2);
    assert_eq!(fans[0].label, "cpu_fan");
    assert_eq!(fans[0].current_rpm, 4200);
    assert_eq!(fans[1].label, "gpu_fan");
    assert_eq!(fans[1].current_rpm, 4100);
}
