// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Driver factory for assembling DeviceContext from profiles and probe results.

use super::sysfs_probe::{probe_battery, probe_display, probe_rgb, probe_thermal};
use crate::hardware::capabilities::{CapabilityState, SystemCapabilities};
use crate::hardware::context::DeviceContext;
use crate::hardware::drivers::AsusctlProxyDriver;
use crate::hardware::profile::DeviceProfile;

pub struct DriverFactory;

impl DriverFactory {
    /// Builds a populated DeviceContext according to the declared profile capabilities.
    pub fn build_context(
        profile: DeviceProfile,
        fallback_proxy: Option<AsusctlProxyDriver>,
    ) -> DeviceContext {
        let mut capabilities = SystemCapabilities {
            schema_version: 1,
            rgb: CapabilityState::Unsupported,
            thermal: CapabilityState::Unsupported,
            battery: CapabilityState::Unsupported,
            display: CapabilityState::Unsupported,
        };

        let mut proxy_cache = fallback_proxy;

        // 1. RGB Subsystem
        let rgb = match &profile.capabilities.rgb {
            None => {
                capabilities.rgb = CapabilityState::Unsupported;
                None
            }
            Some(cfg) => {
                let (cap, driver) = probe_rgb(cfg, &mut proxy_cache);
                capabilities.rgb = cap;
                driver
            }
        };

        // 2. Thermal Subsystem
        let thermal = match &profile.capabilities.thermal {
            None => {
                capabilities.thermal = CapabilityState::Unsupported;
                None
            }
            Some(cfg) => {
                let (cap, driver) = probe_thermal(cfg, &mut proxy_cache);
                capabilities.thermal = cap;
                driver
            }
        };

        // 3. Battery Subsystem
        let battery = match &profile.capabilities.battery {
            None => {
                capabilities.battery = CapabilityState::Unsupported;
                None
            }
            Some(cfg) => {
                let (cap, driver) = probe_battery(cfg, &mut proxy_cache);
                capabilities.battery = cap;
                driver
            }
        };

        // 4. Display Subsystem
        let display = match &profile.capabilities.display {
            None => {
                capabilities.display = CapabilityState::Unsupported;
                None
            }
            Some(cfg) => {
                let (cap, driver) = probe_display(cfg);
                capabilities.display = cap;
                driver
            }
        };

        DeviceContext::with_drivers(profile, capabilities, rgb, thermal, battery, display)
    }

    /// Re-probes the RGB backlighting controller while preserving cached brightness.
    pub fn re_enumerate_rgb(ctx: &mut DeviceContext) {
        let cached_brightness = ctx.rgb.as_ref().and_then(|r| r.get_brightness().ok());

        if let Some(rgb_cfg) = &ctx.profile.capabilities.rgb {
            let mut proxy_cache = None;
            let (cap, mut driver) = probe_rgb(rgb_cfg, &mut proxy_cache);
            ctx.capabilities.rgb = cap;

            if let Some(ref mut d) = driver
                && let Some(b) = cached_brightness
            {
                let _ = d.set_brightness(b);
            }

            ctx.rgb = driver;
        } else {
            ctx.capabilities.rgb = CapabilityState::Unsupported;
            ctx.rgb = None;
        }
    }

    /// Re-probes all platform hardware drivers and refreshes capability state.
    pub fn re_enumerate(ctx: &mut DeviceContext) {
        let refreshed = Self::build_context(ctx.profile.clone(), None);
        ctx.capabilities = refreshed.capabilities;
        ctx.rgb = refreshed.rgb;
        ctx.thermal = refreshed.thermal;
        ctx.battery = refreshed.battery;
        ctx.display = refreshed.display;
    }
}
