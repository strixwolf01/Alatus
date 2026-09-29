//! ASUS hybrid thermal driver combining standard ACPI platform_profile,
//! hwmon fan telemetry, and asus-nb-wmi DebugFS Full Speed fan boost.

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

        // Check if debugfs path is genuinely writable
        let can_full_speed = if supports_full_speed {
            if let Some(ref path) = debugfs_devs_path {
                match std::fs::OpenOptions::new().write(true).open(path) {
                    Ok(_) => true,
                    Err(e) => {
                        tracing::warn!(
                            "DebugFS path '{}' not writable ({e}). Full Speed mode will be disabled (Kernel Lockdown / Secure Boot active).",
                            path.display()
                        );
                        false
                    }
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
            debugfs_fan_register: debugfs_fan_register.unwrap_or(0x00110013),
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
        let default_hwmon = self.sysfs_root.resolve("/devices/platform/asus-nb-wmi/hwmon");
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
            // Default standard ACPI fallback choices
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
        if self.is_full_speed_active.load(Ordering::SeqCst) {
            return Ok(ThermalProfileMode::FullSpeed);
        }

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
        match mode {
            ThermalProfileMode::FullSpeed => {
                if !self.can_full_speed.load(Ordering::SeqCst) {
                    return Err(AlatusError::UnsupportedCapability(
                        "Full Speed fan mode is unavailable (DebugFS write blocked or Lockdown active)",
                    ));
                }

                // 1. First set platform profile to performance
                fs::write(&self.platform_profile_path, "performance\n")
                    .await
                    .map_err(|e| AlatusError::Sysfs {
                        path: self.platform_profile_path.clone(),
                        message: format!("Failed to set base performance profile: {e}"),
                    })?;

                // 2. Write DebugFS fan register for 100% boost
                if let Some(ref devs_path) = self.debugfs_devs_path {
                    let cmd = format!("{:#010x} 0x1\n", self.debugfs_fan_register);
                    fs::write(devs_path, &cmd)
                        .await
                        .map_err(|e| AlatusError::Sysfs {
                            path: devs_path.clone(),
                            message: format!("Failed to write DebugFS fan boost register: {e}"),
                        })?;
                }

                self.is_full_speed_active.store(true, Ordering::SeqCst);
                tracing::info!("Activated Full Speed thermal mode");
            }
            ThermalProfileMode::Quiet
            | ThermalProfileMode::Balanced
            | ThermalProfileMode::Performance => {
                // If previously full speed, disable fan boost first
                if self.is_full_speed_active.load(Ordering::SeqCst) {
                    if let Some(ref devs_path) = self.debugfs_devs_path {
                        let cmd = format!("{:#010x} 0x0\n", self.debugfs_fan_register);
                        let _ = fs::write(devs_path, cmd).await;
                    }
                    self.is_full_speed_active.store(false, Ordering::SeqCst);
                }

                let profile_str = match mode {
                    ThermalProfileMode::Quiet => "quiet",
                    ThermalProfileMode::Balanced => "balanced",
                    ThermalProfileMode::Performance => "performance",
                    _ => unreachable!(),
                };

                fs::write(&self.platform_profile_path, format!("{profile_str}\n"))
                    .await
                    .map_err(|e| match e.kind() {
                        std::io::ErrorKind::PermissionDenied => {
                            AlatusError::PermissionDenied(format!(
                                "Permission denied writing to {}",
                                self.platform_profile_path.display()
                            ))
                        }
                        _ => AlatusError::Sysfs {
                            path: self.platform_profile_path.clone(),
                            message: format!("Failed to write platform_profile: {e}"),
                        },
                    })?;

                tracing::info!("Set thermal profile to {profile_str}");
            }
            ThermalProfileMode::Custom(_) => {
                return Err(AlatusError::UnsupportedCapability(
                    "Custom thermal curves are not supported on this model",
                ));
            }
        }

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
