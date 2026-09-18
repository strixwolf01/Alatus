// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::domain::{
    BrightnessPercent, ChargeThreshold, ColorRgb, RgbTimeoutPolicy, ThermalMode, TimeoutDuration,
};
use crate::hardware::error::DriverError;

/// Low-level interface for keyboard backlighting hardware controllers (e.g. ITE5570 HID).
pub trait RgbDriver: Send + Sync {
    /// Sets the static illumination color across keyboard backlighting zones.
    fn set_color(&mut self, color: ColorRgb) -> Result<(), DriverError>;

    /// Sets the hardware backlighting brightness percentage.
    fn set_brightness(&mut self, brightness: BrightnessPercent) -> Result<(), DriverError>;

    /// Queries the current hardware brightness percentage.
    fn get_brightness(&self) -> Result<BrightnessPercent, DriverError>;

    /// Configures the inactivity sleep policy and duration timer.
    fn set_timeout(
        &mut self,
        policy: RgbTimeoutPolicy,
        duration: TimeoutDuration,
    ) -> Result<(), DriverError>;

    /// Immediately turns off illumination (sleep state).
    fn turn_off(&mut self) -> Result<(), DriverError>;

    /// Wakes illumination and restores cached brightness.
    fn wake(&mut self) -> Result<(), DriverError>;

    /// Returns `true` if illumination is currently powered off/sleeping.
    fn is_sleeping(&self) -> bool;
}

/// Low-level interface for ASUS platform thermal curves and fan controllers (e.g. WMI / DebugFS).
pub trait ThermalDriver: Send + Sync {
    /// Applies a platform firmware thermal profile.
    fn set_mode(&mut self, mode: ThermalMode) -> Result<(), DriverError>;

    /// Reads the active firmware thermal profile.
    fn get_mode(&self) -> Result<ThermalMode, DriverError>;

    /// Returns the list of firmware profiles supported by this driver.
    fn supported_modes(&self) -> &[ThermalMode];

    /// Reads real-time tachometer fan speeds in RPM across platform cooling fans.
    fn read_fan_speeds(&self) -> Result<Vec<u32>, DriverError>;
}

/// Low-level interface for battery charging thresholds (e.g. `charge_control_end_threshold`).
pub trait BatteryDriver: Send + Sync {
    /// Commits a new upper charge threshold limit to the battery controller.
    fn set_charge_threshold(&mut self, threshold: ChargeThreshold) -> Result<(), DriverError>;

    /// Reads the current charge threshold configured in the battery controller.
    fn get_charge_threshold(&self) -> Result<ChargeThreshold, DriverError>;
}

/// Low-level interface for display panel management (e.g. OLED DC dimming & refresh rate).
pub trait DisplayDriver: Send + Sync {
    /// Sets the software / driver OLED flicker-free dimming luminance factor.
    fn set_flicker_free_dimming(&mut self, factor: f32) -> Result<(), DriverError>;

    /// Configures the display panel vertical refresh rate in Hertz.
    fn set_refresh_rate(&mut self, rate: u32) -> Result<(), DriverError>;

    /// Returns the list of vertical refresh rates supported by the panel.
    fn supported_refresh_rates(&self) -> &[u32];
}

/// Low-level interface for ASUS platform power limits (PPT / Boost) and platform switches.
pub trait PlatformPowerDriver: Send + Sync {
    /// Reads an armoury attribute by name.
    fn get_attribute(
        &self,
        name: &str,
    ) -> Result<crate::hardware::drivers::platform::ArmouryAttribute, DriverError>;

    /// Commits a new value for an attribute with hardware safety clamping.
    fn set_attribute(&mut self, name: &str, value: u32) -> Result<(), DriverError>;

    /// Lists all discovered attribute names.
    fn list_attributes(&self) -> Vec<String>;

    /// Reads GPU MUX mode (0 = Discrete, 1 = Optimus/Hybrid).
    fn get_gpu_mux_mode(&self) -> Result<u32, DriverError>;

    /// Commits GPU MUX mode (0 = Discrete, 1 = Optimus/Hybrid).
    fn set_gpu_mux_mode(&mut self, mode: u32) -> Result<(), DriverError>;

    /// Reads panel overdrive status.
    fn get_panel_od(&self) -> Result<bool, DriverError>;

    /// Commits panel overdrive status.
    fn set_panel_od(&mut self, enabled: bool) -> Result<(), DriverError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockRgbDriver {
        color: ColorRgb,
        brightness: BrightnessPercent,
        policy: RgbTimeoutPolicy,
        timeout: TimeoutDuration,
        sleeping: bool,
    }

    impl RgbDriver for MockRgbDriver {
        fn set_color(&mut self, color: ColorRgb) -> Result<(), DriverError> {
            self.color = color;
            Ok(())
        }

        fn set_brightness(&mut self, brightness: BrightnessPercent) -> Result<(), DriverError> {
            self.brightness = brightness;
            Ok(())
        }

        fn get_brightness(&self) -> Result<BrightnessPercent, DriverError> {
            Ok(self.brightness)
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
            Ok(())
        }

        fn wake(&mut self) -> Result<(), DriverError> {
            self.sleeping = false;
            Ok(())
        }

        fn is_sleeping(&self) -> bool {
            self.sleeping
        }
    }

    #[test]
    fn test_mock_rgb_driver_trait() {
        let mut driver = MockRgbDriver {
            color: ColorRgb::new(0, 0, 0),
            brightness: BrightnessPercent::new(50).unwrap(),
            policy: RgbTimeoutPolicy::Never,
            timeout: TimeoutDuration::from_secs(30).unwrap(),
            sleeping: false,
        };

        driver.set_color(ColorRgb::new(255, 0, 128)).unwrap();
        assert_eq!(driver.color, ColorRgb::new(255, 0, 128));

        driver
            .set_brightness(BrightnessPercent::new(80).unwrap())
            .unwrap();
        assert_eq!(driver.get_brightness().unwrap().value(), 80);

        driver.turn_off().unwrap();
        assert!(driver.is_sleeping());
        driver.wake().unwrap();
        assert!(!driver.is_sleeping());
    }

    struct MockThermalDriver {
        current: ThermalMode,
        modes: Vec<ThermalMode>,
        fans: Vec<u32>,
    }

    impl ThermalDriver for MockThermalDriver {
        fn set_mode(&mut self, mode: ThermalMode) -> Result<(), DriverError> {
            if self.modes.contains(&mode) {
                self.current = mode;
                Ok(())
            } else {
                Err(DriverError::Unsupported(format!(
                    "Mode {mode} not supported"
                )))
            }
        }

        fn get_mode(&self) -> Result<ThermalMode, DriverError> {
            Ok(self.current)
        }

        fn supported_modes(&self) -> &[ThermalMode] {
            &self.modes
        }

        fn read_fan_speeds(&self) -> Result<Vec<u32>, DriverError> {
            Ok(self.fans.clone())
        }
    }

    #[test]
    fn test_mock_thermal_driver_trait() {
        let mut driver = MockThermalDriver {
            current: ThermalMode::Balanced,
            modes: vec![
                ThermalMode::Quiet,
                ThermalMode::Balanced,
                ThermalMode::Performance,
            ],
            fans: vec![2400, 2600],
        };

        assert_eq!(driver.get_mode().unwrap(), ThermalMode::Balanced);
        driver.set_mode(ThermalMode::Performance).unwrap();
        assert_eq!(driver.get_mode().unwrap(), ThermalMode::Performance);

        assert!(driver.set_mode(ThermalMode::FullSpeed).is_err());
        assert_eq!(driver.read_fan_speeds().unwrap(), vec![2400, 2600]);
    }

    struct MockBatteryDriver {
        threshold: ChargeThreshold,
    }

    impl BatteryDriver for MockBatteryDriver {
        fn set_charge_threshold(&mut self, threshold: ChargeThreshold) -> Result<(), DriverError> {
            self.threshold = threshold;
            Ok(())
        }

        fn get_charge_threshold(&self) -> Result<ChargeThreshold, DriverError> {
            Ok(self.threshold)
        }
    }

    #[test]
    fn test_mock_battery_driver_trait() {
        let mut driver = MockBatteryDriver {
            threshold: ChargeThreshold::default(),
        };

        assert_eq!(driver.get_charge_threshold().unwrap().value(), 80);
        driver
            .set_charge_threshold(ChargeThreshold::new(60).unwrap())
            .unwrap();
        assert_eq!(driver.get_charge_threshold().unwrap().value(), 60);
    }
}
