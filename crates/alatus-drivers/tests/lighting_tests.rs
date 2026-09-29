use alatus_core::capabilities::LightingCapabilities;
use alatus_core::lighting::{LightingDriver, LightingEffect, LightingMode, RgbColor};
use alatus_drivers::{AsusIte5570LightingDriver, SysfsRoot};
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn test_lighting_device_discovery_and_packet_serialization() {
    let dir = tempdir().unwrap();
    let hidraw_dir = dir.path().join("sys/class/hidraw/hidraw1/device");
    let dev_dir = dir.path().join("sys/dev");
    fs::create_dir_all(&hidraw_dir).unwrap();
    fs::create_dir_all(&dev_dir).unwrap();

    // Create mock uevent matching ITE5570 VID 0x0B05 and PID 0x19B6
    fs::write(
        hidraw_dir.join("uevent"),
        "DRIVER=hid-generic\nHID_ID=0018:00000B05:000019B6\nHID_NAME=ITE5570:00 0B05:19B6\n",
    )
    .unwrap();

    // Create mock /dev/hidraw1 file
    let mock_dev_file = dev_dir.join("hidraw1");
    fs::write(&mock_dev_file, vec![0u8; 17]).unwrap();

    let root = SysfsRoot::new(dir.path().join("sys"));
    let driver = AsusIte5570LightingDriver::new(
        root,
        0x0B05,
        0x19B6,
        0x5A,
        Some(&mock_dev_file),
        3,
    );

    let caps = driver.capabilities();
    assert!(caps.contains(LightingCapabilities::BRIGHTNESS_CONTROL));
    assert!(caps.contains(LightingCapabilities::STATIC_COLOR));
    assert!(caps.contains(LightingCapabilities::BUILTIN_EFFECTS));

    assert_eq!(driver.get_brightness().await.unwrap(), 3);

    // Apply Static Red Effect
    let red_effect = LightingEffect {
        mode: LightingMode::Static,
        primary_color: RgbColor::new(255, 0, 0),
        secondary_color: None,
        speed: 1,
        brightness: 2,
    };
    driver.apply_effect(&red_effect).await.unwrap();

    assert_eq!(driver.get_brightness().await.unwrap(), 2);

    // Inspect the 17-byte packet written to the mock device file
    let packet = fs::read(&mock_dev_file).unwrap();
    assert_eq!(packet.len(), 17);
    assert_eq!(packet[0], 0x5A); // Report ID
    assert_eq!(packet[1], 0xBA); // Command Sub-ID
    assert_eq!(packet[2], 0x00); // Mode Static = 0
    assert_eq!(packet[3], 255);  // Red
    assert_eq!(packet[4], 0);    // Green
    assert_eq!(packet[5], 0);    // Blue
    assert_eq!(packet[6], 1);    // Speed
    assert_eq!(packet[7], 2);    // Brightness

    // Apply Rainbow Effect with brightness 3
    let rainbow_effect = LightingEffect {
        mode: LightingMode::Rainbow,
        primary_color: RgbColor::new(0, 0, 0),
        secondary_color: None,
        speed: 2,
        brightness: 3,
    };
    driver.apply_effect(&rainbow_effect).await.unwrap();

    let packet = fs::read(&mock_dev_file).unwrap();
    assert_eq!(packet[0], 0x5A);
    assert_eq!(packet[1], 0xBA);
    assert_eq!(packet[2], 0x03); // Rainbow = 3
    assert_eq!(packet[6], 2);    // Speed
    assert_eq!(packet[7], 3);    // Brightness = 3
}
