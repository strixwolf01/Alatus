use alatus_core::AlatusError;
use serde::{Deserialize, Serialize};

pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileMetadata {
    pub name: String,
    pub author: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DmiMatch {
    pub sys_vendor: Option<String>,
    pub product_name: Vec<String>,
    pub board_name: Vec<String>,
}

impl DmiMatch {
    pub fn matches(
        &self,
        vendor: Option<&str>,
        product: Option<&str>,
        board: Option<&str>,
    ) -> bool {
        if let Some(ref expected_vendor) = self.sys_vendor {
            if let Some(actual_vendor) = vendor {
                if !actual_vendor
                    .to_lowercase()
                    .contains(&expected_vendor.to_lowercase())
                {
                    return false;
                }
            } else {
                return false;
            }
        }

        let product_matched = if self.product_name.is_empty() {
            true
        } else if let Some(actual_product) = product {
            self.product_name
                .iter()
                .any(|p| actual_product.to_lowercase().contains(&p.to_lowercase()))
        } else {
            false
        };

        let board_matched = if self.board_name.is_empty() {
            true
        } else if let Some(actual_board) = board {
            self.board_name
                .iter()
                .any(|b| actual_board.to_lowercase().contains(&b.to_lowercase()))
        } else {
            false
        };

        product_matched || board_matched
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BatteryConfig {
    pub driver: String,
    pub default_charge_limit: u8,
    pub supported_limits: Vec<u8>,
    pub sysfs_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThermalConfig {
    pub driver: String,
    pub platform_profile_path: String,
    pub debugfs_devs_path: Option<String>,
    pub debugfs_fan_register: Option<u32>,
    #[serde(default)]
    pub supports_full_speed: bool,
    #[serde(default = "default_fan_count")]
    pub fan_count: u32,
}

fn default_fan_count() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LightingConfig {
    pub driver: String,
    pub hid_vendor_id: u16,
    pub hid_product_id: u16,
    pub report_id: u8,
    pub lamp_count: u32,
    pub default_mode: String,
    pub default_brightness: u8,
    #[serde(default = "default_supported_modes")]
    pub supported_modes: Vec<String>,
}

fn default_supported_modes() -> Vec<String> {
    vec!["Static".to_string()]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HotkeyConfig {
    pub driver: String,
    pub device_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Profile {
    pub schema: u32,
    pub metadata: ProfileMetadata,
    #[serde(rename = "match")]
    pub dmi_match: DmiMatch,
    pub battery: BatteryConfig,
    pub thermal: ThermalConfig,
    pub lighting: Option<LightingConfig>,
    pub hotkeys: Option<HotkeyConfig>,
}

impl Profile {
    pub fn parse_toml(content: &str) -> Result<Self, AlatusError> {
        let profile: Profile = toml::from_str(content)
            .map_err(|e| AlatusError::Profile(format!("Failed to parse profile TOML: {e}")))?;

        if profile.schema != SUPPORTED_SCHEMA_VERSION {
            return Err(AlatusError::Profile(format!(
                "Unsupported profile schema version: {}. Expected version {}",
                profile.schema, SUPPORTED_SCHEMA_VERSION
            )));
        }

        profile.validate()?;
        Ok(profile)
    }

    pub fn validate(&self) -> Result<(), AlatusError> {
        if self.battery.supported_limits.is_empty() {
            return Err(AlatusError::Profile(
                "Battery configuration must define at least one supported limit".into(),
            ));
        }
        if !self
            .battery
            .supported_limits
            .contains(&self.battery.default_charge_limit)
        {
            return Err(AlatusError::Profile(format!(
                "Default charge limit {} is not in supported limits {:?}",
                self.battery.default_charge_limit, self.battery.supported_limits
            )));
        }
        Ok(())
    }
}
