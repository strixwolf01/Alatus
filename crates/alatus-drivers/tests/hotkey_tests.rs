use alatus_core::hotkey::HotkeyAction;
use alatus_drivers::{AsusWmiHotkeyDriver, SysfsRoot};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_hotkey_node_discovery() {
    let dir = tempdir().unwrap();
    let event3_dir = dir.path().join("sys/class/input/event3/device");
    let event4_dir = dir.path().join("sys/class/input/event4/device");
    fs::create_dir_all(&event3_dir).unwrap();
    fs::create_dir_all(&event4_dir).unwrap();

    fs::write(event3_dir.join("name"), "Asus WMI hotkeys\n").unwrap();
    fs::write(event4_dir.join("name"), "Power Button\n").unwrap();

    let root = SysfsRoot::new(dir.path().join("sys"));
    let driver = AsusWmiHotkeyDriver::new(root, "Asus WMI hotkeys", None::<&str>);

    let discovered = driver
        .discover_event_node()
        .expect("Should discover event node");
    assert!(discovered.to_str().unwrap().ends_with("/dev/input/event3"));
}

#[test]
fn test_hotkey_action_mapping() {
    // 202 is KEY_PROG3 (Fn+F Fan toggle)
    assert_eq!(
        AsusWmiHotkeyDriver::map_key(202),
        HotkeyAction::FanModeToggle
    );

    // 187 is KEY_F17 (Alternate Fan toggle)
    assert_eq!(
        AsusWmiHotkeyDriver::map_key(187),
        HotkeyAction::FanModeToggle
    );

    // 248 is KEY_MICMUTE
    assert_eq!(
        AsusWmiHotkeyDriver::map_key(248),
        HotkeyAction::MicMuteToggle
    );

    // 530 is KEY_TOUCHPAD_TOGGLE
    assert_eq!(
        AsusWmiHotkeyDriver::map_key(530),
        HotkeyAction::TouchpadToggle
    );

    // 228 is KEY_KBDILLUMTOGGLE
    assert_eq!(
        AsusWmiHotkeyDriver::map_key(228),
        HotkeyAction::AuraModeToggle
    );
}
