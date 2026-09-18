// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::domain::{
    BrightnessPercent, ChargeThreshold, ColorRgb, RgbTimeoutPolicy, ThermalMode, TimeoutDuration,
};
use crate::hardware::capabilities::UnavailableReason;
use crate::hardware::error::DriverError;
use crate::hardware::traits::{BatteryDriver, RgbDriver, ThermalDriver};
use std::sync::{Arc, Mutex};
use zbus::blocking::Proxy;

const ASUS_DESTINATION: &str = "xyz.ljones.Asusd";
const ASUS_PATH: &str = "/xyz/ljones";
const PLATFORM_INTERFACE: &str = "xyz.ljones.Platform";
const AURA_INTERFACE: &str = "xyz.ljones.Aura";

const SUPPORTED_MODES: &[ThermalMode] = &[
    ThermalMode::Quiet,
    ThermalMode::Balanced,
    ThermalMode::Performance,
    ThermalMode::FullSpeed,
];

fn run_blocking_result<F, T>(f: F) -> Result<T, DriverError>
where
    F: FnOnce() -> Result<T, DriverError> + Send + 'static,
    T: Send + 'static,
{
    if tokio::runtime::Handle::try_current().is_ok() {
        match std::thread::spawn(f).join() {
            Ok(res) => res,
            Err(_) => Err(DriverError::Communication(
                "Blocking D-Bus thread panicked".into(),
            )),
        }
    } else {
        f()
    }
}

fn run_blocking_opt<F, T>(f: F) -> Option<T>
where
    F: FnOnce() -> Option<T> + Send + 'static,
    T: Send + 'static,
{
    if tokio::runtime::Handle::try_current().is_ok() {
        std::thread::spawn(f).join().ok().flatten()
    } else {
        f()
    }
}

/// Synchronous proxy wrapper for the `xyz.ljones.Platform` interface.
#[derive(Clone, Debug)]
pub struct AsusdPlatformProxy {
    proxy: Proxy<'static>,
}

impl AsusdPlatformProxy {
    /// Connects to the platform interface on the system bus.
    pub fn new(conn: &zbus::blocking::Connection) -> Result<Self, DriverError> {
        let conn = conn.clone();
        run_blocking_result(move || {
            let proxy = Proxy::new(&conn, ASUS_DESTINATION, ASUS_PATH, PLATFORM_INTERFACE)
                .map_err(|e| {
                    DriverError::Communication(format!("Failed to connect to Platform: {e}"))
                })?;
            Ok(Self { proxy })
        })
    }

    /// Reads the current platform thermal profile.
    pub fn platform_profile(&self) -> Result<String, DriverError> {
        let proxy = self.proxy.clone();
        run_blocking_result(move || {
            proxy
                .get_property::<String>("PlatformProfile")
                .or_else(|_| proxy.get_property::<String>("platform_profile"))
                .map_err(|e| {
                    DriverError::Communication(format!("Failed to read PlatformProfile: {e}"))
                })
        })
    }

    /// Commits a new platform thermal profile.
    pub fn set_platform_profile(&self, profile: &str) -> Result<(), DriverError> {
        let proxy = self.proxy.clone();
        let profile = profile.to_string();
        run_blocking_result(move || {
            proxy
                .set_property("PlatformProfile", &profile)
                .or_else(|_| proxy.set_property("platform_profile", &profile))
                .map_err(|e| {
                    DriverError::Communication(format!("Failed to set PlatformProfile: {e}"))
                })
        })
    }

    /// Reads the current battery charge limit threshold.
    pub fn charge_control_end_threshold(&self) -> Result<u8, DriverError> {
        let proxy = self.proxy.clone();
        run_blocking_result(move || {
            proxy
                .get_property::<u8>("ChargeControlEndThreshold")
                .or_else(|_| proxy.get_property::<u8>("charge_control_end_threshold"))
                .map_err(|e| {
                    DriverError::Communication(format!(
                        "Failed to read ChargeControlEndThreshold: {e}"
                    ))
                })
        })
    }

    /// Commits a new battery charge limit threshold.
    pub fn set_charge_control_end_threshold(&self, threshold: u8) -> Result<(), DriverError> {
        let proxy = self.proxy.clone();
        run_blocking_result(move || {
            proxy
                .set_property("ChargeControlEndThreshold", threshold)
                .or_else(|_| proxy.set_property("charge_control_end_threshold", threshold))
                .map_err(|e| {
                    DriverError::Communication(format!(
                        "Failed to set ChargeControlEndThreshold: {e}"
                    ))
                })
        })
    }
}

/// Synchronous proxy wrapper for the `xyz.ljones.Aura` interface.
#[derive(Clone, Debug)]
pub struct AsusdAuraProxy {
    proxy: Proxy<'static>,
}

impl AsusdAuraProxy {
    /// Connects to the Aura illumination interface on the system bus.
    pub fn new(conn: &zbus::blocking::Connection) -> Result<Self, DriverError> {
        let conn = conn.clone();
        run_blocking_result(move || {
            let proxy =
                Proxy::new(&conn, ASUS_DESTINATION, ASUS_PATH, AURA_INTERFACE).map_err(|e| {
                    DriverError::Communication(format!("Failed to connect to Aura: {e}"))
                })?;
            Ok(Self { proxy })
        })
    }

    /// Reads the current Aura LED animation mode.
    pub fn led_mode(&self) -> Result<String, DriverError> {
        let proxy = self.proxy.clone();
        run_blocking_result(move || {
            proxy
                .get_property::<String>("LedMode")
                .or_else(|_| proxy.get_property::<String>("led_mode"))
                .map_err(|e| DriverError::Communication(format!("Failed to read LedMode: {e}")))
        })
    }

    /// Commits a new Aura LED animation mode.
    pub fn set_led_mode(&self, mode: &str) -> Result<(), DriverError> {
        let proxy = self.proxy.clone();
        let mode = mode.to_string();
        run_blocking_result(move || {
            proxy
                .set_property("LedMode", &mode)
                .or_else(|_| proxy.set_property("led_mode", &mode))
                .map_err(|e| DriverError::Communication(format!("Failed to set LedMode: {e}")))
        })
    }

    /// Reads the current Aura backlight brightness level (0..=3).
    pub fn brightness(&self) -> Result<u8, DriverError> {
        let proxy = self.proxy.clone();
        run_blocking_result(move || {
            proxy
                .get_property::<u8>("Brightness")
                .or_else(|_| proxy.get_property::<u8>("brightness"))
                .map_err(|e| {
                    DriverError::Communication(format!("Failed to read Aura brightness: {e}"))
                })
        })
    }

    /// Commits a new Aura backlight brightness level (0..=3).
    pub fn set_brightness(&self, brightness: u8) -> Result<(), DriverError> {
        let proxy = self.proxy.clone();
        run_blocking_result(move || {
            proxy
                .set_property("Brightness", brightness)
                .or_else(|_| proxy.set_property("brightness", brightness))
                .map_err(|e| {
                    DriverError::Communication(format!("Failed to set Aura brightness: {e}"))
                })
        })
    }
}

/// In-memory state representation for headless testing and mock injection.
#[derive(Clone, Debug)]
pub struct MockAsusdState {
    pub platform_profile: String,
    pub charge_control_end_threshold: u8,
    pub led_mode: String,
    pub brightness: u8,
    pub simulate_failure: bool,
}

impl Default for MockAsusdState {
    fn default() -> Self {
        Self {
            platform_profile: "balanced".to_string(),
            charge_control_end_threshold: 80,
            led_mode: "static".to_string(),
            brightness: 3,
            simulate_failure: false,
        }
    }
}

#[derive(Clone, Debug)]
enum AsusctlBackend {
    Dbus {
        platform: AsusdPlatformProxy,
        aura: Option<AsusdAuraProxy>,
    },
    Mock {
        state: Arc<Mutex<MockAsusdState>>,
    },
}

/// Fallback hardware driver communicating with `xyz.ljones.Asusd` over system D-Bus.
#[derive(Clone, Debug)]
pub struct AsusctlProxyDriver {
    backend: AsusctlBackend,
    cached_color: ColorRgb,
    cached_brightness: BrightnessPercent,
    policy: RgbTimeoutPolicy,
    timeout: TimeoutDuration,
    sleeping: bool,
}

impl AsusctlProxyDriver {
    /// Connects to a live `asusd` instance using an existing system D-Bus connection.
    pub fn new(conn: &zbus::blocking::Connection) -> Result<Self, DriverError> {
        let platform = AsusdPlatformProxy::new(conn)?;
        let aura = AsusdAuraProxy::new(conn).ok();

        Ok(Self {
            backend: AsusctlBackend::Dbus { platform, aura },
            cached_color: ColorRgb::new(255, 255, 255),
            cached_brightness: BrightnessPercent::new(100).unwrap(),
            policy: RgbTimeoutPolicy::Always,
            timeout: TimeoutDuration::default(),
            sleeping: false,
        })
    }

    /// Probes the system D-Bus for an active `xyz.ljones.Asusd` service owner.
    pub fn probe() -> Option<Self> {
        run_blocking_opt(|| {
            let conn = zbus::blocking::Connection::system().ok()?;
            let dbus = zbus::blocking::fdo::DBusProxy::new(&conn).ok()?;
            let bus_name = ASUS_DESTINATION.try_into().ok()?;
            let has_owner = dbus.name_has_owner(bus_name).ok()?;
            if !has_owner {
                return None;
            }

            Self::new(&conn).ok()
        })
    }

    /// Constructs an in-memory mock proxy driver with default state for unit testing.
    pub fn new_mock() -> Self {
        Self::new_mock_with_state(Arc::new(Mutex::new(MockAsusdState::default())))
    }

    /// Constructs an in-memory mock proxy driver referencing a shared state instance.
    pub fn new_mock_with_state(state: Arc<Mutex<MockAsusdState>>) -> Self {
        Self {
            backend: AsusctlBackend::Mock { state },
            cached_color: ColorRgb::new(255, 255, 255),
            cached_brightness: BrightnessPercent::new(100).unwrap(),
            policy: RgbTimeoutPolicy::Always,
            timeout: TimeoutDuration::default(),
            sleeping: false,
        }
    }

    /// Returns a reference to the shared mock state if running in mock mode.
    pub fn mock_state(&self) -> Option<Arc<Mutex<MockAsusdState>>> {
        match &self.backend {
            AsusctlBackend::Mock { state } => Some(Arc::clone(state)),
            _ => None,
        }
    }

    fn percent_to_aura_level(percent: u8) -> u8 {
        match percent {
            0 => 0,
            1..=33 => 1,
            34..=66 => 2,
            _ => 3,
        }
    }

    fn aura_level_to_percent(level: u8) -> u8 {
        match level {
            0 => 0,
            1 => 33,
            2 => 66,
            _ => 100,
        }
    }
}

impl ThermalDriver for AsusctlProxyDriver {
    fn set_mode(&mut self, mode: ThermalMode) -> Result<(), DriverError> {
        let profile_str = match mode {
            ThermalMode::Quiet => "quiet",
            ThermalMode::Balanced => "balanced",
            ThermalMode::Performance => "performance",
            // Asusd does not have a separate full speed profile; map to performance
            ThermalMode::FullSpeed => "performance",
        };

        match &self.backend {
            AsusctlBackend::Dbus { platform, .. } => platform.set_platform_profile(profile_str),
            AsusctlBackend::Mock { state } => {
                let mut guard = state.lock().map_err(|_| {
                    DriverError::Unavailable(UnavailableReason::HardwareError(
                        "Mock lock poisoned".into(),
                    ))
                })?;
                if guard.simulate_failure {
                    return Err(DriverError::Communication("Simulated D-Bus failure".into()));
                }
                guard.platform_profile = profile_str.to_string();
                Ok(())
            }
        }
    }

    fn get_mode(&self) -> Result<ThermalMode, DriverError> {
        let raw = match &self.backend {
            AsusctlBackend::Dbus { platform, .. } => platform.platform_profile()?,
            AsusctlBackend::Mock { state } => {
                let guard = state.lock().map_err(|_| {
                    DriverError::Unavailable(UnavailableReason::HardwareError(
                        "Mock lock poisoned".into(),
                    ))
                })?;
                if guard.simulate_failure {
                    return Err(DriverError::Communication("Simulated D-Bus failure".into()));
                }
                guard.platform_profile.clone()
            }
        };

        match raw.trim().to_lowercase().as_str() {
            "quiet" => Ok(ThermalMode::Quiet),
            "balanced" => Ok(ThermalMode::Balanced),
            "performance" => Ok(ThermalMode::Performance),
            "custom" => Ok(ThermalMode::Performance),
            _ => Ok(ThermalMode::Balanced),
        }
    }

    fn supported_modes(&self) -> &[ThermalMode] {
        SUPPORTED_MODES
    }

    fn read_fan_speeds(&self) -> Result<Vec<u32>, DriverError> {
        Ok(Vec::new())
    }
}

impl BatteryDriver for AsusctlProxyDriver {
    fn set_charge_threshold(&mut self, threshold: ChargeThreshold) -> Result<(), DriverError> {
        match &self.backend {
            AsusctlBackend::Dbus { platform, .. } => {
                platform.set_charge_control_end_threshold(threshold.value())
            }
            AsusctlBackend::Mock { state } => {
                let mut guard = state.lock().map_err(|_| {
                    DriverError::Unavailable(UnavailableReason::HardwareError(
                        "Mock lock poisoned".into(),
                    ))
                })?;
                if guard.simulate_failure {
                    return Err(DriverError::Communication("Simulated D-Bus failure".into()));
                }
                guard.charge_control_end_threshold = threshold.value();
                Ok(())
            }
        }
    }

    fn get_charge_threshold(&self) -> Result<ChargeThreshold, DriverError> {
        let val = match &self.backend {
            AsusctlBackend::Dbus { platform, .. } => platform.charge_control_end_threshold()?,
            AsusctlBackend::Mock { state } => {
                let guard = state.lock().map_err(|_| {
                    DriverError::Unavailable(UnavailableReason::HardwareError(
                        "Mock lock poisoned".into(),
                    ))
                })?;
                if guard.simulate_failure {
                    return Err(DriverError::Communication("Simulated D-Bus failure".into()));
                }
                guard.charge_control_end_threshold
            }
        };

        ChargeThreshold::new(val).map_err(|e| DriverError::InvalidParameter(e.to_string()))
    }
}

impl RgbDriver for AsusctlProxyDriver {
    fn set_color(&mut self, color: ColorRgb) -> Result<(), DriverError> {
        self.cached_color = color;

        match &self.backend {
            AsusctlBackend::Dbus {
                aura: Some(aura), ..
            } => {
                let _ = aura.set_led_mode("static");
                Ok(())
            }
            AsusctlBackend::Dbus { aura: None, .. } => Ok(()),
            AsusctlBackend::Mock { state } => {
                let mut guard = state.lock().map_err(|_| {
                    DriverError::Unavailable(UnavailableReason::HardwareError(
                        "Mock lock poisoned".into(),
                    ))
                })?;
                if guard.simulate_failure {
                    return Err(DriverError::Communication("Simulated D-Bus failure".into()));
                }
                guard.led_mode = "static".to_string();
                Ok(())
            }
        }
    }

    fn set_brightness(&mut self, brightness: BrightnessPercent) -> Result<(), DriverError> {
        let level = Self::percent_to_aura_level(brightness.value());
        self.cached_brightness = brightness;

        match &self.backend {
            AsusctlBackend::Dbus {
                aura: Some(aura), ..
            } => aura.set_brightness(level),
            AsusctlBackend::Dbus { aura: None, .. } => Ok(()),
            AsusctlBackend::Mock { state } => {
                let mut guard = state.lock().map_err(|_| {
                    DriverError::Unavailable(UnavailableReason::HardwareError(
                        "Mock lock poisoned".into(),
                    ))
                })?;
                if guard.simulate_failure {
                    return Err(DriverError::Communication("Simulated D-Bus failure".into()));
                }
                guard.brightness = level;
                Ok(())
            }
        }
    }

    fn get_brightness(&self) -> Result<BrightnessPercent, DriverError> {
        let level = match &self.backend {
            AsusctlBackend::Dbus {
                aura: Some(aura), ..
            } => aura.brightness()?,
            AsusctlBackend::Dbus { aura: None, .. } => {
                return Ok(self.cached_brightness);
            }
            AsusctlBackend::Mock { state } => {
                let guard = state.lock().map_err(|_| {
                    DriverError::Unavailable(UnavailableReason::HardwareError(
                        "Mock lock poisoned".into(),
                    ))
                })?;
                if guard.simulate_failure {
                    return Err(DriverError::Communication("Simulated D-Bus failure".into()));
                }
                guard.brightness
            }
        };

        let pct = Self::aura_level_to_percent(level);
        BrightnessPercent::new(pct).map_err(|e| DriverError::InvalidParameter(e.to_string()))
    }

    fn set_timeout(
        &mut self,
        policy: RgbTimeoutPolicy,
        duration: TimeoutDuration,
    ) -> Result<(), DriverError> {
        self.policy = policy;
        self.timeout = duration;
        Ok(())
    }

    fn turn_off(&mut self) -> Result<(), DriverError> {
        self.sleeping = true;
        match &self.backend {
            AsusctlBackend::Dbus {
                aura: Some(aura), ..
            } => aura.set_brightness(0),
            AsusctlBackend::Dbus { aura: None, .. } => Ok(()),
            AsusctlBackend::Mock { state } => {
                let mut guard = state.lock().map_err(|_| {
                    DriverError::Unavailable(UnavailableReason::HardwareError(
                        "Mock lock poisoned".into(),
                    ))
                })?;
                if guard.simulate_failure {
                    return Err(DriverError::Communication("Simulated D-Bus failure".into()));
                }
                guard.brightness = 0;
                Ok(())
            }
        }
    }

    fn wake(&mut self) -> Result<(), DriverError> {
        self.sleeping = false;
        let level = Self::percent_to_aura_level(self.cached_brightness.value());
        match &self.backend {
            AsusctlBackend::Dbus {
                aura: Some(aura), ..
            } => aura.set_brightness(level),
            AsusctlBackend::Dbus { aura: None, .. } => Ok(()),
            AsusctlBackend::Mock { state } => {
                let mut guard = state.lock().map_err(|_| {
                    DriverError::Unavailable(UnavailableReason::HardwareError(
                        "Mock lock poisoned".into(),
                    ))
                })?;
                if guard.simulate_failure {
                    return Err(DriverError::Communication("Simulated D-Bus failure".into()));
                }
                guard.brightness = level;
                Ok(())
            }
        }
    }

    fn is_sleeping(&self) -> bool {
        self.sleeping
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_proxy_thermal_lifecycle() {
        let mut driver = AsusctlProxyDriver::new_mock();
        assert_eq!(driver.get_mode().unwrap(), ThermalMode::Balanced);

        driver.set_mode(ThermalMode::Quiet).unwrap();
        assert_eq!(driver.get_mode().unwrap(), ThermalMode::Quiet);

        driver.set_mode(ThermalMode::Performance).unwrap();
        assert_eq!(driver.get_mode().unwrap(), ThermalMode::Performance);

        // FullSpeed maps to performance in asusd platform
        driver.set_mode(ThermalMode::FullSpeed).unwrap();
        assert_eq!(driver.get_mode().unwrap(), ThermalMode::Performance);

        assert_eq!(driver.supported_modes().len(), 4);
        assert!(driver.read_fan_speeds().unwrap().is_empty());
    }

    #[test]
    fn test_mock_proxy_battery_lifecycle() {
        let mut driver = AsusctlProxyDriver::new_mock();
        assert_eq!(driver.get_charge_threshold().unwrap().value(), 80);

        driver
            .set_charge_threshold(ChargeThreshold::new(60).unwrap())
            .unwrap();
        assert_eq!(driver.get_charge_threshold().unwrap().value(), 60);

        let state = driver.mock_state().unwrap();
        assert_eq!(state.lock().unwrap().charge_control_end_threshold, 60);
    }

    #[test]
    fn test_mock_proxy_rgb_lifecycle() {
        let mut driver = AsusctlProxyDriver::new_mock();
        assert_eq!(driver.get_brightness().unwrap().value(), 100);

        // Map 50% to aura level 2 (which maps to 66%)
        driver
            .set_brightness(BrightnessPercent::new(50).unwrap())
            .unwrap();
        assert_eq!(driver.get_brightness().unwrap().value(), 66);

        driver.turn_off().unwrap();
        assert!(driver.is_sleeping());
        assert_eq!(driver.get_brightness().unwrap().value(), 0);

        driver.wake().unwrap();
        assert!(!driver.is_sleeping());
        assert_eq!(driver.get_brightness().unwrap().value(), 66);
    }

    #[test]
    fn test_mock_proxy_simulated_failure() {
        let mut driver = AsusctlProxyDriver::new_mock();
        let state = driver.mock_state().unwrap();
        state.lock().unwrap().simulate_failure = true;

        assert!(driver.set_mode(ThermalMode::Quiet).is_err());
        assert!(driver.get_mode().is_err());
        assert!(
            driver
                .set_charge_threshold(ChargeThreshold::new(80).unwrap())
                .is_err()
        );
        assert!(driver.get_charge_threshold().is_err());
        assert!(
            driver
                .set_brightness(BrightnessPercent::new(50).unwrap())
                .is_err()
        );
    }
}
