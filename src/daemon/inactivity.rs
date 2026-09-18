// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::services::config::RgbTimeoutPolicy;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering};

pub struct InactivityState {
    pub timeout_seconds: AtomicU32,
    pub policy: AtomicU8, // 0 = Never, 1 = BatteryOnly, 2 = Always
    pub is_timed_out: AtomicBool,
    pub last_activity_ms: AtomicU64,
}

impl InactivityState {
    pub fn new(timeout_seconds: u32, policy: RgbTimeoutPolicy) -> Self {
        let now_ms = Self::current_ms();
        Self {
            timeout_seconds: AtomicU32::new(timeout_seconds),
            policy: AtomicU8::new(Self::policy_to_code(policy)),
            is_timed_out: AtomicBool::new(false),
            last_activity_ms: AtomicU64::new(now_ms),
        }
    }

    fn policy_to_code(policy: RgbTimeoutPolicy) -> u8 {
        match policy {
            RgbTimeoutPolicy::Never => 0,
            RgbTimeoutPolicy::BatteryOnly => 1,
            RgbTimeoutPolicy::Always => 2,
        }
    }

    pub fn code_to_policy(code: u8) -> RgbTimeoutPolicy {
        match code {
            0 => RgbTimeoutPolicy::Never,
            1 => RgbTimeoutPolicy::BatteryOnly,
            _ => RgbTimeoutPolicy::Always,
        }
    }

    pub fn get_policy(&self) -> RgbTimeoutPolicy {
        Self::code_to_policy(self.policy.load(Ordering::Relaxed))
    }

    pub fn set_policy(
        &self,
        policy: RgbTimeoutPolicy,
        rgb_service: &crate::services::rgb::RgbService,
        on_ac: bool,
    ) {
        self.policy
            .store(Self::policy_to_code(policy), Ordering::Relaxed);
        self.last_activity_ms
            .store(Self::current_ms(), Ordering::Relaxed);
        if policy == RgbTimeoutPolicy::Never || (policy == RgbTimeoutPolicy::BatteryOnly && on_ac) {
            self.wake_if_timed_out(rgb_service);
        }
    }

    pub fn wake_if_timed_out(&self, rgb_service: &crate::services::rgb::RgbService) {
        if self.is_timed_out.swap(false, Ordering::SeqCst) {
            tracing::info!("Waking RGB backlight due to timeout policy or power status update.");
            let _ = rgb_service.wake();
        }
    }

    pub fn wake(&self, rgb_service: &crate::services::rgb::RgbService) {
        self.last_activity_ms
            .store(Self::current_ms(), Ordering::Relaxed);
        let was_timed_out = self.is_timed_out.swap(false, Ordering::SeqCst);
        tracing::info!("Explicit RGB wake requested (was_timed_out={was_timed_out}).");
        let _ = rgb_service.wake();
    }

    fn current_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    pub fn record_activity(&self, rgb_service: &crate::services::rgb::RgbService) {
        self.last_activity_ms
            .store(Self::current_ms(), Ordering::Relaxed);
        if self.is_timed_out.swap(false, Ordering::SeqCst) {
            tracing::info!("User input detected. Waking RGB backlight immediately.");
            let _ = rgb_service.wake();
        }
    }

    /// Returns `true` if an inactivity timeout is currently active based on policy and power status.
    pub fn is_timeout_enabled(&self, on_ac: bool) -> bool {
        let policy = self.get_policy();
        if policy == RgbTimeoutPolicy::Never {
            return false;
        }
        if policy == RgbTimeoutPolicy::BatteryOnly && on_ac {
            return false;
        }
        let timeout = self.timeout_seconds.load(Ordering::Relaxed);
        timeout > 0
    }

    /// Resets the inactivity timer to the current instant and clears any timed-out state.
    /// Used on system resume or dynamic power reconciliation to prevent stale timeouts.
    pub fn reset_activity_timer(&self) {
        self.last_activity_ms
            .store(Self::current_ms(), Ordering::Relaxed);
        self.is_timed_out.store(false, Ordering::SeqCst);
        tracing::info!("Inactivity timer reset (cleared timeout state).");
    }

    pub fn check_timeout(&self, rgb_service: &crate::services::rgb::RgbService, on_ac: bool) {
        if !self.is_timeout_enabled(on_ac) {
            self.wake_if_timed_out(rgb_service);
            return;
        }

        if self.is_timed_out.load(Ordering::Relaxed) {
            return;
        }
        let timeout = self.timeout_seconds.load(Ordering::Relaxed);
        let elapsed_ms =
            Self::current_ms().saturating_sub(self.last_activity_ms.load(Ordering::Relaxed));
        if elapsed_ms >= (timeout as u64 * 1000) && !self.is_timed_out.swap(true, Ordering::SeqCst)
        {
            let policy = self.get_policy();
            tracing::info!(
                "RGB inactivity timeout of {timeout}s reached (policy={policy:?}, on_ac={on_ac}). Dimming backlight."
            );
            let _ = rgb_service.sleep_timeout();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::rgb::RgbService;

    #[test]
    fn test_never_sleep_invariant() {
        let rgb_service = RgbService::new();

        // 1. Policy = Never
        let state_never = InactivityState::new(30, RgbTimeoutPolicy::Never);
        assert!(!state_never.is_timeout_enabled(true));
        assert!(!state_never.is_timeout_enabled(false));

        // Simulate stale activity timestamp from 1 hour ago
        state_never.last_activity_ms.store(0, Ordering::Relaxed);
        state_never.check_timeout(&rgb_service, true);
        assert!(!state_never.is_timed_out.load(Ordering::Relaxed));
        state_never.check_timeout(&rgb_service, false);
        assert!(!state_never.is_timed_out.load(Ordering::Relaxed));

        // 2. Timeout = 0
        let state_zero = InactivityState::new(0, RgbTimeoutPolicy::Always);
        assert!(!state_zero.is_timeout_enabled(true));
        assert!(!state_zero.is_timeout_enabled(false));
        state_zero.last_activity_ms.store(0, Ordering::Relaxed);
        state_zero.check_timeout(&rgb_service, true);
        assert!(!state_zero.is_timed_out.load(Ordering::Relaxed));

        // 3. BatteryOnly on AC
        let state_bat = InactivityState::new(30, RgbTimeoutPolicy::BatteryOnly);
        assert!(!state_bat.is_timeout_enabled(true));
        assert!(state_bat.is_timeout_enabled(false));
        state_bat.last_activity_ms.store(0, Ordering::Relaxed);
        state_bat.check_timeout(&rgb_service, true);
        assert!(!state_bat.is_timed_out.load(Ordering::Relaxed));

        // 4. BatteryOnly on Battery -> Enabled
        state_bat.check_timeout(&rgb_service, false);
        assert!(state_bat.is_timed_out.load(Ordering::Relaxed));
    }

    #[test]
    fn test_reset_activity_timer() {
        let state = InactivityState::new(30, RgbTimeoutPolicy::Always);
        state.is_timed_out.store(true, Ordering::SeqCst);
        state.last_activity_ms.store(100, Ordering::Relaxed);

        state.reset_activity_timer();

        assert!(!state.is_timed_out.load(Ordering::SeqCst));
        assert!(state.last_activity_ms.load(Ordering::Relaxed) > 100);
    }
}
