//! ASUS ITE5570 HID LampArray keyboard lighting driver.
//! Reverse engineered protocol verified from vrgb (vrgb-dev/vrgb).

use crate::sysfs::SysfsRoot;
use alatus_core::{
    capabilities::LightingCapabilities,
    error::AlatusError,
    lighting::{LightingDriver, LightingEffect, LightingMode, RgbColor},
};
use async_trait::async_trait;
use std::fs::File;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;

// ioctl definition for HIDIOCSFEATURE(len)
// #define HIDIOCSFEATURE(len) _IOC(_IOC_WRITE|_IOC_READ, 'H', 0x06, len)
nix::ioctl_readwrite_buf!(hidiocsfeature, b'H', 0x06, u8);

const WMI_BASE_PATH: &str = "/sys/kernel/debug/asus-nb-wmi";

#[derive(Debug)]
pub struct AsusIte5570LightingDriver {
    sysfs_root: SysfsRoot,
    hid_vendor_id: u16,
    hid_product_id: u16,
    firmware_report_id: u8,
    color_report_id: u8,
    rainbow_supported: bool,
    custom_device_path: Option<PathBuf>,
    current_brightness: AtomicU8,
    current_mode: Mutex<LightingMode>,
    current_color: Mutex<RgbColor>,
    supported_modes: Vec<LightingMode>,
}

impl AsusIte5570LightingDriver {
    pub fn new(
        sysfs_root: SysfsRoot,
        hid_vendor_id: u16,
        hid_product_id: u16,
        report_id: u8,
        custom_device_path: Option<impl AsRef<Path>>,
        default_brightness: u8,
        supported_modes: Option<Vec<String>>,
    ) -> Self {
        // Hardware report IDs based on ITE5570 device models (confirmed by vrgb):
        // - 0x19B6 (Vivobook S 14/15 series): firmware=0x0B, color=0x05, rainbow=true
        // - 0x5570 (Vivobook S 16 series): firmware=0x46, color=0x45, rainbow=false
        let (firmware_report_id, color_report_id, rainbow_supported) = match hid_product_id {
            0x5570 => (0x46, 0x45, false),
            0x19B6 => (0x0B, 0x05, true),
            _ => {
                if report_id != 0 && report_id != 0x5A {
                    (report_id, report_id.saturating_sub(6), true)
                } else {
                    (0x0B, 0x05, true)
                }
            }
        };

        let parsed_modes = match supported_modes {
            Some(modes) if !modes.is_empty() => {
                let parsed: Vec<LightingMode> = modes
                    .into_iter()
                    .filter_map(|m| m.parse::<LightingMode>().ok())
                    .collect();
                if parsed.is_empty() {
                    vec![LightingMode::Static]
                } else {
                    parsed
                }
            }
            _ => vec![LightingMode::Static],
        };

        Self {
            sysfs_root,
            hid_vendor_id,
            hid_product_id,
            firmware_report_id,
            color_report_id,
            rainbow_supported,
            custom_device_path: custom_device_path.map(|p| p.as_ref().to_path_buf()),
            current_brightness: AtomicU8::new(default_brightness.min(3)),
            current_mode: Mutex::new(LightingMode::Static),
            current_color: Mutex::new(RgbColor::new(255, 255, 255)),
            supported_modes: parsed_modes,
        }
    }

    pub fn discover_hidraw_node(&self) -> Option<PathBuf> {
        if let Some(ref path) = self.custom_device_path {
            return Some(path.clone());
        }

        let hidraw_class_dir = self.sysfs_root.resolve("/sys/class/hidraw");
        if !hidraw_class_dir.exists() {
            let rel = self.sysfs_root.resolve("/class/hidraw");
            if rel.exists() {
                return self.scan_hidraw_dir(&rel);
            }
            return None;
        }

        self.scan_hidraw_dir(&hidraw_class_dir)
    }

    fn scan_hidraw_dir(&self, dir: &Path) -> Option<PathBuf> {
        let entries = std::fs::read_dir(dir).ok()?;
        let target_pattern =
            format!("{:04x}:{:04x}", self.hid_vendor_id, self.hid_product_id).to_lowercase();

        for entry in entries.flatten() {
            let path = entry.path();
            let uevent_file = path.join("device/uevent");
            if let Ok(uevent) = std::fs::read_to_string(uevent_file) {
                if uevent.to_lowercase().contains(&target_pattern) {
                    if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
                        let dev_path = PathBuf::from(format!("/dev/{file_name}"));
                        return Some(self.sysfs_root.resolve(dev_path));
                    }
                }
            }
        }

        None
    }

    pub fn build_color_report(&self, r: u8, g: u8, b: u8, intensity: u8) -> [u8; 10] {
        [
            self.color_report_id,
            0x01,
            0x00,
            0x00,
            0x00,
            0x00,
            r,
            g,
            b,
            intensity,
        ]
    }

    pub fn build_mode_report(&self, autonomous: bool) -> [u8; 2] {
        [
            self.firmware_report_id,
            if autonomous { 0x01 } else { 0x00 },
        ]
    }

    fn send_mode_packet(
        &self,
        dev_file: &mut Option<File>,
        autonomous: bool,
    ) -> Result<(), AlatusError> {
        let mut buf = self.build_mode_report(autonomous);
        if let Some(ref mut f) = dev_file {
            let fd = f.as_raw_fd();
            unsafe {
                if let Err(e) = hidiocsfeature(fd, &mut buf) {
                    tracing::debug!(
                        "HIDIOCSFEATURE(2) mode ioctl not supported (file fallback): {e}"
                    );
                    let _ = f.write_all(&buf);
                }
            }
        }
        Ok(())
    }

    fn send_color_packet(
        &self,
        dev_file: &mut Option<File>,
        r: u8,
        g: u8,
        b: u8,
        intensity: u8,
    ) -> Result<(), AlatusError> {
        let mut buf = self.build_color_report(r, g, b, intensity);
        if let Some(ref mut f) = dev_file {
            let fd = f.as_raw_fd();
            unsafe {
                if let Err(e) = hidiocsfeature(fd, &mut buf) {
                    tracing::debug!(
                        "HIDIOCSFEATURE(10) color ioctl not supported (file fallback): {e}"
                    );
                    let _ = f.write_all(&buf);
                }
            }
        }
        Ok(())
    }

    fn set_wmi_rainbow(&self, enable: bool) {
        let wmi_base = self.sysfs_root.resolve(WMI_BASE_PATH);
        if !wmi_base.exists() {
            return;
        }

        let method_id = wmi_base.join("method_id");
        let dev_id = wmi_base.join("dev_id");
        let ctrl_param = wmi_base.join("ctrl_param");
        let devs = wmi_base.join("devs");

        if method_id.exists() && dev_id.exists() && ctrl_param.exists() && devs.exists() {
            let _ = std::fs::write(&method_id, "0x00000001\n");
            let _ = std::fs::write(&dev_id, "0x0005002f\n");
            let param = if enable {
                "0x00000000\n"
            } else {
                "0x00000001\n"
            };
            let _ = std::fs::write(&ctrl_param, param);
            let _ = std::fs::read_to_string(&devs);
        }
    }
}

#[async_trait]
impl LightingDriver for AsusIte5570LightingDriver {
    fn name(&self) -> &'static str {
        "asus-ite5570-lamparray"
    }

    fn capabilities(&self) -> LightingCapabilities {
        let mut caps =
            LightingCapabilities::BRIGHTNESS_CONTROL | LightingCapabilities::STATIC_COLOR;
        if self.rainbow_supported && self.supported_modes.contains(&LightingMode::Rainbow) {
            caps |= LightingCapabilities::BUILTIN_EFFECTS;
        }
        caps
    }

    fn supported_modes(&self) -> Vec<LightingMode> {
        self.supported_modes.clone()
    }

    async fn get_brightness(&self) -> Result<u8, AlatusError> {
        Ok(self.current_brightness.load(Ordering::SeqCst))
    }

    async fn set_brightness(&self, level: u8) -> Result<(), AlatusError> {
        let level = level.min(3);
        let current_mode = *self.current_mode.lock().unwrap();
        let current_color = *self.current_color.lock().unwrap();

        self.apply_effect(&LightingEffect {
            mode: current_mode,
            primary_color: current_color,
            secondary_color: None,
            speed: 1,
            brightness: level,
        })
        .await?;

        Ok(())
    }

    async fn apply_effect(&self, effect: &LightingEffect) -> Result<(), AlatusError> {
        if effect.mode != LightingMode::Off && !self.supported_modes.contains(&effect.mode) {
            return Err(AlatusError::UnsupportedCapability(
                "Requested lighting mode is not supported by this hardware profile",
            ));
        }

        let dev_path = match self.discover_hidraw_node() {
            Some(p) => p,
            None => {
                return Err(AlatusError::DeviceNotFound(
                    "ASUS ITE5570 LampArray keyboard not found in sysfs/hidraw".into(),
                ))
            }
        };

        let mut file_opt = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&dev_path)
            .ok();

        if file_opt.is_none() {
            return Err(AlatusError::PermissionDenied(format!(
                "Cannot open hidraw device '{}' for read/write",
                dev_path.display()
            )));
        }

        match effect.mode {
            LightingMode::Rainbow => {
                self.send_mode_packet(&mut file_opt, true)?;
                if self.rainbow_supported {
                    self.set_wmi_rainbow(true);
                }
            }
            LightingMode::Off => {
                self.send_mode_packet(&mut file_opt, false)?;
                if self.rainbow_supported {
                    self.set_wmi_rainbow(false);
                }
                self.send_color_packet(&mut file_opt, 0, 0, 0, 0)?;
            }
            LightingMode::Static | LightingMode::Breathing | LightingMode::Strobe => {
                self.send_mode_packet(&mut file_opt, false)?;
                if self.rainbow_supported {
                    self.set_wmi_rainbow(false);
                }
                let intensity = match effect.brightness {
                    0 => 0,
                    1 => 85,
                    2 => 170,
                    _ => 255,
                };
                self.send_color_packet(
                    &mut file_opt,
                    effect.primary_color.r,
                    effect.primary_color.g,
                    effect.primary_color.b,
                    intensity,
                )?;
            }
        }

        *self.current_mode.lock().unwrap() = effect.mode;
        *self.current_color.lock().unwrap() = effect.primary_color;
        self.current_brightness
            .store(effect.brightness, Ordering::SeqCst);

        tracing::info!(
            "Applied lighting effect: mode={:?}, brightness={}, color={:?}",
            effect.mode,
            effect.brightness,
            effect.primary_color
        );
        Ok(())
    }
}
