// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Pure decision evaluation layer and hardware safety gate.

pub mod decision;
pub mod engine;
pub mod events;
pub mod safety_gate;

pub use decision::{PolicyAction, PolicyDecision};
pub use engine::PolicyEngine;
pub use events::{HardwareSnapshot, SystemEvent};
pub use safety_gate::SafetyGate;
