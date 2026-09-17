// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::daemon::policy::decision::{PolicyAction, PolicyDecision};
use crate::hardware::DeviceContext;

/// Safety gate validating policy decisions against active runtime capabilities
/// before executing actions against hardware drivers.
pub struct SafetyGate;

impl SafetyGate {
    /// Validates `decision` against `ctx.capabilities` and dispatches action if supported and operational.
    ///
    /// Returns `true` if the action was successfully executed on hardware, or `false` if dropped
    /// due to unsupported/unavailable subsystem status or driver execution error.
    pub fn dispatch(decision: &PolicyDecision, ctx: &mut DeviceContext) -> bool {
        match &decision.action {
            PolicyAction::SetThermalMode(mode) => {
                if !ctx.capabilities.thermal.is_supported() {
                    tracing::info!(
                        "SafetyGate: Dropped {:?} - thermal capability not supported or unavailable: {:?}",
                        decision,
                        ctx.capabilities.thermal
                    );
                    return false;
                }

                if let Some(ref mut th) = ctx.thermal {
                    match th.set_mode(*mode) {
                        Ok(()) => {
                            tracing::info!(
                                "SafetyGate: Dispatched {:?} to thermal driver",
                                decision
                            );
                            true
                        }
                        Err(e) => {
                            tracing::error!("SafetyGate: Failed dispatching {:?}: {e}", decision);
                            false
                        }
                    }
                } else {
                    tracing::warn!(
                        "SafetyGate: Thermal capability is marked supported but driver instance is missing"
                    );
                    false
                }
            }

            PolicyAction::SetRgbTimeout(policy) => {
                if !ctx.capabilities.rgb.is_supported() {
                    tracing::info!(
                        "SafetyGate: Dropped {:?} - RGB capability not supported or unavailable: {:?}",
                        decision,
                        ctx.capabilities.rgb
                    );
                    return false;
                }

                if let Some(ref mut rgb) = ctx.rgb {
                    let duration = crate::domain::TimeoutDuration::default();
                    match rgb.set_timeout(*policy, duration) {
                        Ok(()) => {
                            tracing::info!("SafetyGate: Dispatched {:?} to RGB driver", decision);
                            true
                        }
                        Err(e) => {
                            tracing::error!("SafetyGate: Failed dispatching {:?}: {e}", decision);
                            false
                        }
                    }
                } else {
                    tracing::warn!(
                        "SafetyGate: RGB capability is marked supported but driver instance is missing"
                    );
                    false
                }
            }

            PolicyAction::DimDisplay(factor) => {
                if !ctx.capabilities.display.is_supported() {
                    tracing::info!(
                        "SafetyGate: Dropped {:?} - display capability not supported or unavailable: {:?}",
                        decision,
                        ctx.capabilities.display
                    );
                    return false;
                }

                if let Some(ref mut disp) = ctx.display {
                    match disp.set_flicker_free_dimming(*factor) {
                        Ok(()) => {
                            tracing::info!(
                                "SafetyGate: Dispatched {:?} to display driver",
                                decision
                            );
                            true
                        }
                        Err(e) => {
                            tracing::error!("SafetyGate: Failed dispatching {:?}: {e}", decision);
                            false
                        }
                    }
                } else {
                    tracing::warn!(
                        "SafetyGate: Display capability is marked supported but driver instance is missing"
                    );
                    false
                }
            }
        }
    }
}
