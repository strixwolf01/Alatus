//! Dynamic driver registry and component factory for Alatus.

use alatus_core::{
    battery::BatteryDriver, error::AlatusError, hotkey::HotkeyDriver, lighting::LightingDriver,
    thermal::ThermalDriver,
};
use alatus_profile::model::{BatteryConfig, HotkeyConfig, LightingConfig, Profile, ThermalConfig};
use std::collections::HashMap;
use std::sync::Arc;

pub type BatteryFactory =
    Arc<dyn Fn(&BatteryConfig) -> Result<Arc<dyn BatteryDriver>, AlatusError> + Send + Sync>;
pub type ThermalFactory =
    Arc<dyn Fn(&ThermalConfig) -> Result<Arc<dyn ThermalDriver>, AlatusError> + Send + Sync>;
pub type LightingFactory =
    Arc<dyn Fn(&LightingConfig) -> Result<Arc<dyn LightingDriver>, AlatusError> + Send + Sync>;
pub type HotkeyFactory =
    Arc<dyn Fn(&HotkeyConfig) -> Result<Box<dyn HotkeyDriver>, AlatusError> + Send + Sync>;

#[derive(Default)]
pub struct InitializedDrivers {
    pub battery: Option<Arc<dyn BatteryDriver>>,
    pub thermal: Option<Arc<dyn ThermalDriver>>,
    pub lighting: Option<Arc<dyn LightingDriver>>,
    pub hotkeys: Option<Box<dyn HotkeyDriver>>,
}

pub struct DriverRegistry {
    battery_factories: HashMap<String, BatteryFactory>,
    thermal_factories: HashMap<String, ThermalFactory>,
    lighting_factories: HashMap<String, LightingFactory>,
    hotkey_factories: HashMap<String, HotkeyFactory>,
}

impl Default for DriverRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl DriverRegistry {
    pub fn new() -> Self {
        Self {
            battery_factories: HashMap::new(),
            thermal_factories: HashMap::new(),
            lighting_factories: HashMap::new(),
            hotkey_factories: HashMap::new(),
        }
    }

    pub fn with_default_drivers(sysfs_root: alatus_drivers::SysfsRoot) -> Self {
        let mut registry = Self::new();
        let root = sysfs_root.clone();
        registry.register_battery("asus-sysfs-battery", move |cfg| {
            Ok(Arc::new(alatus_drivers::AsusSysfsBatteryDriver::new(
                root.clone(),
                &cfg.sysfs_path,
                cfg.supported_limits.clone(),
            )))
        });
        registry
    }

    pub fn register_battery<F>(&mut self, name: impl Into<String>, factory: F)
    where
        F: Fn(&BatteryConfig) -> Result<Arc<dyn BatteryDriver>, AlatusError> + Send + Sync + 'static,
    {
        self.battery_factories.insert(name.into(), Arc::new(factory));
    }

    pub fn register_thermal<F>(&mut self, name: impl Into<String>, factory: F)
    where
        F: Fn(&ThermalConfig) -> Result<Arc<dyn ThermalDriver>, AlatusError> + Send + Sync + 'static,
    {
        self.thermal_factories.insert(name.into(), Arc::new(factory));
    }

    pub fn register_lighting<F>(&mut self, name: impl Into<String>, factory: F)
    where
        F: Fn(&LightingConfig) -> Result<Arc<dyn LightingDriver>, AlatusError> + Send + Sync + 'static,
    {
        self.lighting_factories.insert(name.into(), Arc::new(factory));
    }

    pub fn register_hotkey<F>(&mut self, name: impl Into<String>, factory: F)
    where
        F: Fn(&HotkeyConfig) -> Result<Box<dyn HotkeyDriver>, AlatusError> + Send + Sync + 'static,
    {
        self.hotkey_factories.insert(name.into(), Arc::new(factory));
    }

    pub fn create_battery(&self, config: &BatteryConfig) -> Result<Arc<dyn BatteryDriver>, AlatusError> {
        let factory = self.battery_factories.get(&config.driver).ok_or_else(|| {
            AlatusError::Driver {
                driver: "registry",
                message: format!("Unknown battery driver: {}", config.driver),
            }
        })?;
        factory(config)
    }

    pub fn create_thermal(&self, config: &ThermalConfig) -> Result<Arc<dyn ThermalDriver>, AlatusError> {
        let factory = self.thermal_factories.get(&config.driver).ok_or_else(|| {
            AlatusError::Driver {
                driver: "registry",
                message: format!("Unknown thermal driver: {}", config.driver),
            }
        })?;
        factory(config)
    }

    pub fn create_lighting(&self, config: &LightingConfig) -> Result<Arc<dyn LightingDriver>, AlatusError> {
        let factory = self.lighting_factories.get(&config.driver).ok_or_else(|| {
            AlatusError::Driver {
                driver: "registry",
                message: format!("Unknown lighting driver: {}", config.driver),
            }
        })?;
        factory(config)
    }

    pub fn create_hotkey(&self, config: &HotkeyConfig) -> Result<Box<dyn HotkeyDriver>, AlatusError> {
        let factory = self.hotkey_factories.get(&config.driver).ok_or_else(|| {
            AlatusError::Driver {
                driver: "registry",
                message: format!("Unknown hotkey driver: {}", config.driver),
            }
        })?;
        factory(config)
    }

    /// Build all component drivers defined in the profile.
    /// Fails per component gracefully without aborting globally.
    pub fn build_from_profile(&self, profile: &Profile) -> InitializedDrivers {
        let mut drivers = InitializedDrivers::default();

        match self.create_battery(&profile.battery) {
            Ok(b) => {
                tracing::info!("Initialized battery driver: {}", b.name());
                drivers.battery = Some(b);
            }
            Err(e) => {
                tracing::warn!("Failed to initialize battery driver '{}': {}", profile.battery.driver, e);
            }
        }

        match self.create_thermal(&profile.thermal) {
            Ok(t) => {
                tracing::info!("Initialized thermal driver: {}", t.name());
                drivers.thermal = Some(t);
            }
            Err(e) => {
                tracing::warn!("Failed to initialize thermal driver '{}': {}", profile.thermal.driver, e);
            }
        }

        if let Some(ref l_config) = profile.lighting {
            match self.create_lighting(l_config) {
                Ok(l) => {
                    tracing::info!("Initialized lighting driver: {}", l.name());
                    drivers.lighting = Some(l);
                }
                Err(e) => {
                    tracing::warn!("Failed to initialize lighting driver '{}': {}", l_config.driver, e);
                }
            }
        }

        if let Some(ref h_config) = profile.hotkeys {
            match self.create_hotkey(h_config) {
                Ok(h) => {
                    tracing::info!("Initialized hotkey driver: {}", h.name());
                    drivers.hotkeys = Some(h);
                }
                Err(e) => {
                    tracing::warn!("Failed to initialize hotkey driver '{}': {}", h_config.driver, e);
                }
            }
        }

        drivers
    }
}
