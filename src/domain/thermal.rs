// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::domain::error::DomainError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Platform thermal profile mode supported across ASUS notebook firmware.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThermalMode {
    /// Silent / power-saving fan curve.
    Quiet,
    /// Standard balanced acoustic and thermal curve.
    #[default]
    Balanced,
    /// High-performance fan boost profile.
    Performance,
    /// Maximum fan speed curve for thermal stress scenarios.
    FullSpeed,
}

impl ThermalMode {
    /// Returns the static lowercase identifier for this thermal mode.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Quiet => "quiet",
            Self::Balanced => "balanced",
            Self::Performance => "performance",
            Self::FullSpeed => "full_speed",
        }
    }
}

impl fmt::Display for ThermalMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for ThermalMode {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().trim() {
            "quiet" | "silent" => Ok(Self::Quiet),
            "balanced" | "standard" | "default" => Ok(Self::Balanced),
            "performance" | "perf" | "high" | "boost" => Ok(Self::Performance),
            "full_speed" | "fullspeed" | "full" | "max" => Ok(Self::FullSpeed),
            other => Err(DomainError::InvalidThermalMode(other.to_string())),
        }
    }
}

impl TryFrom<&str> for ThermalMode {
    type Error = DomainError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        value.parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thermal_mode_string_representations() {
        assert_eq!(ThermalMode::Quiet.as_str(), "quiet");
        assert_eq!(ThermalMode::Balanced.as_str(), "balanced");
        assert_eq!(ThermalMode::Performance.as_str(), "performance");
        assert_eq!(ThermalMode::FullSpeed.as_str(), "full_speed");
        assert_eq!(ThermalMode::default(), ThermalMode::Balanced);
    }

    #[test]
    fn test_thermal_mode_parsing() {
        assert_eq!("quiet".parse::<ThermalMode>().unwrap(), ThermalMode::Quiet);
        assert_eq!("SILENT".parse::<ThermalMode>().unwrap(), ThermalMode::Quiet);
        assert_eq!(
            "balanced".parse::<ThermalMode>().unwrap(),
            ThermalMode::Balanced
        );
        assert_eq!(
            "performance".parse::<ThermalMode>().unwrap(),
            ThermalMode::Performance
        );
        assert_eq!(
            "perf".parse::<ThermalMode>().unwrap(),
            ThermalMode::Performance
        );
        assert_eq!(
            "full_speed".parse::<ThermalMode>().unwrap(),
            ThermalMode::FullSpeed
        );
        assert_eq!(
            "max".parse::<ThermalMode>().unwrap(),
            ThermalMode::FullSpeed
        );

        assert!(matches!(
            "invalid".parse::<ThermalMode>(),
            Err(DomainError::InvalidThermalMode(_))
        ));
    }

    #[test]
    fn test_try_from_str() {
        assert_eq!(ThermalMode::try_from("quiet").unwrap(), ThermalMode::Quiet);
        assert!(ThermalMode::try_from("turbo").is_err());
    }

    #[test]
    fn test_serde_roundtrip() {
        let modes = [
            (ThermalMode::Quiet, "\"quiet\""),
            (ThermalMode::Balanced, "\"balanced\""),
            (ThermalMode::Performance, "\"performance\""),
            (ThermalMode::FullSpeed, "\"full_speed\""),
        ];

        for (mode, expected_json) in modes {
            let serialized = serde_json::to_string(&mode).unwrap();
            assert_eq!(serialized, expected_json);

            let deserialized: ThermalMode = serde_json::from_str(&serialized).unwrap();
            assert_eq!(deserialized, mode);
        }
    }
}
