//! ASUS WMI evdev hotkey driver.

use crate::sysfs::SysfsRoot;
use alatus_core::{
    error::AlatusError,
    hotkey::{HotkeyAction, HotkeyDriver, HotkeyEvent},
};
use async_trait::async_trait;
use evdev::{Device, EventStream, EventType, Key};
use std::path::{Path, PathBuf};

pub struct AsusWmiHotkeyDriver {
    sysfs_root: SysfsRoot,
    device_name: String,
    custom_device_path: Option<PathBuf>,
    stream: Option<EventStream>,
}

impl std::fmt::Debug for AsusWmiHotkeyDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsusWmiHotkeyDriver")
            .field("device_name", &self.device_name)
            .field("custom_device_path", &self.custom_device_path)
            .finish()
    }
}

impl AsusWmiHotkeyDriver {
    pub fn new(
        sysfs_root: SysfsRoot,
        device_name: impl Into<String>,
        custom_device_path: Option<impl AsRef<Path>>,
    ) -> Self {
        Self {
            sysfs_root,
            device_name: device_name.into(),
            custom_device_path: custom_device_path.map(|p| p.as_ref().to_path_buf()),
            stream: None,
        }
    }

    pub fn discover_event_node(&self) -> Option<PathBuf> {
        if let Some(ref path) = self.custom_device_path {
            return Some(path.clone());
        }

        let asus_wmi_by_path = self.sysfs_root.resolve("/dev/input/by-path/platform-asus-nb-wmi-event");
        if asus_wmi_by_path.exists() {
            return Some(asus_wmi_by_path);
        }

        let input_class_dir = self.sysfs_root.resolve("/sys/class/input");
        if !input_class_dir.exists() {
            let rel = self.sysfs_root.resolve("/class/input");
            if rel.exists() {
                return self.scan_input_dir(&rel);
            }
            return None;
        }

        self.scan_input_dir(&input_class_dir)
    }

    fn scan_input_dir(&self, dir: &Path) -> Option<PathBuf> {
        let entries = std::fs::read_dir(dir).ok()?;

        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = match path.file_name().and_then(|f| f.to_str()) {
                Some(name) if name.starts_with("event") => name.to_string(),
                _ => continue,
            };

            // Check device/name or name
            let name_file = path.join("device/name");
            let alt_name_file = path.join("name");

            let name_content = std::fs::read_to_string(&name_file)
                .or_else(|_| std::fs::read_to_string(&alt_name_file))
                .ok();

            if let Some(content) = name_content {
                let lower = content.to_lowercase();
                if lower.contains(&self.device_name.to_lowercase())
                    || lower.contains("asus wmi hotkeys")
                    || lower.contains("asus-nb-wmi")
                {
                    let dev_path = PathBuf::from(format!("/dev/input/{file_name}"));
                    return Some(self.sysfs_root.resolve(dev_path));
                }
            }
        }

        None
    }

    pub fn map_key(code: u16) -> HotkeyAction {
        // Known ASUS WMI and OEM scancodes:
        // 148 = KEY_PROG1 (Fn+F on Zenbook/ROG variants)
        // 187 = KEY_F17 (Fn+F on standard Linux Asus WMI)
        // 190 = KEY_F20 (Fn+F alternate mapping)
        // 202 = KEY_PROG3 (Fn+F fan mode toggle)
        // 203 = KEY_PROG4 (Fn+F alternate toggle)
        // 482 = ASUS WMI hotkey event code (Fn+F on Vivobook S / Zenbook OLED)
        // 582 = ASUS notification / fan hotkey
        match code {
            148 | 187 | 190 | 202 | 203 | 482 | 582 => HotkeyAction::FanModeToggle,
            248 => HotkeyAction::MicMuteToggle,
            530 => HotkeyAction::TouchpadToggle,
            228 => HotkeyAction::AuraModeToggle,
            _ => match Key::new(code) {
                Key::KEY_PROG3 | Key::KEY_F17 => HotkeyAction::FanModeToggle,
                Key::KEY_MICMUTE => HotkeyAction::MicMuteToggle,
                Key::KEY_TOUCHPAD_TOGGLE | Key::KEY_F21 => HotkeyAction::TouchpadToggle,
                Key::KEY_KBDILLUMTOGGLE | Key::KEY_KBDILLUMUP => HotkeyAction::AuraModeToggle,
                Key::KEY_BRIGHTNESSUP => HotkeyAction::BrightnessUp,
                Key::KEY_BRIGHTNESSDOWN => HotkeyAction::BrightnessDown,
                Key::KEY_SLEEP => HotkeyAction::Sleep,
                _ => HotkeyAction::Custom(code as u32),
            },
        }
    }

    async fn ensure_stream(&mut self) -> Result<&mut EventStream, AlatusError> {
        if self.stream.is_none() {
            let path = self.discover_event_node().ok_or_else(|| {
                AlatusError::DeviceNotFound(format!(
                    "ASUS WMI input device '{}' not found in sysfs/input",
                    self.device_name
                ))
            })?;

            let device = Device::open(&path).map_err(|e| AlatusError::Io {
                path: path.clone(),
                source: e,
            })?;

            let async_stream = device.into_event_stream().map_err(|e| AlatusError::Io {
                path: path.clone(),
                source: e,
            })?;

            self.stream = Some(async_stream);
        }

        Ok(self.stream.as_mut().unwrap())
    }
}

#[async_trait]
impl HotkeyDriver for AsusWmiHotkeyDriver {
    fn name(&self) -> &'static str {
        "asus-wmi-evdev"
    }

    async fn next_event(&mut self) -> Result<HotkeyEvent, AlatusError> {
        let stream = self.ensure_stream().await?;

        loop {
            match stream.next_event().await {
                Ok(ev) => {
                    // Only trigger on key press (value == 1)
                    if ev.event_type() == EventType::KEY && ev.value() == 1 {
                        let code = ev.code();
                        let action = Self::map_key(code);
                        return Ok(HotkeyEvent {
                            action,
                            raw_code: code as u32,
                        });
                    }
                }
                Err(e) => {
                    self.stream = None;
                    return Err(AlatusError::Io {
                        path: PathBuf::from("/dev/input"),
                        source: e,
                    });
                }
            }
        }
    }
}
