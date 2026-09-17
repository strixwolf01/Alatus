// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::hardware::error::DriverError;
use crate::hardware::traits::DisplayDriver;

const DEFAULT_SUPPORTED_REFRESH_RATES: &[u32] = &[60, 120];

/// Display driver handling OLED flicker-free software dimming and panel refresh rate switching.
#[derive(Debug)]
pub struct OledDisplayDriver {
    dimming_factor: f32,
    active_refresh_rate: u32,
    supported_rates: Vec<u32>,
}

impl Default for OledDisplayDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl OledDisplayDriver {
    /// Constructs driver with standard ASUS OLED panel defaults.
    pub fn new() -> Self {
        Self {
            dimming_factor: 1.0,
            active_refresh_rate: 120,
            supported_rates: DEFAULT_SUPPORTED_REFRESH_RATES.to_vec(),
        }
    }

    /// Constructs driver with custom supported refresh rates.
    pub fn with_refresh_rates(rates: Vec<u32>) -> Self {
        let initial_rate = rates.first().copied().unwrap_or(60);
        Self {
            dimming_factor: 1.0,
            active_refresh_rate: initial_rate,
            supported_rates: rates,
        }
    }

    /// Returns the current active dimming factor (0.0 to 1.0).
    pub fn dimming_factor(&self) -> f32 {
        self.dimming_factor
    }

    /// Returns the current active refresh rate in Hertz.
    pub fn active_refresh_rate(&self) -> u32 {
        self.active_refresh_rate
    }
}

impl DisplayDriver for OledDisplayDriver {
    fn set_flicker_free_dimming(&mut self, factor: f32) -> Result<(), DriverError> {
        let clamped = factor.clamp(0.0, 1.0);
        self.dimming_factor = clamped;
        Ok(())
    }

    fn set_refresh_rate(&mut self, rate: u32) -> Result<(), DriverError> {
        if self.supported_rates.contains(&rate) {
            self.active_refresh_rate = rate;
            Ok(())
        } else {
            Err(DriverError::Unsupported(format!(
                "Refresh rate {rate}Hz is not supported by this display panel"
            )))
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
    fn test_oled_display_driver_dimming() {
        let mut driver = OledDisplayDriver::new();
        assert_eq!(driver.dimming_factor(), 1.0);

        driver.set_flicker_free_dimming(0.75).unwrap();
        assert_eq!(driver.dimming_factor(), 0.75);

        // Clamping check
        driver.set_flicker_free_dimming(1.5).unwrap();
        assert_eq!(driver.dimming_factor(), 1.0);

        driver.set_flicker_free_dimming(-0.5).unwrap();
        assert_eq!(driver.dimming_factor(), 0.0);
    }

    #[test]
    fn test_oled_display_driver_refresh_rates() {
        let mut driver = OledDisplayDriver::new();
        assert_eq!(driver.supported_refresh_rates(), &[60, 120]);
        assert_eq!(driver.active_refresh_rate(), 120);

        driver.set_refresh_rate(60).unwrap();
        assert_eq!(driver.active_refresh_rate(), 60);

        assert!(driver.set_refresh_rate(144).is_err());
    }
}

