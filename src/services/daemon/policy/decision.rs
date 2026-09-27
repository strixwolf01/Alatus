// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::domain::{RgbTimeoutPolicy, ThermalMode};
use std::fmt;
use std::time::Instant;

/// Specific hardware action requested by the policy evaluation layer.
#[derive(Debug, Clone, PartialEq)]
pub enum PolicyAction {
    SetThermalMode(ThermalMode),
    SetRgbTimeout(RgbTimeoutPolicy),
    DimDisplay(f32),
}

/// A prioritized decision produced by policy evaluation, decoupled from hardware execution.
#[derive(Debug, Clone, PartialEq)]
pub struct PolicyDecision {
    pub action: PolicyAction,
    pub reason: &'static str,
    pub priority: u8,
    pub timestamp: Instant,
}

impl PolicyDecision {
    /// Constructs a new `PolicyDecision` with the current timestamp.
    pub fn new(action: PolicyAction, reason: &'static str, priority: u8) -> Self {
        Self {
            action,
            reason,
            priority,
            timestamp: Instant::now(),
        }
    }

    /// Constructs a `PolicyDecision` with an explicit timestamp for deterministic testing.
    pub fn with_timestamp(
        action: PolicyAction,
        reason: &'static str,
        priority: u8,
        timestamp: Instant,
    ) -> Self {
        Self {
            action,
            reason,
            priority,
            timestamp,
        }
    }
}

impl fmt::Display for PolicyDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.action {
            PolicyAction::SetThermalMode(mode) => {
                write!(
                    f,
                    "PolicyDecision(SetThermalMode({mode}), reason=\"{}\", priority={})",
                    self.reason, self.priority
                )
            }
            PolicyAction::SetRgbTimeout(policy) => {
                write!(
                    f,
                    "PolicyDecision(SetRgbTimeout({policy:?}), reason=\"{}\", priority={})",
                    self.reason, self.priority
                )
            }
            PolicyAction::DimDisplay(factor) => {
                write!(
                    f,
                    "PolicyDecision(DimDisplay({factor:.2}), reason=\"{}\", priority={})",
                    self.reason, self.priority
                )
            }
        }
    }
}
