// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use std::time::{Duration, Instant};

use alatus::daemon::policy::{
    HardwareSnapshot, PolicyAction, PolicyDecision, PolicyEngine, SafetyGate, SystemEvent,
};
use alatus::domain::{RgbTimeoutPolicy, ThermalMode};
use alatus::hardware::DeviceContext;
use alatus::hardware::capabilities::{
    CapabilityState, RgbCapabilityDetails, ThermalCapabilityDetails, UnavailableReason,
};
use alatus::hardware::mock::{MockRgbDriver, MockThermalDriver};

// ============================================================================
// Test 1: AC State Debouncing & Flapping Suppression
// ============================================================================

#[test]
fn test_ac_debounce_flapping_suppression() {
    let mut engine = PolicyEngine::new();
    let start = Instant::now();

    // Initial state is on AC.
    // Event 0 (t = 0ms): AC disconnected -> emits decision.
    let decision_0 = engine.evaluate_event_at(SystemEvent::AcStateChanged(false), start);
    assert!(
        decision_0.is_some(),
        "First AC transition must produce a decision"
    );
    let d0 = decision_0.unwrap();
    assert_eq!(d0.action, PolicyAction::SetThermalMode(ThermalMode::Balanced));
    assert_eq!(d0.reason, "ac-disconnected");

    // Rapid flapping: 10 toggles within 100ms.
    let mut suppressed_count = 0;
    for i in 1..=10 {
        let t = start + Duration::from_millis(i * 10);
        let toggled_state = i % 2 == 1; // alternating true/false
        let decision = engine.evaluate_event_at(SystemEvent::AcStateChanged(toggled_state), t);
        if decision.is_none() {
            suppressed_count += 1;
        }
    }

    assert_eq!(
        suppressed_count, 10,
        "All 10 rapid AC toggles within 100ms must be debounced and suppressed"
    );

    // Fast-forward past 1500ms debounce window: transition must be accepted.
    let later = start + Duration::from_millis(1600);
    let decision_later = engine.evaluate_event_at(SystemEvent::AcStateChanged(true), later);
    assert!(
        decision_later.is_some(),
        "Transition after debounce window expires must be accepted"
    );
    let dl = decision_later.unwrap();
    assert_eq!(dl.action, PolicyAction::SetThermalMode(ThermalMode::Balanced));
    assert_eq!(dl.reason, "ac-connected");
}

// ============================================================================
// Test 2: Battery Hysteresis Deadband (<= 15% enter Quiet, >= 20% exit Quiet)
// ============================================================================

#[test]
fn test_battery_hysteresis_deadband() {
    let mut engine = PolicyEngine::new();
    let t0 = Instant::now();

    // Switch to battery power first
    let _ = engine.evaluate_event_at(SystemEvent::AcStateChanged(false), t0);

    // High battery: no low-power override
    let d80 = engine.evaluate_event_at(
        SystemEvent::BatteryLevelChanged(80),
        t0 + Duration::from_secs(2),
    );
    assert!(d80.is_none());
    assert!(!engine.low_battery_quiet_active);

    // Normal discharge above threshold: 16% (not yet critical)
    let d16 = engine.evaluate_event_at(
        SystemEvent::BatteryLevelChanged(16),
        t0 + Duration::from_secs(3),
    );
    assert!(d16.is_none());
    assert!(!engine.low_battery_quiet_active);

    // Critical threshold hit: 15% -> enter Quiet mode
    let d15 = engine.evaluate_event_at(
        SystemEvent::BatteryLevelChanged(15),
        t0 + Duration::from_secs(4),
    );
    assert!(
        d15.is_some(),
        "Battery at 15% must trigger critical low-power decision"
    );
    let d15_unwrapped = d15.unwrap();
    assert_eq!(
        d15_unwrapped.action,
        PolicyAction::SetThermalMode(ThermalMode::Quiet)
    );
    assert_eq!(d15_unwrapped.reason, "critical-battery");
    assert!(engine.low_battery_quiet_active);

    // Deadband test: fluctuations between 15% and 19% must stay in Quiet mode
    let d16_deadband = engine.evaluate_event_at(
        SystemEvent::BatteryLevelChanged(16),
        t0 + Duration::from_secs(5),
    );
    assert!(
        d16_deadband.is_none(),
        "Fluctuation to 16% must remain in Quiet mode (deadband)"
    );
    assert!(engine.low_battery_quiet_active);

    let d19_deadband = engine.evaluate_event_at(
        SystemEvent::BatteryLevelChanged(19),
        t0 + Duration::from_secs(6),
    );
    assert!(
        d19_deadband.is_none(),
        "Fluctuation to 19% must remain in Quiet mode (deadband)"
    );
    assert!(engine.low_battery_quiet_active);

    // Reaching 20%: exit Quiet mode deadband -> restore Balanced
    let d20 = engine.evaluate_event_at(
        SystemEvent::BatteryLevelChanged(20),
        t0 + Duration::from_secs(7),
    );
    assert!(
        d20.is_some(),
        "Battery recovering to 20% must exit low-power Quiet mode"
    );
    let d20_unwrapped = d20.unwrap();
    assert_eq!(
        d20_unwrapped.action,
        PolicyAction::SetThermalMode(ThermalMode::Balanced)
    );
    assert_eq!(d20_unwrapped.reason, "battery-restored");
    assert!(!engine.low_battery_quiet_active);
}

// ============================================================================
// Test 3: Safety Gate Dropping Unavailable Subsystems
// ============================================================================

#[test]
fn test_safety_gate_drops_unavailable_or_unsupported() {
    let mut ctx = DeviceContext::new();

    // 1. RGB Capability is Unavailable -> drop RGB decisions safely without panic
    ctx.capabilities.rgb = CapabilityState::Unavailable(UnavailableReason::HardwareError(
        "USB interface disconnected".to_string(),
    ));
    ctx.rgb = None;

    let rgb_decision = PolicyDecision::new(
        PolicyAction::SetRgbTimeout(RgbTimeoutPolicy::OnlyOnBattery),
        "low-power",
        50,
    );
    let executed = SafetyGate::dispatch(&rgb_decision, &mut ctx);
    assert!(
        !executed,
        "SafetyGate must reject RGB action when subsystem is Unavailable"
    );

    // 2. Display Capability is Unsupported -> drop Display decisions safely
    ctx.capabilities.display = CapabilityState::Unsupported;
    ctx.display = None;

    let display_decision = PolicyDecision::new(PolicyAction::DimDisplay(0.5), "dim-screen", 40);
    let executed_disp = SafetyGate::dispatch(&display_decision, &mut ctx);
    assert!(
        !executed_disp,
        "SafetyGate must reject display action when subsystem is Unsupported"
    );

    // 3. Thermal is Supported with active mock driver -> dispatch succeeds
    ctx.capabilities.thermal = CapabilityState::Supported(ThermalCapabilityDetails {
        supported_modes: vec![ThermalMode::Quiet, ThermalMode::Balanced],
        fan_count: 2,
        supports_fan_telemetry: true,
    });
    ctx.thermal = Some(Box::new(MockThermalDriver::new()));

    let thermal_decision = PolicyDecision::new(
        PolicyAction::SetThermalMode(ThermalMode::Quiet),
        "policy-cooling",
        80,
    );
    let executed_thermal = SafetyGate::dispatch(&thermal_decision, &mut ctx);
    assert!(
        executed_thermal,
        "SafetyGate must successfully dispatch action to supported driver"
    );
    assert_eq!(
        ctx.thermal.as_ref().unwrap().get_mode().unwrap(),
        ThermalMode::Quiet
    );
}

// ============================================================================
// Test 4: Startup State Reconciliation
// ============================================================================

#[test]
fn test_startup_state_reconciliation_on_low_battery() {
    let mut engine = PolicyEngine::new();

    // Snapshot: Unplugged (battery power) with critical 10% charge
    let snapshot = HardwareSnapshot::new(false, Some(10), Some(ThermalMode::Balanced));
    let decisions = engine.reconcile_startup(snapshot);

    assert_eq!(decisions.len(), 2);
    assert_eq!(
        decisions[0].action,
        PolicyAction::SetThermalMode(ThermalMode::Quiet)
    );
    assert_eq!(decisions[0].reason, "critical-battery-startup");
    assert_eq!(
        decisions[1].action,
        PolicyAction::SetRgbTimeout(RgbTimeoutPolicy::OnlyOnBattery)
    );
    assert!(engine.low_battery_quiet_active);
    assert_eq!(engine.current_thermal_mode, Some(ThermalMode::Quiet));
}

#[test]
fn test_startup_state_reconciliation_on_ac_power() {
    let mut engine = PolicyEngine::new();

    // Snapshot: Connected to AC power with 80% charge
    let snapshot = HardwareSnapshot::new(true, Some(80), Some(ThermalMode::Quiet));
    let decisions = engine.reconcile_startup(snapshot);

    assert_eq!(decisions.len(), 2);
    assert_eq!(
        decisions[0].action,
        PolicyAction::SetThermalMode(ThermalMode::Balanced)
    );
    assert_eq!(decisions[0].reason, "startup-ac");
    assert_eq!(
        decisions[1].action,
        PolicyAction::SetRgbTimeout(RgbTimeoutPolicy::Always)
    );
    assert!(!engine.low_battery_quiet_active);
    assert_eq!(engine.current_thermal_mode, Some(ThermalMode::Balanced));
}

// ============================================================================
// Test 5: End-to-End Startup Reconciliation Through Safety Gate
// ============================================================================

#[test]
fn test_end_to_end_startup_reconciliation_dispatch() {
    let mut engine = PolicyEngine::new();

    // Configure mock DeviceContext
    let mut ctx = DeviceContext::new();
    ctx.capabilities.thermal = CapabilityState::Supported(ThermalCapabilityDetails {
        supported_modes: vec![
            ThermalMode::Quiet,
            ThermalMode::Balanced,
            ThermalMode::Performance,
        ],
        fan_count: 2,
        supports_fan_telemetry: true,
    });
    ctx.thermal = Some(Box::new(MockThermalDriver::new()));

    ctx.capabilities.rgb = CapabilityState::Supported(RgbCapabilityDetails {
        max_brightness: 255,
        supports_custom_color: true,
        supports_inactivity_timeout: true,
        supported_zones: vec!["zone0".to_string()],
    });
    ctx.rgb = Some(Box::new(MockRgbDriver::new()));

    // Simulate startup on battery with 12% charge
    let snapshot = HardwareSnapshot::new(false, Some(12), Some(ThermalMode::Performance));
    let decisions = engine.reconcile_startup(snapshot);

    for decision in &decisions {
        let dispatched = SafetyGate::dispatch(decision, &mut ctx);
        assert!(
            dispatched,
            "Decision {:?} must dispatch successfully",
            decision
        );
    }

    // Verify drivers were updated according to policy decisions
    assert_eq!(
        ctx.thermal.as_ref().unwrap().get_mode().unwrap(),
        ThermalMode::Quiet
    );
}

