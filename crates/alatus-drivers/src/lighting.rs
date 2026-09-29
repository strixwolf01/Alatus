//! ASUS ITE5570 HID LampArray keyboard lighting driver.

use crate::sysfs::SysfsRoot;
use alatus_core::{
    capabilities::LightingCapabilities,
    error::AlatusError,
    lighting::{LightingDriver, LightingEffect, LightingMode, RgbColor},
};
use async_trait::async_trait;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;
use tokio::fs;

// ioctl definition for HIDIOCSFEATURE(17)
// #define HIDIOCSFEATURE(len) _IOC(_IOC_WRITE|_IOC_READ, 'H', 0x06, len)
nix::ioctl_readwrite_buf!(hidiocsfeature, b'H', 0x06, u8);

#[derive(Debug)]
pub struct AsusIte5570LightingDriver {
    sysfs_root: SysfsRoot,
    hid_vendor_id: u16,
    hid_product_id: u16,
    report_id: u8,
    custom_device_path: Option<PathBuf>,
    current_brightness: AtomicU8,
    current_mode: Mutex<LightingMode>,
    current_color: Mutex<RgbColor>,
}

impl AsusIte5570LightingDriver {
    pub fn new(
        sysfs_root: SysfsRoot,
        hid_vendor_id: u16,
        hid_product_id: u16,
        report_id: u8,
        custom_device_path: Option<impl AsRef<Path>>,
        default_brightness: u8,
    ) -> Self {
        Self {
            sysfs_root,
            hid_vendor_id,
            hid_product_id,
            report_id: if report_id == 0 { 0x5A } else { report_id },
            custom_device_path: custom_device_path.map(|p| p.as_ref().to_path_buf()),
            current_brightness: AtomicU8::new(default_brightness.min(3)),
            current_mode: Mutex::new(LightingMode::Static),
            current_color: Mutex::new(RgbColor::new(255, 255, 255)),
        }
    }

    pub fn discover_hidraw_node(&self) -> Option<PathBuf> {
        if let Some(ref path) = self.custom_device_path {
            return Some(path.clone());
        }

        let hidraw_class_dir = self.sysfs_root.resolve("/sys/class/hidraw");
        if !hidraw_class_dir.exists() {
            // Also try relative class/hidraw if sysfs root is isolated
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

    pub fn build_report(
        &self,
        mode: LightingMode,
        color: RgbColor,
        speed: u8,
        brightness: u8,
    ) -> [u8; 17] {
        let mut buf = [0u8; 17];
        buf[0] = self.report_id; // 0x5A
        buf[1] = 0xBA; // Keyboard lighting command sub-id

        buf[2] = match mode {
            LightingMode::Static => 0x00,
            LightingMode::Breathing => 0x01,
            LightingMode::Strobe => 0x02,
            LightingMode::Rainbow => 0x03,
            LightingMode::Off => 0x00,
        };

        buf[3] = color.r;
        buf[4] = color.g;
        buf[5] = color.b;
        buf[6] = speed;
        buf[7] = if mode == LightingMode::Off {
            0
        } else {
            brightness.min(3)
        };

        buf
    }

    async fn send_report(&self, buf: &mut [u8; 17]) -> Result<(), AlatusError> {
        let path = self.discover_hidraw_node().ok_or_else(|| {
            AlatusError::DeviceNotFound(format!(
                "ASUS ITE5570 LampArray device ({:04X}:{:04X}) not found in sysfs/hidraw",
                self.hid_vendor_id, self.hid_product_id
            ))
        })?;

        // Try sending via ioctl if it's a character device; fallback to file write for tests
        let file = File::options()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| AlatusError::Io {
                path: path.clone(),
                source: e,
            })?;

        let res = unsafe { hidiocsfeature(file.as_raw_fd(), buf) };
        if let Err(err) = res {
            // If ioctl is not supported on this fd (e.g. regular test file), fallback to writing bytes directly
            if err == nix::errno::Errno::ENOTTY || err == nix::errno::Errno::EINVAL {
                fs::write(&path, &buf[..])
                    .await
                    .map_err(|e| AlatusError::Io {
                        path: path.clone(),
                        source: e,
                    })?;
            } else {
                return Err(AlatusError::Driver {
                    driver: "asus-ite5570-lamparray",
                    message: format!("ioctl HIDIOCSFEATURE failed on {}: {err}", path.display()),
                });
            }
        }

        Ok(())
    }
}

#[async_trait]
impl LightingDriver for AsusIte5570LightingDriver {
    fn name(&self) -> &'static str {
        "asus-ite5570-lamparray"
    }

    fn capabilities(&self) -> LightingCapabilities {
        LightingCapabilities::BRIGHTNESS_CONTROL
            | LightingCapabilities::STATIC_COLOR
            | LightingCapabilities::BUILTIN_EFFECTS
    }

    async fn set_brightness(&self, level: u8) -> Result<(), AlatusError> {
        let level = level.min(3);
        let mode = *self.current_mode.lock().unwrap();
        let color = *self.current_color.lock().unwrap();

        let mut report = self.build_report(mode, color, 1, level);
        self.send_report(&mut report).await?;

        self.current_brightness.store(level, Ordering::SeqCst);
        tracing::info!("Set keyboard backlight brightness to {level}");
        Ok(())
    }

    async fn get_brightness(&self) -> Result<u8, AlatusError> {
        Ok(self.current_brightness.load(Ordering::SeqCst))
    }

    async fn apply_effect(&self, effect: &LightingEffect) -> Result<(), AlatusError> {
        let brightness = effect.brightness.min(3);
        let mut report =
            self.build_report(effect.mode, effect.primary_color, effect.speed, brightness);
        self.send_report(&mut report).await?;

        *self.current_mode.lock().unwrap() = effect.mode;
        *self.current_color.lock().unwrap() = effect.primary_color;
        self.current_brightness.store(brightness, Ordering::SeqCst);

        tracing::info!(
            "Applied lighting effect: mode={:?}, color=({:?}), brightness={}",
            effect.mode,
            effect.primary_color,
            brightness
        );
        Ok(())
    }
}
