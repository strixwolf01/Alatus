//! ASUS hybrid thermal driver combining standard ACPI platform_profile,
//! hwmon fan telemetry, and asus-nb-wmi DebugFS fan state sequencing.

use crate::sysfs::SysfsRoot;
use alatus_core::{
    capabilities::ThermalCapabilities,
    error::AlatusError,
    thermal::{FanStatus, ThermalDriver, ThermalProfileMode},
};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::fs;

pub const DEFAULT_FAN_REGISTER: u32 = 0x00110019;

#[derive(Debug)]
pub struct AsusHybridThermalDriver {
    sysfs_root: SysfsRoot,
    platform_profile_path: PathBuf,
    debugfs_devs_path: Option<PathBuf>,
    debugfs_fan_register: u32,
    hwmon_dir: Option<PathBuf>,
    can_full_speed: AtomicBool,
    is_full_speed_active: AtomicBool,
}

impl AsusHybridThermalDriver {
    pub fn new(
        sysfs_root: SysfsRoot,
        platform_profile_path: impl AsRef<Path>,
        debugfs_devs_path: Option<impl AsRef<Path>>,
        debugfs_fan_register: Option<u32>,
        hwmon_dir: Option<impl AsRef<Path>>,
        supports_full_speed: bool,
    ) -> Self {
        let platform_profile_path = sysfs_root.resolve(platform_profile_path);
        let debugfs_devs_path = debugfs_devs_path.map(|p| sysfs_root.resolve(p));
        let hwmon_dir = hwmon_dir.map(|p| sysfs_root.resolve(p));

        // Check if debugfs path is genuinely accessible
        let can_full_speed = if supports_full_speed {
            if let Some(ref path) = debugfs_devs_path {
                let base = path.parent().unwrap_or(path);
                let dev_id_path = base.join("dev_id");
                if dev_id_path.exists() {
                    std::fs::OpenOptions::new()
                        .write(true)
                        .open(&dev_id_path)
                        .is_ok()
                } else {
                    std::fs::OpenOptions::new().write(true).open(path).is_ok()
                }
            } else {
                false
            }
        } else {
            false
        };

        Self {
            sysfs_root,
            platform_profile_path,
            debugfs_devs_path,
            debugfs_fan_register: debugfs_fan_register.unwrap_or(DEFAULT_FAN_REGISTER),
            hwmon_dir,
            can_full_speed: AtomicBool::new(can_full_speed),
            is_full_speed_active: AtomicBool::new(false),
        }
    }

    pub fn set_can_full_speed(&self, can: bool) {
        self.can_full_speed.store(can, Ordering::SeqCst);
    }

    fn find_hwmon_dir(&self) -> Option<PathBuf> {
        if let Some(ref dir) = self.hwmon_dir {
            if dir.exists() {
                return Some(dir.clone());
            }
        }

        // Try standard asus-nb-wmi hwmon path
        let default_hwmon = self
            .sysfs_root
            .resolve("/devices/platform/asus-nb-wmi/hwmon");
        if let Ok(entries) = std::fs::read_dir(&default_hwmon) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    return Some(path);
                }
            }
        }

        None
    }
}

#[async_trait]
impl ThermalDriver for AsusHybridThermalDriver {
    fn name(&self) -> &'static str {
        "asus-hybrid-thermal"
    }

    fn capabilities(&self) -> ThermalCapabilities {
        let mut caps = ThermalCapabilities::empty();
        if self.platform_profile_path.exists() {
            caps |= ThermalCapabilities::PLATFORM_PROFILE_STANDARD;
        }
        if self.can_full_speed.load(Ordering::SeqCst) {
            caps |= ThermalCapabilities::FULL_SPEED_FAN;
        }
        if self.find_hwmon_dir().is_some() {
            caps |= ThermalCapabilities::FAN_RPM_READBACK;
        }
        caps
    }

    async fn available_profiles(&self) -> Result<Vec<ThermalProfileMode>, AlatusError> {
        let choices_path = self
            .platform_profile_path
            .parent()
            .unwrap_or_else(|| Path::new("/"))
            .join("platform_profile_choices");

        let mut modes = Vec::new();
        if let Ok(content) = fs::read_to_string(&choices_path).await {
            for word in content.split_whitespace() {
                match word.to_lowercase().as_str() {
                    "quiet" => modes.push(ThermalProfileMode::Quiet),
                    "balanced" => modes.push(ThermalProfileMode::Balanced),
                    "performance" => modes.push(ThermalProfileMode::Performance),
                    _ => {}
                }
            }
        } else {
            // Default standard choices
            modes = vec![
                ThermalProfileMode::Quiet,
                ThermalProfileMode::Balanced,
                ThermalProfileMode::Performance,
            ];
        }

        if self.can_full_speed.load(Ordering::SeqCst) {
            modes.push(ThermalProfileMode::FullSpeed);
        }

        Ok(modes)
    }

    async fn get_current_profile(&self) -> Result<ThermalProfileMode, AlatusError> {
        // 1. Try reading live hardware state from DebugFS ctrl_param FIRST
        if let Some(ref devs_path) = self.debugfs_devs_path {
            let base = devs_path.parent().unwrap_or(devs_path);
            let dev_id_path = base.join("dev_id");
            let ctrl_param_path = base.join("ctrl_param");

            if dev_id_path.exists() && ctrl_param_path.exists() {
                let reg_str = format!("{:#x}\n", self.debugfs_fan_register);
                if fs::write(&dev_id_path, &reg_str).await.is_ok() {
                    if let Ok(content) = fs::read_to_string(&ctrl_param_path).await {
                        let trimmed = content.trim();
                        let mode = match trimmed {
                            "0x00000000" | "0" => Some(ThermalProfileMode::Balanced),
                            "0x00000001" | "1" => Some(ThermalProfileMode::Quiet),
                            "0x00000002" | "2" => Some(ThermalProfileMode::Performance),
                            "0x00000003" | "3" => Some(ThermalProfileMode::FullSpeed),
                            _ => None,
                        };
                        if let Some(m) = mode {
                            self.is_full_speed_active
                                .store(m == ThermalProfileMode::FullSpeed, Ordering::SeqCst);
                            return Ok(m);
                        }
                    }
                }
            }
        }

        if self.is_full_speed_active.load(Ordering::SeqCst) {
            return Ok(ThermalProfileMode::FullSpeed);
        }

        // 2. Fall back to standard platform_profile
        let content = fs::read_to_string(&self.platform_profile_path)
            .await
            .map_err(|e| AlatusError::Sysfs {
                path: self.platform_profile_path.clone(),
                message: format!("Failed to read platform_profile: {e}"),
            })?;

        match content.trim().to_lowercase().as_str() {
            "quiet" => Ok(ThermalProfileMode::Quiet),
            "balanced" => Ok(ThermalProfileMode::Balanced),
            "performance" => Ok(ThermalProfileMode::Performance),
            other => Err(AlatusError::Sysfs {
                path: self.platform_profile_path.clone(),
                message: format!("Unknown platform profile string: '{other}'"),
            }),
        }
    }

    async fn set_profile(&self, mode: ThermalProfileMode) -> Result<(), AlatusError> {
        let fan_val = match mode {
            ThermalProfileMode::Balanced => 0,
            ThermalProfileMode::Quiet => 1,
            ThermalProfileMode::Performance => 2,
            ThermalProfileMode::FullSpeed => 3,
            ThermalProfileMode::Custom(_) => {
                return Err(AlatusError::UnsupportedCapability(
                    "Custom thermal curves are not supported on this model",
                ));
            }
        };

        // 1. Keep Linux kernel platform_profile in sync FIRST so kernel does not clobber hardware registers afterwards
        if self.platform_profile_path.exists() {
            let kernel_profile = match mode {
                ThermalProfileMode::Quiet => "quiet\n",
                ThermalProfileMode::Balanced => "balanced\n",
                ThermalProfileMode::Performance | ThermalProfileMode::FullSpeed => "performance\n",
                _ => "balanced\n",
            };
            let _ = fs::write(&self.platform_profile_path, kernel_profile).await;
        }

        // 2. Write to asus-nb-wmi DebugFS AFTER so kernel does not overwrite it
        if let Some(ref devs_path) = self.debugfs_devs_path {
            let base = devs_path.parent().unwrap_or(devs_path);
            let dev_id_path = base.join("dev_id");
            let ctrl_param_path = base.join("ctrl_param");

            if dev_id_path.exists() && ctrl_param_path.exists() {
                let reg_str = format!("{:#x}\n", self.debugfs_fan_register);
                if let Err(e) = fs::write(&dev_id_path, &reg_str).await {
                    tracing::warn!("Failed writing to dev_id: {e}");
                }
                if let Err(e) = fs::write(&ctrl_param_path, format!("{fan_val}\n")).await {
                    tracing::warn!("Failed writing to ctrl_param: {e}");
                }
                // Trigger ACPI evaluation by reading devs
                let _ = fs::read_to_string(devs_path).await;
            } else if devs_path.exists() {
                // Fallback for mock environments / legacy kernels
                let cmd = format!("{:#010x} 0x{fan_val}\n", self.debugfs_fan_register);
                let _ = fs::write(devs_path, cmd).await;
            }
        }

        if mode == ThermalProfileMode::FullSpeed {
            self.is_full_speed_active.store(true, Ordering::SeqCst);
        } else {
            self.is_full_speed_active.store(false, Ordering::SeqCst);
        }

        tracing::info!("Set thermal profile to {:?}", mode);
        Ok(())
    }

    async fn get_fans(&self) -> Result<Vec<FanStatus>, AlatusError> {
        let hwmon = match self.find_hwmon_dir() {
            Some(dir) => dir,
            None => return Ok(Vec::new()),
        };

        let mut fans = Vec::new();
        let mut idx = 1;
        loop {
            let input_path = hwmon.join(format!("fan{idx}_input"));
            if !input_path.exists() {
                break;
            }

            let rpm = match fs::read_to_string(&input_path).await {
                Ok(s) => s.trim().parse::<u32>().unwrap_or(0),
                Err(_) => 0,
            };

            let label_path = hwmon.join(format!("fan{idx}_label"));
            let label = fs::read_to_string(&label_path)
                .await
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| format!("Fan {idx}"));

            fans.push(FanStatus {
                label,
                current_rpm: rpm,
                max_rpm: Some(5500),
            });

            idx += 1;
        }

        Ok(fans)
    }
}
