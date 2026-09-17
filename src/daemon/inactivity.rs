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

    pub fn check_timeout(&self, rgb_service: &crate::services::rgb::RgbService, on_ac: bool) {
        let policy = self.get_policy();
        match policy {
            RgbTimeoutPolicy::Never => {
                self.wake_if_timed_out(rgb_service);
                return;
            }
            RgbTimeoutPolicy::BatteryOnly => {
                if on_ac {
                    self.wake_if_timed_out(rgb_service);
                    return;
                }
            }
            RgbTimeoutPolicy::Always => {}
        }

        let timeout = self.timeout_seconds.load(Ordering::Relaxed);
        if timeout == 0 {
            self.wake_if_timed_out(rgb_service);
            return;
        }
        if self.is_timed_out.load(Ordering::Relaxed) {
            return;
        }
        let elapsed_ms =
            Self::current_ms().saturating_sub(self.last_activity_ms.load(Ordering::Relaxed));
        if elapsed_ms >= (timeout as u64 * 1000) && !self.is_timed_out.swap(true, Ordering::SeqCst)
        {
            tracing::info!(
                "RGB inactivity timeout of {timeout}s reached (policy={policy:?}, on_ac={on_ac}). Dimming backlight."
            );
            let _ = rgb_service.sleep_timeout();
        }
    }
}
