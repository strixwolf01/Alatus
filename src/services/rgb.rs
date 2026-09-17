// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Internal RGB lighting service for ASUS keyboard backlight control.
//!
//! Provides minimal, rock-solid static color control via the LampArray HID interface
//! and physical brightness management via the Linux sysfs LED subsystem.

use crate::services::alatus_rgb_wrapper::{self, RgbDevice};
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
    target_brightness: u32,
    last_sysfs_brightness: u32,
    is_timed_out: bool,
}

impl Default for RgbService {
    fn default() -> Self {
        Self::new()
    }
}

impl RgbService {
    pub fn new() -> Self {
        let dev = alatus_rgb_wrapper::discover().ok();
        let _ = wmi_unlock();

        let (initial_brightness, initial_sysfs) = if let Some((cur, max)) = read_sysfs_brightness()
        {
            let p = sysfs_to_percent(cur, max);
            (if p > 0 { p } else { 80 }, cur)
        } else {
            (80, 0)
        };

        Self {
            state: Mutex::new(RgbServiceState {
                device: dev,
                red: 255,
                green: 255,
                blue: 255,
                target_brightness: initial_brightness,
                last_sysfs_brightness: initial_sysfs,
                is_timed_out: false,
            }),
        }
    }

    fn ensure_device(state: &mut RgbServiceState) -> Result<RgbDevice, String> {
        if let Some(ref dev) = state.device {
            if Path::new(&dev.path).exists()
                && std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&dev.path)
                    .is_ok()
            {
                return Ok(dev.clone());
            }
            state.device = None;
        }
        let dev = alatus_rgb_wrapper::discover()?;
        state.device = Some(dev.clone());
        Ok(dev)
    }

    pub fn get_status(&self) -> RgbStatus {
        let mut state = self.state.lock().unwrap();
        let dev = Self::ensure_device(&mut state).ok();

        // Only sync if user changed sysfs externally (e.g. Fn keys), not during timeout
        if !state.is_timed_out
            && let Some((cur, max)) = read_sysfs_brightness()
            && cur != state.last_sysfs_brightness
            && cur > 0
        {
            state.target_brightness = sysfs_to_percent(cur, max);
            state.last_sysfs_brightness = cur;
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
            brightness: state.target_brightness,
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
        if let Err(e) = alatus_rgb_wrapper::set_firmware_mode(&dev, false) {
            state.device = None;
            return Err(e);
        }

        if state.is_timed_out {
            let _ = alatus_rgb_wrapper::off(&dev, r, g, b);
            return Ok(());
        }

        let intensity = alatus_rgb_wrapper::percent_to_intensity(state.target_brightness);
        if let Err(e) = alatus_rgb_wrapper::set_color(&dev, r, g, b, intensity) {
            state.device = None;
            return Err(e);
        }

        if let Some((cur, max)) = read_sysfs_brightness()
            && cur == 0
        {
            let target = if state.target_brightness == 0 {
                80
            } else {
                state.target_brightness
            };
            let sysfs_val = percent_to_sysfs(target, max);
            let _ = write_sysfs_brightness(sysfs_val);
            state.target_brightness = sysfs_to_percent(sysfs_val, max);
        }

        Ok(())
    }

    pub fn get_brightness(&self) -> u32 {
        let mut state = self.state.lock().unwrap();
        // Only sync if user changed sysfs externally (e.g. Fn keys), not during timeout
        if !state.is_timed_out
            && let Some((cur, max)) = read_sysfs_brightness()
            && cur != state.last_sysfs_brightness
            && cur > 0
        {
            state.target_brightness = sysfs_to_percent(cur, max);
            state.last_sysfs_brightness = cur;
        }
        state.target_brightness
    }

    pub fn set_brightness(&self, brightness: u32) -> Result<(), String> {
        if brightness > 100 {
            return Err("Brightness must be between 0 and 100".to_string());
        }
        let mut state = self.state.lock().unwrap();
        state.is_timed_out = false;
        state.target_brightness = brightness;
        let dev = Self::ensure_device(&mut state)?;

        if let Some((_, max)) = read_sysfs_brightness() {
            let sysfs_val = percent_to_sysfs(brightness, max);
            write_sysfs_brightness(sysfs_val)?;
            state.last_sysfs_brightness = sysfs_val;
        }

        if brightness == 0 {
            let _ = alatus_rgb_wrapper::off(&dev, state.red, state.green, state.blue);
        } else {
            let _ = wmi_unlock();
            let _ = alatus_rgb_wrapper::set_firmware_mode(&dev, false);
            let intensity = alatus_rgb_wrapper::percent_to_intensity(brightness);
            let _ =
                alatus_rgb_wrapper::set_color(&dev, state.red, state.green, state.blue, intensity);
        }

        Ok(())
    }

    pub fn turn_off(&self) -> Result<(), String> {
        self.set_brightness(0)
    }

    pub fn turn_on(&self) -> Result<(), String> {
        self.set_brightness(100)
    }

    pub fn is_timed_out(&self) -> bool {
        self.state.lock().unwrap().is_timed_out
    }

    pub fn target_brightness(&self) -> u32 {
        self.state.lock().unwrap().target_brightness
    }

    pub fn wake(&self) -> Result<(), String> {
        {
            let mut state = self.state.lock().unwrap();
            state.is_timed_out = false;
        }
        self.reapply()
    }

    /// Turns off backlight for inactivity timeout without resetting configured brightness level.
    pub fn sleep_timeout(&self) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        state.is_timed_out = true;
        let dev = match Self::ensure_device(&mut state) {
            Ok(d) => d,
            Err(e) => {
                state.device = None;
                return Err(e);
            }
        };
        let _ = alatus_rgb_wrapper::off(&dev, state.red, state.green, state.blue);
        let _ = write_sysfs_brightness(0);
        state.last_sysfs_brightness = 0;
        Ok(())
    }

    pub fn reapply(&self) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        state.is_timed_out = false;
        let dev = match Self::ensure_device(&mut state) {
            Ok(d) => d,
            Err(e) => {
                state.device = None;
                return Err(e);
            }
        };
        let _ = wmi_unlock();
        if let Err(e) = alatus_rgb_wrapper::set_firmware_mode(&dev, false) {
            state.device = None;
            return Err(e);
        }

        if state.target_brightness == 0 {
            let _ = alatus_rgb_wrapper::off(&dev, state.red, state.green, state.blue);
            let _ = write_sysfs_brightness(0);
            state.last_sysfs_brightness = 0;
        } else {
            let intensity = alatus_rgb_wrapper::percent_to_intensity(state.target_brightness);
            if let Err(e) =
                alatus_rgb_wrapper::set_color(&dev, state.red, state.green, state.blue, intensity)
            {
                state.device = None;
                return Err(e);
            }
            if let Some((_, max)) = read_sysfs_brightness() {
                let sysfs_val = percent_to_sysfs(state.target_brightness, max);
                let _ = write_sysfs_brightness(sysfs_val);
                state.last_sysfs_brightness = sysfs_val;
            }
        }
        Ok(())
    }

    /// Clears the cached device descriptor, performs WMI unlock, and reapplies
    /// current RGB color and brightness settings with retry backoff.
    pub fn reset_and_reapply(&self) -> Result<(), String> {
        let _ = wmi_unlock();
        {
            let mut state = self.state.lock().unwrap();
            state.device = None;
        }

        let mut last_err = String::new();
        for _ in 0..10 {
            match self.reapply() {
                Ok(()) => return Ok(()),
                Err(e) => {
                    last_err = e;
                    std::thread::sleep(std::time::Duration::from_millis(300));
                }
            }
        }
        Err(last_err)
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

    #[test]
    fn test_target_brightness_preserved_across_timeout() {
        let sysfs_cur = read_sysfs_brightness().map(|(c, _)| c).unwrap_or(0);
        let service = RgbService {
            state: Mutex::new(RgbServiceState {
                device: None,
                red: 255,
                green: 255,
                blue: 255,
                target_brightness: 80,
                last_sysfs_brightness: sysfs_cur,
                is_timed_out: false,
            }),
        };

        assert_eq!(service.get_brightness(), 80);
        assert_eq!(service.get_status().brightness, 80);

        // Enter timeout sleep
        {
            let mut state = service.state.lock().unwrap();
            state.is_timed_out = true;
        }

        // Status and brightness still reflect target preference, not 0
        assert_eq!(service.get_brightness(), 80);
        assert_eq!(service.get_status().brightness, 80);
        assert!(service.is_timed_out());

        // Wake
        {
            let mut state = service.state.lock().unwrap();
            state.is_timed_out = false;
        }

        assert_eq!(service.get_brightness(), 80);
        assert_eq!(service.get_status().brightness, 80);
        assert!(!service.is_timed_out());
    }
}
