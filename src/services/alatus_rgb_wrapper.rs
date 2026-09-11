// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Driver and hardware control wrapper for ASUS ITE5570 HID LampArray keyboard backlight.
//!
//! Handles communication with the embedded controller via Linux `/dev/hidraw*`
//! using the `HIDIOCSFEATURE` ioctl. Supports setting static RGB colors, brightness
//! levels, toggling autonomous firmware mode vs host control, and OEM effects.

use std::fs::OpenOptions;
use std::os::fd::AsRawFd;
use std::path::Path;

/// `HIDIOCSFEATURE(length)` — `_IOW('H', 0x06, int)` ioctl request for HID feature reports.
const HIDIOCSFEATURE_BASE: libc::c_ulong = 0xC000_4806;

fn hid_ioc_sfeature(length: usize) -> libc::c_ulong {
    HIDIOCSFEATURE_BASE | ((length as libc::c_ulong) << 16)
}

/// Host control mode: embedded controller accepts commands from host.
pub const MODE_HOST: u8 = 0x00;
/// Firmware/autonomous mode: embedded controller runs built-in lighting animation.
pub const MODE_FIRMWARE: u8 = 0x01;

/// Hardware profile specification for supported ITE5570 LampArray controllers.
#[derive(Debug, PartialEq, Eq)]
pub struct DeviceProfile {
    pub hid_id: &'static str,
    pub hid_name: &'static str,
    pub model: &'static str,
    pub firmware_report_id: u8,
    pub color_report_id: u8,
    pub required_modules: &'static [&'static str],
}

pub const SUPPORTED_DEVICES: &[DeviceProfile] = &[
    DeviceProfile {
        hid_id: "0018:00000B05:000019B6",
        hid_name: "ITE5570:00 0B05:19B6",
        model: "ASUS Vivobook / Zenbook series with (ITE5570 0x19B6) Chipset",
        firmware_report_id: 0x0B,
        color_report_id: 0x05,
        required_modules: &["asus-nb-wmi"],
    },
    DeviceProfile {
        hid_id: "0018:00000B05:00005570",
        hid_name: "ITE5570:00 0B05:5570",
        model: "ASUS Vivobook / Zenbook series with (ITE5570 0x5570) Chipset",
        firmware_report_id: 0x46,
        color_report_id: 0x45,
        required_modules: &[],
    },
];

/// A detected ASUS LampArray RGB controller ready for feature report I/O.
#[derive(Debug, Clone)]
pub struct RgbDevice {
    pub path: String,
    pub hid_id: String,
    pub hid_name: String,
    pub model: &'static str,
    pub firmware_report_id: u8,
    pub color_report_id: u8,
    pub required_modules: &'static [&'static str],
}

/// Check if a Linux kernel module is loaded under `/sys/module/`.
pub fn module_loaded(name: &str) -> bool {
    Path::new(&format!("/sys/module/{}", name.replace('-', "_"))).exists()
}

/// Match and score a detected HID device against known hardware profiles.
fn score_device(hid_id: &str, hid_name: &str) -> (i32, Option<&'static DeviceProfile>) {
    for profile in SUPPORTED_DEVICES {
        if profile.hid_id == hid_id {
            return (100, Some(profile));
        }
    }
    for profile in SUPPORTED_DEVICES {
        if profile.hid_name == hid_name {
            return (90, Some(profile));
        }
    }
    (-1, None)
}

/// Discovers connected ITE5570 keyboard backlight controllers via `/sys/class/hidraw`.
pub fn discover() -> Result<RgbDevice, String> {
    let base = Path::new("/sys/class/hidraw");
    if !base.exists() {
        return Err("No hidraw subsystem found under /sys/class/hidraw".to_string());
    }

    let mut best: Option<RgbDevice> = None;
    let mut best_score = -1;

    let mut entries: Vec<_> = match std::fs::read_dir(base) {
        Ok(entries) => entries.flatten().map(|e| e.path()).collect(),
        Err(e) => return Err(format!("Failed to read {}: {}", base.display(), e)),
    };
    entries.sort();

    for dev in entries {
        let uevent = dev.join("device").join("uevent");
        if !uevent.exists() {
            continue;
        }
        let content = match std::fs::read_to_string(&uevent) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut hid_id = String::new();
        let mut hid_name = String::new();
        for line in content.lines() {
            if let Some(v) = line.strip_prefix("HID_ID=") {
                hid_id = v.trim().to_string();
            } else if let Some(v) = line.strip_prefix("HID_NAME=") {
                hid_name = v.trim().to_string();
            }
        }

        let (score, profile) = score_device(&hid_id, &hid_name);
        if let Some(profile) = profile
            && score > best_score
        {
            best = Some(RgbDevice {
                path: format!(
                    "/dev/{}",
                    dev.file_name().unwrap_or_default().to_string_lossy()
                ),
                hid_id: hid_id.clone(),
                hid_name: hid_name.clone(),
                model: profile.model,
                firmware_report_id: profile.firmware_report_id,
                color_report_id: profile.color_report_id,
                required_modules: profile.required_modules,
            });
            best_score = score;
        }
    }

    match best {
        Some(dev) => {
            let missing: Vec<&str> = dev
                .required_modules
                .iter()
                .copied()
                .filter(|m| !module_loaded(m))
                .collect();
            if !missing.is_empty() {
                return Err(format!(
                    "Required kernel module missing: {}. \
                     The ITE5570 controller may ignore HID LampArray commands until it is loaded.\n\
                     Load once:\n  sudo modprobe {}\n\
                     Load on boot:\n  echo {} | sudo tee /etc/modules-load.d/{}.conf",
                    missing.join(", "),
                    missing.join(" "),
                    missing[0],
                    missing[0]
                ));
            }
            Ok(dev)
        }
        None => Err(
            "No supported ITE5570 HID LampArray device found under /sys/class/hidraw. \
             Expected ASUS ITE5570 controller (HID_ID 0018:00000B05:000019B6 or 0018:00000B05:00005570)."
                .to_string(),
        ),
    }
}

/// Send a HID feature report `[report_id, ..payload]` to the device node via ioctl.
fn hid_set_feature(dev: &RgbDevice, report_id: u8, payload: &[u8]) -> Result<(), String> {
    let mut buf = Vec::with_capacity(1 + payload.len());
    buf.push(report_id);
    buf.extend_from_slice(payload);

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&dev.path)
        .map_err(|err| {
            format!(
                "Cannot open {}: {}. Ensure permissions or udev rules are configured.",
                dev.path, err
            )
        })?;

    let cmd = hid_ioc_sfeature(buf.len());
    let ret = unsafe { libc::ioctl(file.as_raw_fd(), cmd, buf.as_mut_ptr()) };
    if ret < 0 {
        let err = std::io::Error::last_os_error();
        return Err(format!(
            "HID feature report 0x{:02X} to {} failed: {}",
            report_id, dev.path, err
        ));
    }
    Ok(())
}

/// Switches between autonomous firmware mode and host control mode.
pub fn set_firmware_mode(dev: &RgbDevice, enabled: bool) -> Result<(), String> {
    hid_set_feature(
        dev,
        dev.firmware_report_id,
        &[if enabled { MODE_FIRMWARE } else { MODE_HOST }],
    )
}

/// Constructs a static color feature report packet buffer.
pub fn build_asus_rgb_packet(report_id: u8, r: u8, g: u8, b: u8, intensity: u8) -> Vec<u8> {
    vec![report_id, 0x01, 0x00, 0x00, 0x00, 0x00, r, g, b, intensity]
}

/// Sends a static color feature report with RGB values and intensity byte.
pub fn set_color(dev: &RgbDevice, r: u8, g: u8, b: u8, intensity: u8) -> Result<(), String> {
    let payload = [
        0x01,
        0x00,
        0x00,
        0x00,
        0x00,
        r.clamp(0, 255),
        g.clamp(0, 255),
        b.clamp(0, 255),
        intensity.clamp(0, 255),
    ];
    hid_set_feature(dev, dev.color_report_id, &payload)
}

/// Converts a brightness percentage (0-100) to an 8-bit intensity byte (0-255).
pub fn percent_to_intensity(percent: u32) -> u8 {
    let p = percent.clamp(0, 100);
    (((p * 255) + 50) / 100) as u8
}

/// High-level static color control: enables host mode and applies color + brightness.
pub fn set_static(dev: &RgbDevice, r: u8, g: u8, b: u8, percent: u32) -> Result<(), String> {
    let intensity = percent_to_intensity(percent);
    set_firmware_mode(dev, false)?;
    set_color(dev, r, g, b, intensity)
}

/// Turns off the backlight while maintaining host mode.
pub fn off(dev: &RgbDevice, r: u8, g: u8, b: u8) -> Result<(), String> {
    set_static(dev, r, g, b, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_device() -> RgbDevice {
        RgbDevice {
            path: "/dev/nonexistent-hidraw".to_string(),
            hid_id: "0018:00000B05:000019B6".to_string(),
            hid_name: "ITE5570:00 0B05:19B6".to_string(),
            model: "TESTS",
            firmware_report_id: 0x0B,
            color_report_id: 0x05,
            required_modules: &[],
        }
    }

    #[test]
    fn intensity_curve_linear_quantization() {
        for (p, expected) in [
            (0u32, 0u8),
            (1, 3),
            (32, 82),
            (33, 84),
            (50, 128),
            (66, 168),
            (80, 204),
            (100, 255),
        ] {
            assert_eq!(percent_to_intensity(p), expected, "percent {p}");
        }
        assert_eq!(percent_to_intensity(150), 255, "clamped");
    }

    #[test]
    fn ioctl_fails_cleanly_on_missing_node() {
        let dev = fake_device();
        let err = set_static(&dev, 255, 0, 0, 50).unwrap_err();
        assert!(
            err.contains("Cannot open"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn firmware_mode_needs_device() {
        let dev = fake_device();
        let err = set_firmware_mode(&dev, false).unwrap_err();
        assert!(err.contains("Cannot open"), "unexpected: {err}");
    }

    #[test]
    fn score_prefers_exact_hid_id() {
        assert_eq!(
            score_device("0018:00000B05:000019B6", "anything"),
            (100, Some(&SUPPORTED_DEVICES[0]))
        );
        assert_eq!(
            score_device("who", "ITE5570:00 0B05:5570"),
            (90, Some(&SUPPORTED_DEVICES[1]))
        );
        assert_eq!(score_device("who", "USB Mouse"), (-1, None));
    }

    #[test]
    #[ignore]
    fn rgb_hardware_roundtrip() {
        let dev = discover().expect("ITE5570 HID device should be discoverable");
        assert_eq!(dev.hid_id, "0018:00000B05:000019B6");
        assert_eq!(dev.firmware_report_id, 0x0B);
        assert_eq!(dev.color_report_id, 0x05);

        let scrollback = |r: u8, g: u8, b: u8, p: u32| {
            set_static(&dev, r, g, b, p).expect("static colour write must succeed");
        };

        set_firmware_mode(&dev, false).expect("host-mode handshake must succeed");
        set_firmware_mode(&dev, true).expect("firmware/autonomous mode must be representable");
        set_firmware_mode(&dev, false).expect("back to host mode");

        scrollback(180, 0, 255, 40);
        set_static(&dev, 180, 0, 255, 100).expect("brightness 100");
        set_static(&dev, 180, 0, 255, 66).expect("brightness 66");
        off(&dev, 180, 0, 255).expect("off");
    }
}
