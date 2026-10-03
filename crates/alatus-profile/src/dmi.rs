use crate::model::Profile;
use alatus_core::AlatusError;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DmiInfo {
    pub sys_vendor: Option<String>,
    pub product_name: Option<String>,
    pub board_name: Option<String>,
    pub bios_version: Option<String>,
}

pub struct DmiReader {
    sysfs_dmi_path: PathBuf,
}

impl Default for DmiReader {
    fn default() -> Self {
        Self {
            sysfs_dmi_path: PathBuf::from("/sys/class/dmi/id"),
        }
    }
}

impl DmiReader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_sysfs_path(path: impl Into<PathBuf>) -> Self {
        Self {
            sysfs_dmi_path: path.into(),
        }
    }

    pub fn read_dmi(&self) -> Result<DmiInfo, AlatusError> {
        let read_field = |filename: &str| -> Option<String> {
            let path = self.sysfs_dmi_path.join(filename);
            fs::read_to_string(&path)
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        };

        Ok(DmiInfo {
            sys_vendor: read_field("sys_vendor"),
            product_name: read_field("product_name"),
            board_name: read_field("board_name"),
            bios_version: read_field("bios_version"),
        })
    }
}

pub const S5506MA_DEFAULT_PROFILE: &str = r#"
schema = 1

[metadata]
name = "ASUS Vivobook S 15 OLED (S5506MA)"
author = "Alatus Team"
description = "Embedded default hardware mapping for Vivobook S15 with ITE5570 LampArray keyboard"

[match]
sys_vendor = "ASUSTeK COMPUTER INC."
product_name = ["Vivobook S 15", "S5506MA"]
board_name = ["S5506MA"]

[battery]
driver = "asus-sysfs-battery"
default_charge_limit = 80
supported_limits = [60, 80, 100]
sysfs_path = "/sys/class/power_supply/BAT0"

[thermal]
driver = "asus-hybrid-thermal"
platform_profile_path = "/sys/firmware/acpi/platform_profile"
debugfs_devs_path = "/sys/kernel/debug/asus-nb-wmi/devs"
debugfs_fan_register = 0x00110019
supports_full_speed = true
fan_count = 2

[lighting]
driver = "asus-ite5570-lamparray"
hid_vendor_id = 0x0B05
hid_product_id = 0x19B6
report_id = 0x5A
lamp_count = 1
default_mode = "Static"
default_brightness = 3

[hotkeys]
driver = "asus-wmi-evdev"
device_name = "Asus WMI hotkeys"
"#;

pub struct ProfileResolver {
    search_dirs: Vec<PathBuf>,
}

impl Default for ProfileResolver {
    fn default() -> Self {
        Self {
            search_dirs: vec![
                PathBuf::from("/etc/alatus/profiles"),
                PathBuf::from("/usr/share/alatus/profiles"),
            ],
        }
    }
}

impl ProfileResolver {
    pub fn new(search_dirs: Vec<PathBuf>) -> Self {
        Self { search_dirs }
    }

    pub fn resolve(&self, dmi: &DmiInfo) -> Result<Profile, AlatusError> {
        let mut matched_profiles: Vec<(Profile, (usize, usize), PathBuf)> = Vec::new();

        for (dir_idx, dir) in self.search_dirs.iter().enumerate() {
            if !dir.exists() {
                continue;
            }
            let dir_priority = self.search_dirs.len().saturating_sub(dir_idx);

            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|ext| ext.to_str()) == Some("toml") {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if let Ok(profile) = Profile::parse_toml(&content) {
                                if profile.dmi_match.matches(
                                    dmi.sys_vendor.as_deref(),
                                    dmi.product_name.as_deref(),
                                    dmi.board_name.as_deref(),
                                ) {
                                    let mut score = 0;
                                    if !profile.dmi_match.product_name.is_empty() {
                                        score += 2;
                                    }
                                    if !profile.dmi_match.board_name.is_empty() {
                                        score += 2;
                                    }
                                    matched_profiles.push((profile, (score, dir_priority), path));
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some((best_profile, _, path)) = matched_profiles.into_iter().max_by_key(|(_, score, _)| *score) {
            tracing::info!(
                "Resolved machine profile from {}: {}",
                path.display(),
                best_profile.metadata.name
            );
            return Ok(best_profile);
        }

        // Fallback to embedded default profile if matched
        let default_profile = Profile::parse_toml(S5506MA_DEFAULT_PROFILE)?;
        if default_profile.dmi_match.matches(
            dmi.sys_vendor.as_deref(),
            dmi.product_name.as_deref(),
            dmi.board_name.as_deref(),
        ) {
            tracing::info!(
                "Using embedded default profile: {}",
                default_profile.metadata.name
            );
            return Ok(default_profile);
        }

        Err(AlatusError::Profile(format!(
            "No compatible hardware profile found for DMI: vendor={:?}, product={:?}, board={:?}",
            dmi.sys_vendor, dmi.product_name, dmi.board_name
        )))
    }
}
