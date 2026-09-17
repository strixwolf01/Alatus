// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::domain::error::DomainError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;
use std::str::FromStr;
use std::time::Duration;

/// Validated RGB brightness percentage (0..=100%).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BrightnessPercent(u8);

impl BrightnessPercent {
    pub const MIN: u8 = 0;
    pub const MAX: u8 = 100;
    pub const DEFAULT: u8 = 100;

    /// Validates and constructs a new `BrightnessPercent`.
    pub fn new(val: u8) -> Result<Self, DomainError> {
        if val <= Self::MAX {
            Ok(Self(val))
        } else {
            Err(DomainError::InvalidBrightness(val))
        }
    }

    /// Returns the raw percentage value.
    pub fn value(&self) -> u8 {
        self.0
    }
}

impl Default for BrightnessPercent {
    fn default() -> Self {
        Self(Self::DEFAULT)
    }
}

impl TryFrom<u8> for BrightnessPercent {
    type Error = DomainError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl Deref for BrightnessPercent {
    type Target = u8;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for BrightnessPercent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}%", self.0)
    }
}

/// 24-bit sRGB color representation with hex parsing and serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct ColorRgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl ColorRgb {
    /// Constructs a color from red, green, and blue 8-bit components.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parses a color from standard hex formats (`#RRGGBB`, `RRGGBB`, `#RGB`, `RGB`).
    pub fn from_hex(s: &str) -> Result<Self, DomainError> {
        let clean = s.trim().trim_start_matches('#');
        match clean.len() {
            6 => {
                let r = u8::from_str_radix(&clean[0..2], 16)
                    .map_err(|_| DomainError::InvalidHexColor(s.to_string()))?;
                let g = u8::from_str_radix(&clean[2..4], 16)
                    .map_err(|_| DomainError::InvalidHexColor(s.to_string()))?;
                let b = u8::from_str_radix(&clean[4..6], 16)
                    .map_err(|_| DomainError::InvalidHexColor(s.to_string()))?;
                Ok(Self { r, g, b })
            }
            3 => {
                let r_nibble = u8::from_str_radix(&clean[0..1], 16)
                    .map_err(|_| DomainError::InvalidHexColor(s.to_string()))?;
                let g_nibble = u8::from_str_radix(&clean[1..2], 16)
                    .map_err(|_| DomainError::InvalidHexColor(s.to_string()))?;
                let b_nibble = u8::from_str_radix(&clean[2..3], 16)
                    .map_err(|_| DomainError::InvalidHexColor(s.to_string()))?;
                Ok(Self {
                    r: (r_nibble << 4) | r_nibble,
                    g: (g_nibble << 4) | g_nibble,
                    b: (b_nibble << 4) | b_nibble,
                })
            }
            _ => Err(DomainError::InvalidHexColor(s.to_string())),
        }
    }

    /// Formats the color as a standard lowercase `#rrggbb` hex string.
    pub fn to_hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

impl fmt::Display for ColorRgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl FromStr for ColorRgb {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_hex(s)
    }
}

/// Keyboard backlighting inactivity timeout policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RgbTimeoutPolicy {
    /// Inactivity sleep is disabled; backlighting remains on indefinitely.
    Never,
    /// Inactivity sleep activates only when running on battery power.
    OnlyOnBattery,
    /// Inactivity sleep activates on both AC and battery power.
    #[default]
    Always,
}

impl RgbTimeoutPolicy {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Never => "never",
            Self::OnlyOnBattery => "only_on_battery",
            Self::Always => "always",
        }
    }
}

impl fmt::Display for RgbTimeoutPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for RgbTimeoutPolicy {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().trim() {
            "never" | "off" | "disabled" | "none" => Ok(Self::Never),
            "only_on_battery" | "battery_only" | "battery" | "bat" => Ok(Self::OnlyOnBattery),
            "always" | "all" | "enabled" | "on" => Ok(Self::Always),
            other => Err(DomainError::InvalidTimeoutPolicy(other.to_string())),
        }
    }
}

/// Bounded inactivity timeout duration (10s..=600s).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TimeoutDuration(u64);

impl TimeoutDuration {
    pub const MIN_SECS: u64 = 10;
    pub const MAX_SECS: u64 = 600;
    pub const DEFAULT_SECS: u64 = 60;

    /// Constructs a `TimeoutDuration` from seconds, validating boundaries.
    pub fn from_secs(secs: u64) -> Result<Self, DomainError> {
        if (Self::MIN_SECS..=Self::MAX_SECS).contains(&secs) {
            Ok(Self(secs))
        } else {
            Err(DomainError::InvalidTimeoutSeconds(secs))
        }
    }

    /// Constructs a `TimeoutDuration` from a `std::time::Duration`.
    pub fn from_duration(duration: Duration) -> Result<Self, DomainError> {
        Self::from_secs(duration.as_secs())
    }

    /// Returns the duration in seconds.
    pub fn as_secs(&self) -> u64 {
        self.0
    }

    /// Converts into a `std::time::Duration`.
    pub fn as_duration(&self) -> Duration {
        Duration::from_secs(self.0)
    }
}

impl Default for TimeoutDuration {
    fn default() -> Self {
        Self(Self::DEFAULT_SECS)
    }
}

impl TryFrom<u64> for TimeoutDuration {
    type Error = DomainError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::from_secs(value)
    }
}

impl TryFrom<Duration> for TimeoutDuration {
    type Error = DomainError;

    fn try_from(value: Duration) -> Result<Self, Self::Error> {
        Self::from_duration(value)
    }
}

impl fmt::Display for TimeoutDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}s", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brightness_bounds() {
        assert_eq!(BrightnessPercent::new(0).unwrap().value(), 0);
        assert_eq!(BrightnessPercent::new(50).unwrap().value(), 50);
        assert_eq!(BrightnessPercent::new(100).unwrap().value(), 100);
        assert_eq!(BrightnessPercent::default().value(), 100);

        assert_eq!(
            BrightnessPercent::new(101),
            Err(DomainError::InvalidBrightness(101))
        );
        assert_eq!(
            BrightnessPercent::new(255),
            Err(DomainError::InvalidBrightness(255))
        );

        let bp = BrightnessPercent::try_from(42).unwrap();
        assert_eq!(*bp, 42);
        assert_eq!(bp.to_string(), "42%");
    }

    #[test]
    fn test_color_rgb_hex_parsing() {
        let c1 = ColorRgb::from_hex("#ff8800").unwrap();
        assert_eq!(c1, ColorRgb::new(255, 136, 0));
        assert_eq!(c1.to_hex(), "#ff8800");
        assert_eq!(c1.to_string(), "#ff8800");

        let c2 = ColorRgb::from_hex("00ffcc").unwrap();
        assert_eq!(c2, ColorRgb::new(0, 255, 204));

        let c3 = ColorRgb::from_hex("#f80").unwrap();
        assert_eq!(c3, ColorRgb::new(0xff, 0x88, 0x00));

        assert!(ColorRgb::from_hex("#xyz123").is_err());
        assert!(ColorRgb::from_hex("#12").is_err());
        assert!(ColorRgb::from_hex("1234567").is_err());
    }

    #[test]
    fn test_rgb_timeout_policy_parsing_and_serde() {
        assert_eq!(
            "never".parse::<RgbTimeoutPolicy>().unwrap(),
            RgbTimeoutPolicy::Never
        );
        assert_eq!(
            "battery".parse::<RgbTimeoutPolicy>().unwrap(),
            RgbTimeoutPolicy::OnlyOnBattery
        );
        assert_eq!(
            "only_on_battery".parse::<RgbTimeoutPolicy>().unwrap(),
            RgbTimeoutPolicy::OnlyOnBattery
        );
        assert_eq!(
            "always".parse::<RgbTimeoutPolicy>().unwrap(),
            RgbTimeoutPolicy::Always
        );

        assert!(matches!(
            "unknown".parse::<RgbTimeoutPolicy>(),
            Err(DomainError::InvalidTimeoutPolicy(_))
        ));

        let ser = serde_json::to_string(&RgbTimeoutPolicy::OnlyOnBattery).unwrap();
        assert_eq!(ser, "\"only_on_battery\"");
        let de: RgbTimeoutPolicy = serde_json::from_str(&ser).unwrap();
        assert_eq!(de, RgbTimeoutPolicy::OnlyOnBattery);
    }

    #[test]
    fn test_timeout_duration_bounds() {
        assert_eq!(TimeoutDuration::from_secs(10).unwrap().as_secs(), 10);
        assert_eq!(TimeoutDuration::from_secs(60).unwrap().as_secs(), 60);
        assert_eq!(TimeoutDuration::from_secs(600).unwrap().as_secs(), 600);
        assert_eq!(TimeoutDuration::default().as_secs(), 60);

        assert_eq!(
            TimeoutDuration::from_secs(9),
            Err(DomainError::InvalidTimeoutSeconds(9))
        );
        assert_eq!(
            TimeoutDuration::from_secs(601),
            Err(DomainError::InvalidTimeoutSeconds(601))
        );

        let dur = Duration::from_secs(45);
        let td = TimeoutDuration::try_from(dur).unwrap();
        assert_eq!(td.as_duration(), dur);
        assert_eq!(td.to_string(), "45s");
    }
}
