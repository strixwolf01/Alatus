use alatus_core::{
    battery::{BatteryDriver, BatteryInfo, BatteryStatus},
    capabilities::BatteryCapabilities,
    error::AlatusError,
};
use alatus_profile::dmi::S5506MA_DEFAULT_PROFILE;
use alatus_profile::model::Profile;
use alatus_registry::DriverRegistry;
use async_trait::async_trait;
use std::sync::Arc;

struct MockBatteryDriver;

#[async_trait]
impl BatteryDriver for MockBatteryDriver {
    fn name(&self) -> &'static str {
        "mock-battery"
    }

    fn capabilities(&self) -> BatteryCapabilities {
        BatteryCapabilities::CHARGE_LIMIT_CONFIGURABLE
    }

    async fn get_info(&self) -> Result<BatteryInfo, AlatusError> {
        Ok(BatteryInfo {
            percentage: 80,
            status: BatteryStatus::Charging,
            charge_limit: Some(80),
            health_percentage: Some(99),
            power_now_microwatts: Some(15000000),
        })
    }

    async fn get_charge_limit(&self) -> Result<u8, AlatusError> {
        Ok(80)
    }

    async fn set_charge_limit(&self, _limit: u8) -> Result<(), AlatusError> {
        Ok(())
    }

    async fn get_health_percentage(&self) -> Result<u8, AlatusError> {
        Ok(99)
    }

    async fn is_charging(&self) -> Result<bool, AlatusError> {
        Ok(true)
    }
}

#[test]
fn test_registry_registration_and_fallback() {
    let mut registry = DriverRegistry::new();
    registry.register_battery("asus-sysfs-battery", |_cfg| Ok(Arc::new(MockBatteryDriver)));

    let profile = Profile::parse_toml(S5506MA_DEFAULT_PROFILE).unwrap();
    let drivers = registry.build_from_profile(&profile);

    // Battery should succeed
    assert!(drivers.battery.is_some());
    assert_eq!(drivers.battery.unwrap().name(), "mock-battery");

    // Thermal was not registered, so it fails gracefully without panic
    assert!(drivers.thermal.is_none());
}
