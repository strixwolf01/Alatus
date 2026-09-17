// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! In-memory mock hardware drivers with programmable error injection for headless testing.

use crate::domain::{
    BrightnessPercent, ChargeThreshold, ColorRgb, RgbTimeoutPolicy, ThermalMode, TimeoutDuration,
};
use crate::hardware::error::DriverError;
use crate::hardware::traits::{BatteryDriver, DisplayDriver, RgbDriver, ThermalDriver};

/// In-memory mock driver for RGB keyboard illumination with failure simulation.
#[derive(Debug, Clone)]
pub struct MockRgbDriver {
    pub color: ColorRgb,
    pub brightness: BrightnessPercent,
    pub policy: RgbTimeoutPolicy,
    pub timeout: TimeoutDuration,
    pub sleeping: bool,
    pub simulate_failure: bool,
}

impl Default for MockRgbDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl MockRgbDriver {
    pub fn new() -> Self {
        Self {
            color: ColorRgb::new(255, 255, 255),
            brightness: BrightnessPercent::new(100).unwrap(),
            policy: RgbTimeoutPolicy::Always,
            timeout: TimeoutDuration::default(),
            sleeping: false,
            simulate_failure: false,
        }
    }

    pub fn with_simulate_failure(mut self, fail: bool) -> Self {
        self.simulate_failure = fail;
        self
    }
}

impl RgbDriver for MockRgbDriver {
    fn set_color(&mut self, color: ColorRgb) -> Result<(), DriverError> {
        if self.simulate_failure {
            return Err(DriverError::Communication("Simulated RGB failure".into()));
        }
        self.color = color;
        Ok(())
    }

    fn set_brightness(&mut self, brightness: BrightnessPercent) -> Result<(), DriverError> {
        if self.simulate_failure {
            return Err(DriverError::Communication("Simulated RGB failure".into()));
        }
        self.brightness = brightness;
        Ok(())
    }

    fn get_brightness(&self) -> Result<BrightnessPercent, DriverError> {
        if self.simulate_failure {
            return Err(DriverError::Communication("Simulated RGB failure".into()));
        }
        Ok(self.brightness)
    }

    fn set_timeout(
        &mut self,
        policy: RgbTimeoutPolicy,
        duration: TimeoutDuration,
    ) -> Result<(), DriverError> {
        if self.simulate_failure {
            return Err(DriverError::Communication("Simulated RGB failure".into()));
        }
        self.policy = policy;
        self.timeout = duration;
        Ok(())
    }

    fn turn_off(&mut self) -> Result<(), DriverError> {
        if self.simulate_failure {
            return Err(DriverError::Communication("Simulated RGB failure".into()));
        }
        self.sleeping = true;
        Ok(())
    }

    fn wake(&mut self) -> Result<(), DriverError> {
        if self.simulate_failure {
            return Err(DriverError::Communication("Simulated RGB failure".into()));
        }
        self.sleeping = false;
        Ok(())
    }

    fn is_sleeping(&self) -> bool {
        self.sleeping
    }
}

/// In-memory mock driver for thermal profiles and fan speed telemetry.
#[derive(Debug, Clone)]
pub struct MockThermalDriver {
    pub current_mode: ThermalMode,
    pub supported: Vec<ThermalMode>,
    pub fan_speeds: Vec<u32>,
    pub simulate_failure: bool,
}

impl Default for MockThermalDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl MockThermalDriver {
    pub fn new() -> Self {
        Self {
            current_mode: ThermalMode::Balanced,
            supported: vec![
                ThermalMode::Quiet,
                ThermalMode::Balanced,
                ThermalMode::Performance,
                ThermalMode::FullSpeed,
            ],
            fan_speeds: vec![2400, 2600],
            simulate_failure: false,
        }
    }

    pub fn with_simulate_failure(mut self, fail: bool) -> Self {
        self.simulate_failure = fail;
        self
    }
}

impl ThermalDriver for MockThermalDriver {
    fn set_mode(&mut self, mode: ThermalMode) -> Result<(), DriverError> {
        if self.simulate_failure {
            return Err(DriverError::Communication(
                "Simulated thermal failure".into(),
            ));
        }
        if self.supported.contains(&mode) {
            self.current_mode = mode;
            Ok(())
        } else {
            Err(DriverError::Unsupported(format!("Mode {mode} unsupported")))
        }
    }

    fn get_mode(&self) -> Result<ThermalMode, DriverError> {
        if self.simulate_failure {
            return Err(DriverError::Communication(
                "Simulated thermal failure".into(),
            ));
        }
        Ok(self.current_mode)
    }

    fn supported_modes(&self) -> &[ThermalMode] {
        &self.supported
    }

    fn read_fan_speeds(&self) -> Result<Vec<u32>, DriverError> {
        if self.simulate_failure {
            return Err(DriverError::Communication(
                "Simulated thermal failure".into(),
            ));
        }
        Ok(self.fan_speeds.clone())
    }
}

/// In-memory mock driver for battery charging threshold limits.
#[derive(Debug, Clone)]
pub struct MockBatteryDriver {
    pub threshold: ChargeThreshold,
    pub simulate_failure: bool,
}

impl Default for MockBatteryDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl MockBatteryDriver {
    pub fn new() -> Self {
        Self {
            threshold: ChargeThreshold::default(),
            simulate_failure: false,
        }
    }

    pub fn with_simulate_failure(mut self, fail: bool) -> Self {
        self.simulate_failure = fail;
        self
    }
}

impl BatteryDriver for MockBatteryDriver {
    fn set_charge_threshold(&mut self, threshold: ChargeThreshold) -> Result<(), DriverError> {
        if self.simulate_failure {
            return Err(DriverError::Communication(
                "Simulated battery failure".into(),
            ));
        }
        self.threshold = threshold;
        Ok(())
    }

    fn get_charge_threshold(&self) -> Result<ChargeThreshold, DriverError> {
        if self.simulate_failure {
            return Err(DriverError::Communication(
                "Simulated battery failure".into(),
            ));
        }
        Ok(self.threshold)
    }
}

/// In-memory mock driver for display dimming and panel refresh rates.
#[derive(Debug, Clone)]
pub struct MockDisplayDriver {
    pub dimming_factor: f32,
    pub refresh_rate: u32,
    pub supported_rates: Vec<u32>,
    pub simulate_failure: bool,
}

impl Default for MockDisplayDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl MockDisplayDriver {
    pub fn new() -> Self {
        Self {
            dimming_factor: 1.0,
            refresh_rate: 120,
            supported_rates: vec![60, 120],
            simulate_failure: false,
        }
    }

    pub fn with_simulate_failure(mut self, fail: bool) -> Self {
        self.simulate_failure = fail;
        self
    }
}

impl DisplayDriver for MockDisplayDriver {
    fn set_flicker_free_dimming(&mut self, factor: f32) -> Result<(), DriverError> {
        if self.simulate_failure {
            return Err(DriverError::Communication(
                "Simulated display failure".into(),
            ));
        }
        self.dimming_factor = factor.clamp(0.0, 1.0);
        Ok(())
    }

    fn set_refresh_rate(&mut self, rate: u32) -> Result<(), DriverError> {
        if self.simulate_failure {
            return Err(DriverError::Communication(
                "Simulated display failure".into(),
            ));
        }
        if self.supported_rates.contains(&rate) {
            self.refresh_rate = rate;
            Ok(())
        } else {
            Err(DriverError::Unsupported(format!("Rate {rate} unsupported")))
        }
    }

    fn supported_refresh_rates(&self) -> &[u32] {
        &self.supported_rates
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_rgb_driver_state_and_failure() {
        let mut driver = MockRgbDriver::new();
        driver.set_color(ColorRgb::new(255, 0, 0)).unwrap();
        assert_eq!(driver.color, ColorRgb::new(255, 0, 0));

        driver.turn_off().unwrap();
        assert!(driver.is_sleeping());
        driver.wake().unwrap();
        assert!(!driver.is_sleeping());

        // Error injection
        let mut failing = MockRgbDriver::new().with_simulate_failure(true);
        assert!(failing.set_color(ColorRgb::new(0, 255, 0)).is_err());
        assert!(failing.get_brightness().is_err());
        assert!(failing.turn_off().is_err());
    }

    #[test]
    fn test_mock_thermal_driver_state_and_failure() {
        let mut driver = MockThermalDriver::new();
        assert_eq!(driver.get_mode().unwrap(), ThermalMode::Balanced);
        driver.set_mode(ThermalMode::Performance).unwrap();
        assert_eq!(driver.get_mode().unwrap(), ThermalMode::Performance);
        assert_eq!(driver.read_fan_speeds().unwrap(), vec![2400, 2600]);

        // Error injection
        let mut failing = MockThermalDriver::new().with_simulate_failure(true);
        assert!(failing.set_mode(ThermalMode::Quiet).is_err());
        assert!(failing.read_fan_speeds().is_err());
    }

    #[test]
    fn test_mock_battery_driver_state_and_failure() {
        let mut driver = MockBatteryDriver::new();
        assert_eq!(driver.get_charge_threshold().unwrap().value(), 80);
        driver
            .set_charge_threshold(ChargeThreshold::new(60).unwrap())
            .unwrap();
        assert_eq!(driver.get_charge_threshold().unwrap().value(), 60);

        // Error injection
        let mut failing = MockBatteryDriver::new().with_simulate_failure(true);
        assert!(failing.get_charge_threshold().is_err());
        assert!(
            failing
                .set_charge_threshold(ChargeThreshold::new(80).unwrap())
                .is_err()
        );
    }

    #[test]
    fn test_mock_display_driver_state_and_failure() {
        let mut driver = MockDisplayDriver::new();
        assert_eq!(driver.dimming_factor, 1.0);
        assert_eq!(driver.refresh_rate, 120);

        driver.set_flicker_free_dimming(0.5).unwrap();
        assert_eq!(driver.dimming_factor, 0.5);

        driver.set_refresh_rate(60).unwrap();
        assert_eq!(driver.refresh_rate, 60);
        assert!(driver.set_refresh_rate(240).is_err());

        // Error injection
        let mut failing = MockDisplayDriver::new().with_simulate_failure(true);
        assert!(failing.set_flicker_free_dimming(0.8).is_err());
        assert!(failing.set_refresh_rate(60).is_err());
    }
}
