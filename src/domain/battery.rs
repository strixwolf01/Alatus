// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::domain::error::DomainError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;

/// Bounded battery charge threshold (50..=100%), enforced in alignment with ASUS EC/WMI specifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ChargeThreshold(u8);

impl ChargeThreshold {
    /// Minimum supported battery charge threshold on ASUS platforms.
    pub const MIN: u8 = 50;
    /// Maximum battery charge threshold (100% full capacity).
    pub const MAX: u8 = 100;
    /// Recommended battery health default threshold (80%).
    pub const DEFAULT: u8 = 80;

    /// Validates and constructs a new `ChargeThreshold`.
    pub fn new(val: u8) -> Result<Self, DomainError> {
        if (Self::MIN..=Self::MAX).contains(&val) {
            Ok(Self(val))
        } else {
            Err(DomainError::InvalidChargeThreshold(val))
        }
    }

    /// Returns the underlying numeric threshold value.
    pub fn value(&self) -> u8 {
        self.0
    }
}

impl Default for ChargeThreshold {
    fn default() -> Self {
        Self(Self::DEFAULT)
    }
}

impl TryFrom<u8> for ChargeThreshold {
    type Error = DomainError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl Deref for ChargeThreshold {
    type Target = u8;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for ChargeThreshold {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}%", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_charge_thresholds() {
        assert_eq!(ChargeThreshold::new(50).unwrap().value(), 50);
        assert_eq!(ChargeThreshold::new(80).unwrap().value(), 80);
        assert_eq!(ChargeThreshold::new(100).unwrap().value(), 100);
        assert_eq!(ChargeThreshold::default().value(), 80);
    }

    #[test]
    fn test_invalid_charge_thresholds() {
        assert_eq!(
            ChargeThreshold::new(0),
            Err(DomainError::InvalidChargeThreshold(0))
        );
        assert_eq!(
            ChargeThreshold::new(49),
            Err(DomainError::InvalidChargeThreshold(49))
        );
        assert_eq!(
            ChargeThreshold::new(101),
            Err(DomainError::InvalidChargeThreshold(101))
        );
        assert_eq!(
            ChargeThreshold::new(255),
            Err(DomainError::InvalidChargeThreshold(255))
        );
    }

    #[test]
    fn test_try_from_and_deref() {
        let thresh = ChargeThreshold::try_from(75).expect("75 is valid");
        assert_eq!(*thresh, 75);
        assert_eq!(thresh.value(), 75);

        assert!(ChargeThreshold::try_from(40).is_err());
    }

    #[test]
    fn test_display_format() {
        let thresh = ChargeThreshold::new(80).unwrap();
        assert_eq!(thresh.to_string(), "80%");
    }

    #[test]
    fn test_serde_roundtrip() {
        let original = ChargeThreshold::new(60).unwrap();
        let serialized = serde_json::to_string(&original).unwrap();
        assert_eq!(serialized, "60");

        let deserialized: ChargeThreshold = serde_json::from_str(&serialized).unwrap();
        assert_eq!(original, deserialized);
    }
}
