//! ASUS sysfs-based battery driver.

use crate::sysfs::SysfsRoot;
use alatus_core::{
    battery::{BatteryDriver, BatteryInfo, BatteryStatus},
    capabilities::BatteryCapabilities,
    error::AlatusError,
};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Debug, Clone)]
pub struct AsusSysfsBatteryDriver {
    sysfs_root: SysfsRoot,
    battery_dir: PathBuf,
    supported_limits: Vec<u8>,
}

impl AsusSysfsBatteryDriver {
    pub fn new(
        sysfs_root: SysfsRoot,
        battery_path: impl AsRef<Path>,
        supported_limits: Vec<u8>,
    ) -> Self {
        let battery_dir = sysfs_root.resolve(battery_path);
        Self {
            sysfs_root,
            battery_dir,
            supported_limits: if supported_limits.is_empty() {
                vec![60, 80, 100]
            } else {
                supported_limits
            },
        }
    }

    pub fn sysfs_root(&self) -> &SysfsRoot {
        &self.sysfs_root
    }

    async fn read_sysfs_string(&self, attribute: &str) -> Result<String, AlatusError> {
        let path = self.battery_dir.join(attribute);
        fs::read_to_string(&path)
            .await
            .map(|s| s.trim().to_string())
            .map_err(|e| AlatusError::Sysfs {
                path,
                message: e.to_string(),
            })
    }

    async fn read_sysfs_u64(&self, attribute: &str) -> Result<u64, AlatusError> {
        let s = self.read_sysfs_string(attribute).await?;
        s.parse::<u64>().map_err(|e| AlatusError::Sysfs {
            path: self.battery_dir.join(attribute),
            message: format!("Failed to parse integer: {e}"),
        })
    }

    async fn get_threshold_file(&self) -> Result<&'static str, AlatusError> {
        let primary = self.battery_dir.join("charge_control_end_threshold");
        if fs::try_exists(&primary).await.unwrap_or(false) {
            return Ok("charge_control_end_threshold");
        }
        let fallback = self.battery_dir.join("charge_control_limit_max");
        if fs::try_exists(&fallback).await.unwrap_or(false) {
            return Ok("charge_control_limit_max");
        }
        Err(AlatusError::Sysfs {
            path: self.battery_dir.clone(),
            message: "No charge control threshold attribute found".into(),
        })
    }
}

#[async_trait]
impl BatteryDriver for AsusSysfsBatteryDriver {
    fn name(&self) -> &'static str {
        "asus-sysfs-battery"
    }

    fn capabilities(&self) -> BatteryCapabilities {
        let mut caps = BatteryCapabilities::empty();
        if self
            .battery_dir
            .join("charge_control_end_threshold")
            .exists()
            || self.battery_dir.join("charge_control_limit_max").exists()
        {
            caps |= BatteryCapabilities::CHARGE_LIMIT_CONFIGURABLE;
        }
        if self.battery_dir.join("energy_full").exists()
            && self.battery_dir.join("energy_full_design").exists()
        {
            caps |= BatteryCapabilities::HEALTH_REPORTING;
        }
        if self.battery_dir.join("power_now").exists() {
            caps |= BatteryCapabilities::DISCHARGE_RATE_REPORTING;
        }
        caps
    }

    async fn get_info(&self) -> Result<BatteryInfo, AlatusError> {
        let capacity = self.read_sysfs_u64("capacity").await.unwrap_or(0) as u8;
        let status_str = self
            .read_sysfs_string("status")
            .await
            .unwrap_or_else(|_| "Unknown".into());

        let status = match status_str.to_lowercase().as_str() {
            "charging" => BatteryStatus::Charging,
            "discharging" => BatteryStatus::Discharging,
            "not charging" => BatteryStatus::NotCharging,
            "full" => BatteryStatus::Full,
            _ => BatteryStatus::Unknown,
        };

        let charge_limit = self.get_charge_limit().await.ok();
        let health_percentage = self.get_health_percentage().await.ok();
        let power_now = self.read_sysfs_u64("power_now").await.ok();

        Ok(BatteryInfo {
            percentage: capacity,
            status,
            charge_limit,
            health_percentage,
            power_now_microwatts: power_now,
        })
    }

    async fn get_charge_limit(&self) -> Result<u8, AlatusError> {
        let attr = self.get_threshold_file().await?;
        let val = self.read_sysfs_u64(attr).await?;
        Ok(val as u8)
    }

    async fn set_charge_limit(&self, limit: u8) -> Result<(), AlatusError> {
        if !self.supported_limits.contains(&limit) {
            return Err(AlatusError::InvalidParameter(format!(
                "Charge limit {}% is not supported. Supported limits: {:?}",
                limit, self.supported_limits
            )));
        }

        let attr = self.get_threshold_file().await?;
        let path = self.battery_dir.join(attr);

        fs::write(&path, format!("{limit}\n"))
            .await
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::PermissionDenied => AlatusError::PermissionDenied(format!(
                    "Permission denied writing to {}",
                    path.display()
                )),
                _ => AlatusError::Sysfs {
                    path: path.clone(),
                    message: format!("Failed to write charge limit: {e}"),
                },
            })?;

        tracing::info!("Updated battery charge limit to {}%", limit);
        Ok(())
    }

    async fn get_health_percentage(&self) -> Result<u8, AlatusError> {
        let energy_full = self.read_sysfs_u64("energy_full").await?;
        let energy_design = self.read_sysfs_u64("energy_full_design").await?;

        if energy_design == 0 {
            return Err(AlatusError::Sysfs {
                path: self.battery_dir.join("energy_full_design"),
                message: "energy_full_design is 0".into(),
            });
        }

        let health = (energy_full as f64 / energy_design as f64 * 100.0).round() as u8;
        Ok(health.min(100))
    }

    async fn is_charging(&self) -> Result<bool, AlatusError> {
        let status = self.read_sysfs_string("status").await?;
        Ok(status.eq_ignore_ascii_case("charging"))
    }
}
