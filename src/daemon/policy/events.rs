// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::domain::ThermalMode;

/// Observable environmental and system telemetry events triggering policy evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum SystemEvent {
    /// AC power supply connection state changed (`true` = connected / On AC).
    AcStateChanged(bool),
    /// Battery charge level percentage updated (0..=100).
    BatteryLevelChanged(u8),
    /// Temperature or thermal threshold alert received from embedded controller.
    ThermalAlert(u32),
    /// Host system resumed from low-power suspend / sleep state.
    ResumeFromSuspend,
}

/// Point-in-time snapshot of hardware status used for startup reconciliation.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HardwareSnapshot {
    pub on_ac: bool,
    pub battery_level: Option<u8>,
    pub current_thermal_mode: Option<ThermalMode>,
}

impl HardwareSnapshot {
    /// Constructs a new hardware snapshot.
    pub fn new(
        on_ac: bool,
        battery_level: Option<u8>,
        current_thermal_mode: Option<ThermalMode>,
    ) -> Self {
        Self {
            on_ac,
            battery_level,
            current_thermal_mode,
        }
    }
}
