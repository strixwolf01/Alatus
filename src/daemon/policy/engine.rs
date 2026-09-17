// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::daemon::policy::decision::{PolicyAction, PolicyDecision};
use crate::daemon::policy::events::{HardwareSnapshot, SystemEvent};
use crate::domain::{RgbTimeoutPolicy, ThermalMode};
use std::time::{Duration, Instant};

/// Dynamic policy engine evaluating environmental events and hardware states.
///
/// Operates strictly as a pure decision engine (Policy ≠ Hardware Driver),
/// enforcing signal debouncing, hysteresis deadbands, and cooldown stability.
pub struct PolicyEngine {
    pub on_ac: bool,
    pub battery_level: Option<u8>,
    pub current_thermal_mode: Option<ThermalMode>,
    pub low_battery_quiet_active: bool,
    pub debounce_duration: Duration,
    pub last_ac_event_time: Option<Instant>,
    pub last_ac_state: Option<bool>,
    pub last_applied_thermal_mode: Option<ThermalMode>,
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PolicyEngine {
    /// Constructs a `PolicyEngine` with a standard 1.5s AC debounce window.
    pub fn new() -> Self {
        Self {
            on_ac: true,
            battery_level: None,
            current_thermal_mode: None,
            low_battery_quiet_active: false,
            debounce_duration: Duration::from_millis(1500),
            last_ac_event_time: None,
            last_ac_state: None,
            last_applied_thermal_mode: None,
        }
    }

    /// Constructs a `PolicyEngine` with a custom debounce duration.
    pub fn with_debounce(debounce_duration: Duration) -> Self {
        Self {
            on_ac: true,
            battery_level: None,
            current_thermal_mode: None,
            low_battery_quiet_active: false,
            debounce_duration,
            last_ac_event_time: None,
            last_ac_state: None,
            last_applied_thermal_mode: None,
        }
    }

    /// Evaluates an event at current time.
    pub fn evaluate_event(&mut self, event: SystemEvent) -> Option<PolicyDecision> {
        self.evaluate_event_at(event, Instant::now())
    }

    /// Evaluates an event at an explicit timestamp for deterministic testing.
    pub fn evaluate_event_at(
        &mut self,
        event: SystemEvent,
        now: Instant,
    ) -> Option<PolicyDecision> {
        match event {
            SystemEvent::AcStateChanged(new_ac) => {
                // If redundant with current state, ignore
                if self.last_ac_state == Some(new_ac) {
                    return None;
                }

                // Debounce check: suppress flapping transitions within window
                if let Some(last_time) = self.last_ac_event_time
                    && now.duration_since(last_time) < self.debounce_duration
                {
                    tracing::debug!(
                        "PolicyEngine: Debouncing AC transition to {} (elapsed: {:?})",
                        new_ac,
                        now.duration_since(last_time)
                    );
                    return None;
                }

                // Record valid transition
                self.last_ac_event_time = Some(now);
                self.last_ac_state = Some(new_ac);
                self.on_ac = new_ac;

                if new_ac {
                    // Restored to AC: clear low battery hysteresis override
                    self.low_battery_quiet_active = false;
                    let target_mode = ThermalMode::Balanced;
                    self.last_applied_thermal_mode = Some(target_mode);
                    self.current_thermal_mode = Some(target_mode);
                    Some(PolicyDecision::with_timestamp(
                        PolicyAction::SetThermalMode(target_mode),
                        "ac-connected",
                        70,
                        now,
                    ))
                } else {
                    // Switched to Battery power
                    let target_mode = if self.low_battery_quiet_active
                        || self.battery_level.is_some_and(|l| l <= 15)
                    {
                        self.low_battery_quiet_active = true;
                        ThermalMode::Quiet
                    } else {
                        ThermalMode::Balanced
                    };

                    self.last_applied_thermal_mode = Some(target_mode);
                    self.current_thermal_mode = Some(target_mode);
                    let reason = if self.low_battery_quiet_active {
                        "critical-battery-unplugged"
                    } else {
                        "ac-disconnected"
                    };
                    Some(PolicyDecision::with_timestamp(
                        PolicyAction::SetThermalMode(target_mode),
                        reason,
                        70,
                        now,
                    ))
                }
            }

            SystemEvent::BatteryLevelChanged(level) => {
                self.battery_level = Some(level);

                // Hysteresis deadband evaluation on battery power:
                // Enter Quiet at <= 15%.
                // Exit Quiet only when >= 20% (or AC connected).
                if !self.on_ac {
                    if level <= 15 {
                        if !self.low_battery_quiet_active {
                            self.low_battery_quiet_active = true;
                            let target = ThermalMode::Quiet;
                            if self.last_applied_thermal_mode != Some(target) {
                                self.last_applied_thermal_mode = Some(target);
                                self.current_thermal_mode = Some(target);
                                return Some(PolicyDecision::with_timestamp(
                                    PolicyAction::SetThermalMode(target),
                                    "critical-battery",
                                    90,
                                    now,
                                ));
                            }
                        }
                    } else if level >= 20 && self.low_battery_quiet_active {
                        // Recovered past deadband threshold
                        self.low_battery_quiet_active = false;
                        let target = ThermalMode::Balanced;
                        if self.last_applied_thermal_mode != Some(target) {
                            self.last_applied_thermal_mode = Some(target);
                            self.current_thermal_mode = Some(target);
                            return Some(PolicyDecision::with_timestamp(
                                PolicyAction::SetThermalMode(target),
                                "battery-restored",
                                80,
                                now,
                            ));
                        }
                    }
                }
                None
            }

            SystemEvent::ThermalAlert(temp) => {
                // Emergency cooling boost if core temperature reaches critical limit
                if temp >= 95 {
                    let target = ThermalMode::FullSpeed;
                    if self.last_applied_thermal_mode != Some(target) {
                        self.last_applied_thermal_mode = Some(target);
                        self.current_thermal_mode = Some(target);
                        return Some(PolicyDecision::with_timestamp(
                            PolicyAction::SetThermalMode(target),
                            "thermal-emergency-overheat",
                            100,
                            now,
                        ));
                    }
                }
                None
            }

            SystemEvent::ResumeFromSuspend => {
                let target_mode = if self.on_ac {
                    ThermalMode::Balanced
                } else if self.low_battery_quiet_active
                    || self.battery_level.is_some_and(|l| l <= 15)
                {
                    self.low_battery_quiet_active = true;
                    ThermalMode::Quiet
                } else {
                    ThermalMode::Balanced
                };

                self.last_applied_thermal_mode = Some(target_mode);
                self.current_thermal_mode = Some(target_mode);
                Some(PolicyDecision::with_timestamp(
                    PolicyAction::SetThermalMode(target_mode),
                    "resume-reconcile",
                    75,
                    now,
                ))
            }
        }
    }

    /// Reconciles startup hardware state and generates initial configuration decisions.
    pub fn reconcile_startup(&mut self, current_state: HardwareSnapshot) -> Vec<PolicyDecision> {
        self.on_ac = current_state.on_ac;
        self.battery_level = current_state.battery_level;
        self.current_thermal_mode = current_state.current_thermal_mode;
        self.last_ac_state = Some(current_state.on_ac);
        self.last_ac_event_time = Some(Instant::now());

        let mut decisions = Vec::new();

        if !current_state.on_ac && current_state.battery_level.is_some_and(|level| level <= 15) {
            self.low_battery_quiet_active = true;
            let target = ThermalMode::Quiet;
            self.last_applied_thermal_mode = Some(target);
            self.current_thermal_mode = Some(target);
            decisions.push(PolicyDecision::new(
                PolicyAction::SetThermalMode(target),
                "critical-battery-startup",
                90,
            ));
            decisions.push(PolicyDecision::new(
                PolicyAction::SetRgbTimeout(RgbTimeoutPolicy::OnlyOnBattery),
                "startup-battery-rgb",
                50,
            ));
        } else if !current_state.on_ac {
            let target = ThermalMode::Balanced;
            self.last_applied_thermal_mode = Some(target);
            self.current_thermal_mode = Some(target);
            decisions.push(PolicyDecision::new(
                PolicyAction::SetThermalMode(target),
                "startup-battery",
                50,
            ));
            decisions.push(PolicyDecision::new(
                PolicyAction::SetRgbTimeout(RgbTimeoutPolicy::OnlyOnBattery),
                "startup-battery-rgb",
                50,
            ));
        } else {
            let target = ThermalMode::Balanced;
            self.last_applied_thermal_mode = Some(target);
            self.current_thermal_mode = Some(target);
            decisions.push(PolicyDecision::new(
                PolicyAction::SetThermalMode(target),
                "startup-ac",
                50,
            ));
            decisions.push(PolicyDecision::new(
                PolicyAction::SetRgbTimeout(RgbTimeoutPolicy::Always),
                "startup-ac-rgb",
                50,
            ));
        }

        decisions
    }
}
