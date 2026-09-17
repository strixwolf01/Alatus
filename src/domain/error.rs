// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use thiserror::Error;

/// Domain-level errors representing invariant violations and parsing failures.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DomainError {
    #[error("Charge threshold must be between 50 and 100, got {0}")]
    InvalidChargeThreshold(u8),

    #[error("Brightness percent must be between 0 and 100, got {0}")]
    InvalidBrightness(u8),

    #[error("Timeout duration must be between 10s and 600s, got {0}s")]
    InvalidTimeoutSeconds(u64),

    #[error("Invalid hex color string: {0}")]
    InvalidHexColor(String),

    #[error("Unknown thermal mode: '{0}' (expected: quiet, balanced, performance, full_speed)")]
    InvalidThermalMode(String),

    #[error("Unknown RGB timeout policy: '{0}' (expected: never, only_on_battery, always)")]
    InvalidTimeoutPolicy(String),
}
