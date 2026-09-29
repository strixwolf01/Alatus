use alatus_drivers::{AsusIte5570LightingDriver, SysfsRoot};
use alatus_ipc::{LightingProxy, LIGHTING_OBJECT_PATH};
use alatusd::LightingService;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;
use zbus::connection::Builder;
use zbus::Connection;

#[tokio::test]
async fn test_lighting_service_and_proxy_over_session_dbus() {
    let dir = tempdir().unwrap();
    let dev_dir = dir.path().join("sys/dev");
    let hidraw_dir = dir.path().join("sys/class/hidraw/hidraw1/device");
    fs::create_dir_all(&dev_dir).unwrap();
    fs::create_dir_all(&hidraw_dir).unwrap();

    let mock_dev_file = dev_dir.join("hidraw1");
    fs::write(&mock_dev_file, vec![0u8; 17]).unwrap();

    fs::write(
        hidraw_dir.join("uevent"),
        "DRIVER=hid-generic\nHID_ID=0018:00000B05:000019B6\nHID_NAME=ITE5570:00 0B05:19B6\n",
    )
    .unwrap();

    let root = SysfsRoot::new(dir.path().join("sys"));
    let driver = Arc::new(AsusIte5570LightingDriver::new(
        root,
        0x0B05,
        0x19B6,
        0x5A,
        Some(&mock_dev_file),
        3,
    ));

    let service = LightingService::new(driver);
    let test_bus_name = "org.alatus.TestDaemonLighting";

    let server_conn = match Builder::session() {
        Ok(b) => match b.name(test_bus_name) {
            Ok(b) => match b.serve_at(LIGHTING_OBJECT_PATH, service) {
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

    let proxy = LightingProxy::builder(&client_conn)
        .destination(test_bus_name)
        .unwrap()
        .path(LIGHTING_OBJECT_PATH)
        .unwrap()
        .build()
        .await
        .unwrap();

    // 1. Initial brightness
    let brightness = proxy.get_brightness().await.expect("get_brightness");
    assert_eq!(brightness, 3);

    // 2. Set brightness to 2
    proxy.set_brightness(2).await.expect("set_brightness");
    assert_eq!(proxy.get_brightness().await.unwrap(), 2);

    // 3. Set color to Green (0, 255, 0)
    proxy.set_color(0, 255, 0).await.expect("set_color");

    // 4. Set mode to Breathing
    proxy.set_mode("Breathing".into(), 2).await.expect("set_mode");

    // 5. Inspect state
    let state = proxy.get_state().await.expect("get_state");
    assert_eq!(state.mode, "Breathing");
    assert_eq!(state.brightness, 2);
    assert_eq!(state.r, 0);
    assert_eq!(state.g, 255);
    assert_eq!(state.b, 0);
    assert_eq!(state.speed, 2);

    // 6. Verify packet written to mock device file
    let packet = fs::read(&mock_dev_file).unwrap();
    assert_eq!(packet[0], 0x5A); // Report ID
    assert_eq!(packet[1], 0xBA); // Command Sub-ID
    assert_eq!(packet[2], 0x01); // Breathing = 1
    assert_eq!(packet[3], 0);    // Red
    assert_eq!(packet[4], 255);  // Green
    assert_eq!(packet[5], 0);    // Blue
    assert_eq!(packet[6], 2);    // Speed
    assert_eq!(packet[7], 2);    // Brightness

    drop(server_conn);
}
