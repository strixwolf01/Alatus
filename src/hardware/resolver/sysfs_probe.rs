// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Kernel and sysfs interface probing routines for hardware capabilities.

use crate::hardware::capabilities::{
    BatteryCapabilityDetails, CapabilityState, DisplayCapabilityDetails, RgbCapabilityDetails,
    ThermalCapabilityDetails, UnavailableReason,
};
use crate::hardware::drivers::{
    AsusWmiDriver, AsusctlProxyDriver, Ite5570Driver, OledDisplayDriver, SysfsBatteryDriver,
};
use crate::hardware::profile::model::{
    BatteryProfileConfig, DisplayProfileConfig, RgbProfileConfig, ThermalProfileConfig,
};
use crate::hardware::traits::{BatteryDriver, DisplayDriver, RgbDriver, ThermalDriver};
use std::path::Path;

/// Probes RGB backlight capabilities and instantiates the driver.
pub fn probe_rgb(
    rgb_cfg: &RgbProfileConfig,
    proxy_cache: &mut Option<AsusctlProxyDriver>,
) -> (CapabilityState<RgbCapabilityDetails>, Option<Box<dyn RgbDriver>>) {
    let is_ite = rgb_cfg.driver == "ite5570";
    let has_hid = is_ite && crate::services::alatus_rgb_wrapper::discover().is_ok();
    let has_sysfs = is_ite
        && Path::new(crate::services::rgb::SYS_KBD_BACKLIGHT)
            .join("brightness")
            .exists();

    if has_hid || has_sysfs {
        (
            CapabilityState::Supported(RgbCapabilityDetails {
                max_brightness: 100,
                supports_custom_color: true,
                supports_inactivity_timeout: rgb_cfg.supports_timeout,
                supported_zones: vec!["keyboard".to_string()],
            }),
            Some(Box::new(Ite5570Driver::new())),
        )
    } else if let Some(proxy) = get_or_probe_proxy(proxy_cache) {
        (
            CapabilityState::Supported(RgbCapabilityDetails {
                max_brightness: 100,
                supports_custom_color: true,
                supports_inactivity_timeout: rgb_cfg.supports_timeout,
                supported_zones: vec!["keyboard".to_string()],
            }),
            Some(Box::new(proxy)),
        )
    } else {
        (
            CapabilityState::Unavailable(UnavailableReason::KernelInterfaceMissing),
            None,
        )
    }
}

/// Probes thermal management capabilities and instantiates the driver.
pub fn probe_thermal(
    _thermal_cfg: &ThermalProfileConfig,
    proxy_cache: &mut Option<AsusctlProxyDriver>,
) -> (CapabilityState<ThermalCapabilityDetails>, Option<Box<dyn ThermalDriver>>) {
    let driver = AsusWmiDriver::new();
    if driver.get_mode().is_ok() {
        (
            CapabilityState::Supported(ThermalCapabilityDetails {
                supported_modes: driver.supported_modes().to_vec(),
                fan_count: 2,
                supports_fan_telemetry: true,
            }),
            Some(Box::new(driver)),
        )
    } else if let Some(proxy) = get_or_probe_proxy(proxy_cache) {
        (
            CapabilityState::Supported(ThermalCapabilityDetails {
                supported_modes: proxy.supported_modes().to_vec(),
                fan_count: 2,
                supports_fan_telemetry: false,
            }),
            Some(Box::new(proxy)),
        )
    } else {
        (
            CapabilityState::Unavailable(UnavailableReason::KernelInterfaceMissing),
            None,
        )
    }
}

/// Probes battery charging limit capabilities and instantiates the driver.
pub fn probe_battery(
    bat_cfg: &BatteryProfileConfig,
    proxy_cache: &mut Option<AsusctlProxyDriver>,
) -> (CapabilityState<BatteryCapabilityDetails>, Option<Box<dyn BatteryDriver>>) {
    let driver = match &bat_cfg.sysfs_path {
        Some(custom_path) => SysfsBatteryDriver::with_path(std::path::PathBuf::from(custom_path)),
        None => SysfsBatteryDriver::new(),
    };

    if driver.get_charge_threshold().is_ok() {
        (
            CapabilityState::Supported(BatteryCapabilityDetails {
                min_threshold: 50,
                max_threshold: 100,
                supports_charge_threshold: true,
            }),
            Some(Box::new(driver)),
        )
    } else if let Some(proxy) = get_or_probe_proxy(proxy_cache) {
        (
            CapabilityState::Supported(BatteryCapabilityDetails {
                min_threshold: 20,
                max_threshold: 100,
                supports_charge_threshold: true,
            }),
            Some(Box::new(proxy)),
        )
    } else {
        (
            CapabilityState::Unavailable(UnavailableReason::KernelInterfaceMissing),
            None,
        )
    }
}

/// Probes display capabilities and instantiates the driver.
pub fn probe_display(
    disp_cfg: &DisplayProfileConfig,
) -> (CapabilityState<DisplayCapabilityDetails>, Option<Box<dyn DisplayDriver>>) {
    let driver = OledDisplayDriver::with_refresh_rates(disp_cfg.refresh_rates.clone());
    (
        CapabilityState::Supported(DisplayCapabilityDetails {
            supports_flicker_free_dimming: disp_cfg.supports_flicker_free,
            supported_refresh_rates: disp_cfg.refresh_rates.clone(),
        }),
        Some(Box::new(driver)),
    )
}

fn get_or_probe_proxy(cache: &mut Option<AsusctlProxyDriver>) -> Option<AsusctlProxyDriver> {
    if cache.is_none() {
        *cache = AsusctlProxyDriver::probe();
    }
    cache.clone()
}
