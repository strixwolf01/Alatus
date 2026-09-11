// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Internal RGB lighting service for ASUS keyboard backlight control.
//!
//! Provides minimal, rock-solid static color control via the LampArray HID interface
//! and physical brightness management via the Linux sysfs LED subsystem.

use crate::services::alatus_rgb_wrapper::{self as ascend_rgb_wrapper, RgbDevice};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

pub const DEBUGFS_BASE: &str = "/sys/kernel/debug/asus-nb-wmi";
pub const ASUS_WMI_DEVID_KBD_RGB: u32 = 0x0005002F;
pub const SYS_KBD_BACKLIGHT: &str = "/sys/class/leds/asus::kbd_backlight";

/// Snapshot of current RGB status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RgbStatus {
    pub available: bool,
    pub model: String,
    pub path: String,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub brightness: u32,
}

/// Performs ASUS WMI hardware unlock for the keyboard controller.
///
/// Writing `0x0005002F` to `dev_id`, `1` to `ctrl_param`, and reading `devs`
/// marks OOBE as complete and unlocks the controller for host writes.
pub fn wmi_unlock() -> Result<(), String> {
    let base = Path::new(DEBUGFS_BASE);
    if !base.exists() {
        return Ok(());
    }

    let method_id_file = base.join("method_id");
    let dev_id_file = base.join("dev_id");
    let ctrl_param_file = base.join("ctrl_param");
    let devs_file = base.join("devs");

    if !dev_id_file.exists() || !ctrl_param_file.exists() || !devs_file.exists() {
        return Ok(());
    }

    if method_id_file.exists() {
        let _ = std::fs::write(&method_id_file, "0x00000001\n");
    }

    std::fs::write(&dev_id_file, format!("0x{ASUS_WMI_DEVID_KBD_RGB:08x}\n"))
        .map_err(|e| format!("WMI write dev_id failed: {e}"))?;
    std::fs::write(&ctrl_param_file, "0x00000001\n")
        .map_err(|e| format!("WMI write ctrl_param failed: {e}"))?;
    let _ =
        std::fs::read_to_string(&devs_file).map_err(|e| format!("WMI read devs failed: {e}"))?;

    tracing::info!("ASUS WMI keyboard controller unlocked");
    Ok(())
}

/// Reads current and maximum brightness from `/sys/class/leds/asus::kbd_backlight`.
pub fn read_sysfs_brightness() -> Option<(u32, u32)> {
    let base = Path::new(SYS_KBD_BACKLIGHT);
    if !base.exists() {
        return None;
    }
    let cur_str = std::fs::read_to_string(base.join("brightness")).ok()?;
    let max_str = std::fs::read_to_string(base.join("max_brightness")).ok()?;
    let cur = cur_str.trim().parse::<u32>().ok()?;
    let max = max_str.trim().parse::<u32>().ok()?;
    Some((cur, max))
}

/// Writes discrete brightness value to `/sys/class/leds/asus::kbd_backlight/brightness`.
pub fn write_sysfs_brightness(val: u32) -> Result<(), String> {
    let path = Path::new(SYS_KBD_BACKLIGHT).join("brightness");
    if !path.exists() {
        return Ok(());
    }
    std::fs::write(&path, format!("{val}\n"))
        .map_err(|e| format!("Failed to write brightness to {}: {e}", path.display()))
}

/// Converts a discrete sysfs brightness value to an approximate percentage (0..=100).
pub fn sysfs_to_percent(cur: u32, max: u32) -> u32 {
    if max == 0 {
        return 0;
    }
    (((cur as u64 * 100) + (max as u64 / 2)) / max as u64).min(100) as u32
}

/// Converts either a discrete value (0..=max) or a percentage (0..=100) to discrete sysfs brightness.
pub fn percent_to_sysfs(val: u32, max: u32) -> u32 {
    if max == 0 {
        return 0;
    }
    if val <= max {
        val
    } else {
        let p = val.min(100);
        (((p as u64 * max as u64) + 50) / 100).min(max as u64) as u32
    }
}

/// RGB service managing controller interaction and state persistence.
pub struct RgbService {
    state: Mutex<RgbServiceState>,
}

struct RgbServiceState {
    device: Option<RgbDevice>,
    red: u8,
    green: u8,
    blue: u8,
    brightness: u32,
}

impl Default for RgbService {
    fn default() -> Self {
        Self::new()
    }
}

impl RgbService {
    pub fn new() -> Self {
        let dev = ascend_rgb_wrapper::discover().ok();
        let _ = wmi_unlock();

        let initial_brightness = if let Some((cur, max)) = read_sysfs_brightness() {
            sysfs_to_percent(cur, max)
        } else {
            100
        };

        Self {
            state: Mutex::new(RgbServiceState {
                device: dev,
                red: 255,
                green: 255,
                blue: 255,
                brightness: initial_brightness,
            }),
        }
    }

    fn ensure_device(state: &mut RgbServiceState) -> Result<RgbDevice, String> {
        if let Some(ref dev) = state.device
            && Path::new(&dev.path).exists()
        {
            return Ok(dev.clone());
        }
        let dev = ascend_rgb_wrapper::discover()?;
        state.device = Some(dev.clone());
        Ok(dev)
    }

    pub fn get_status(&self) -> RgbStatus {
        let mut state = self.state.lock().unwrap();
        let dev = Self::ensure_device(&mut state).ok();

        if let Some((cur, max)) = read_sysfs_brightness() {
            state.brightness = sysfs_to_percent(cur, max);
        }

        RgbStatus {
            available: dev.is_some(),
            model: dev
                .as_ref()
                .map(|d| d.model.to_string())
                .unwrap_or_else(|| "None detected".to_string()),
            path: dev.as_ref().map(|d| d.path.clone()).unwrap_or_default(),
            red: state.red,
            green: state.green,
            blue: state.blue,
            brightness: state.brightness,
        }
    }

    pub fn get_color(&self) -> (u8, u8, u8) {
        let state = self.state.lock().unwrap();
        (state.red, state.green, state.blue)
    }

    pub fn set_color(&self, r: u8, g: u8, b: u8) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        let dev = Self::ensure_device(&mut state)?;
        state.red = r;
        state.green = g;
        state.blue = b;

        let _ = wmi_unlock();
        ascend_rgb_wrapper::set_firmware_mode(&dev, false)?;
        ascend_rgb_wrapper::set_color(&dev, r, g, b, 255)?;

        if let Some((cur, max)) = read_sysfs_brightness()
            && cur == 0
        {
            let target = if state.brightness == 0 {
                100
            } else {
                state.brightness
            };
            let sysfs_val = percent_to_sysfs(target, max);
            let _ = write_sysfs_brightness(sysfs_val);
            state.brightness = sysfs_to_percent(sysfs_val, max);
        }

        Ok(())
    }

    pub fn get_brightness(&self) -> u32 {
        if let Some((cur, max)) = read_sysfs_brightness() {
            sysfs_to_percent(cur, max)
        } else {
            self.state.lock().unwrap().brightness
        }
    }

    pub fn set_brightness(&self, brightness: u32) -> Result<(), String> {
        if brightness > 100 {
            return Err("Brightness must be between 0 and 100".to_string());
        }
        let mut state = self.state.lock().unwrap();
        let dev = Self::ensure_device(&mut state)?;

        if let Some((_, max)) = read_sysfs_brightness() {
            let sysfs_val = percent_to_sysfs(brightness, max);
            write_sysfs_brightness(sysfs_val)?;
            state.brightness = sysfs_to_percent(sysfs_val, max);
        } else {
            state.brightness = brightness;
        }

        if brightness == 0 {
            let _ = ascend_rgb_wrapper::off(&dev, state.red, state.green, state.blue);
        } else {
            let _ = wmi_unlock();
            let _ = ascend_rgb_wrapper::set_firmware_mode(&dev, false);
            let _ = ascend_rgb_wrapper::set_color(&dev, state.red, state.green, state.blue, 255);
        }

        Ok(())
    }

    pub fn turn_off(&self) -> Result<(), String> {
        self.set_brightness(0)
    }

    pub fn turn_on(&self) -> Result<(), String> {
        self.set_brightness(100)
    }

    pub fn reapply(&self) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        let dev = Self::ensure_device(&mut state)?;
        let _ = wmi_unlock();
        ascend_rgb_wrapper::set_firmware_mode(&dev, false)?;

        if state.brightness == 0 {
            let _ = ascend_rgb_wrapper::off(&dev, state.red, state.green, state.blue);
            let _ = write_sysfs_brightness(0);
        } else {
            ascend_rgb_wrapper::set_color(&dev, state.red, state.green, state.blue, 255)?;
            if let Some((_, max)) = read_sysfs_brightness() {
                let sysfs_val = percent_to_sysfs(state.brightness, max);
                let _ = write_sysfs_brightness(sysfs_val);
            }
        }
        Ok(())
    }

    /// Clears the cached device descriptor, performs WMI unlock, and reapplies
    /// current RGB color and brightness settings. Useful after sleep/resume.
    pub fn reset_and_reapply(&self) -> Result<(), String> {
        let _ = wmi_unlock();
        {
            let mut state = self.state.lock().unwrap();
            state.device = None;
        }
        self.reapply()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb_status_serialization() {
        let status = RgbStatus {
            available: true,
            model: "ASUS Test".to_string(),
            path: "/dev/hidraw0".to_string(),
            red: 255,
            green: 128,
            blue: 0,
            brightness: 80,
        };
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: RgbStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, deserialized);
    }

    #[test]
    fn test_percent_to_sysfs_and_back() {
        assert_eq!(percent_to_sysfs(0, 3), 0);
        assert_eq!(percent_to_sysfs(1, 3), 1);
        assert_eq!(percent_to_sysfs(2, 3), 2);
        assert_eq!(percent_to_sysfs(3, 3), 3);
        assert_eq!(percent_to_sysfs(100, 3), 3);
        assert_eq!(percent_to_sysfs(50, 3), 2);

        assert_eq!(sysfs_to_percent(0, 3), 0);
        assert_eq!(sysfs_to_percent(1, 3), 33);
        assert_eq!(sysfs_to_percent(2, 3), 67);
        assert_eq!(sysfs_to_percent(3, 3), 100);
    }
}
