// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use std::fmt;

use super::types::{
    Action, EventHandled, EventKind, FirmwareModeConfig, FirmwareModeState,
    FirmwareModeStateStatus, decide_action, derive_status,
};
use crate::services::firmware_mode::{
    self, FirmwareMode, FirmwareModeError, read_firmware_mode_privileged, set_firmware_mode,
};

/// Hardware access point for the controller.
///
/// Kept behind a trait so all policy tests run against a fake ECU. The
/// production implementation is [`PrivilegedDriver`].
pub trait FirmwareModeDriver {
    /// Read the actual firmware mode.
    fn read(&self) -> impl std::future::Future<Output = Result<FirmwareMode, String>> + Send;

    /// Write the mode as one verified composite operation.
    fn write(
        &self,
        mode: FirmwareMode,
    ) -> impl std::future::Future<Output = Result<firmware_mode::WriteResult, FirmwareModeError>> + Send;
}

/// Production driver: reads/writes through the fixed, non-injectable `pkexec`
/// batches of the backend.
#[derive(Debug, Clone, Copy, Default)]
pub struct PrivilegedDriver;

impl FirmwareModeDriver for PrivilegedDriver {
    async fn read(&self) -> Result<FirmwareMode, String> {
        read_firmware_mode_privileged().await
    }

    async fn write(
        &self,
        mode: FirmwareMode,
    ) -> Result<firmware_mode::WriteResult, FirmwareModeError> {
        set_firmware_mode(mode).await
    }
}

/// Convenience alias for the production controller.
pub type DefaultController = FirmwareModeController<PrivilegedDriver>;

/// State machine that tracks requested/actual mode and decides event handling.
///
/// Safe by construction: with no requested mode it can never write; auto
/// re-applies are capped at one per event; failures lock the auto path.
pub struct FirmwareModeController<D: FirmwareModeDriver> {
    config: FirmwareModeConfig,
    state: FirmwareModeState,
    reapply_used: bool,
    driver: D,
}

impl<D: FirmwareModeDriver> FirmwareModeController<D> {
    /// Start in pure read-only mode (not managed, no auto-reapply).
    pub fn new(driver: D) -> Self {
        Self::from_config(
            FirmwareModeConfig {
                requested: None,
                auto_reapply: false,
            },
            driver,
        )
    }

    /// Start from a persisted configuration.
    pub fn from_config(config: FirmwareModeConfig, driver: D) -> Self {
        let state = FirmwareModeState {
            requested: config.requested,
            actual: None,
            status: derive_status(config.requested, None),
            auto_locked: false,
            last_error: None,
        };
        Self {
            config,
            state,
            reapply_used: false,
            driver,
        }
    }

    pub fn config(&self) -> &FirmwareModeConfig {
        &self.config
    }

    pub fn state(&self) -> &FirmwareModeState {
        &self.state
    }

    /// The pure decision for `event` without performing it.
    pub fn decide(&self, event: EventKind) -> Action {
        let auto = self.config.auto_reapply && !self.state.auto_locked;
        decide_action(
            self.config.requested,
            self.state.status,
            event,
            auto,
            self.reapply_used,
        )
    }

    /// Handle one event; performs at most one re-apply (see no-fight rules).
    pub async fn handle_event(&mut self, event: EventKind) -> EventHandled {
        self.reapply_used = false;
        match self.decide(event) {
            Action::ReadOnly => {
                self.recapture().await;
                EventHandled::Refreshed
            }
            Action::Reapply => {
                let Some(requested) = self.config.requested else {
                    self.recapture().await;
                    return EventHandled::Refreshed;
                };
                self.reapply_used = true;
                match self.write_once(requested).await {
                    Ok(_) => EventHandled::Reapplied,
                    Err(e) => EventHandled::ReapplyFailed(e),
                }
            }
        }
    }

    /// Change the desired mode.
    ///
    /// `None` switches to pure read-only reporting; `Some(mode)` immediately
    /// performs one write attempt and reports the applied mode. Failures leave
    /// the request in place but never retry from here.
    pub async fn set_requested(
        &mut self,
        requested: Option<FirmwareMode>,
    ) -> Result<FirmwareMode, String> {
        self.config.requested = requested;
        self.state.requested = requested;
        self.state.auto_locked = false;
        self.reapply_used = false;
        match requested {
            None => {
                self.recapture().await;
                Ok(self.state.actual.unwrap_or(FirmwareMode::Unknown(0)))
            }
            Some(mode) => self.write_once(mode).await.map(|wr| wr.actual),
        }
    }

    /// Explicit user-initiated re-apply: writes once, bypassing the auto lock
    /// (this is the "act again" escape hatch from the no-fight rules).
    pub async fn reapply(&mut self) -> Result<firmware_mode::WriteResult, String> {
        let Some(requested) = self.config.requested else {
            return Err("no firmware thermal mode configured".to_string());
        };
        self.state.auto_locked = false;
        self.reapply_used = true;
        self.write_once(requested).await
    }

    /// Pure read-only synchronization, used by the event loop.
    ///
    /// Performs exactly one read of the actual mode and re-derives the status
    /// from `Requested` vs `Actual`. It can never write or re-apply, even when
    /// `auto_reapply` is enabled — the automatic write path stays dormant until
    /// later phases activate it. On a failed read the state is marked `ReadError`
    /// (or stays `Disabled` when unmanaged) and a later event can re-read.
    ///
    /// Returns the freshly read actual mode, or the read error message.
    pub async fn refresh(&mut self) -> Result<FirmwareMode, String> {
        self.recapture().await;
        match &self.state.last_error {
            Some(e) => Err(e.clone()),
            None => Ok(self.state.actual.unwrap_or(FirmwareMode::Unknown(0))),
        }
    }

    /// Re-read the actual mode and update the derived status.
    async fn recapture(&mut self) {
        match self.driver.read().await {
            Ok(actual) => {
                self.state.actual = Some(actual);
                self.state.last_error = None;
                self.state.status = derive_status(self.config.requested, Some(actual));
            }
            Err(e) => {
                self.state.last_error = Some(e);
                if self.config.requested.is_some() {
                    self.state.auto_locked = true;
                    self.state.status = FirmwareModeStateStatus::ReadError;
                } else {
                    self.state.status = FirmwareModeStateStatus::Disabled;
                }
            }
        }
    }

    /// Perform one write, reflect the backend's verified outcome in state.
    async fn write_once(
        &mut self,
        mode: FirmwareMode,
    ) -> Result<firmware_mode::WriteResult, String> {
        self.state.status = FirmwareModeStateStatus::Pending;
        match self.driver.write(mode).await {
            Ok(wr) => {
                self.state.actual = Some(wr.actual);
                self.state.last_error = None;
                self.state.status = match wr.status {
                    firmware_mode::FirmwareModeStatus::Sync => FirmwareModeStateStatus::Sync,
                    firmware_mode::FirmwareModeStatus::Mismatch(actual) => {
                        FirmwareModeStateStatus::Overridden(actual)
                    }
                };
                Ok(wr)
            }
            Err(e) => {
                self.state.last_error = Some(e.to_string());
                self.state.auto_locked = true;
                self.state.status = match &e {
                    FirmwareModeError::Rejected(_)
                    | FirmwareModeError::Unsupported(_)
                    | FirmwareModeError::UnsupportedHardware(_) => {
                        FirmwareModeStateStatus::Rejected
                    }
                    FirmwareModeError::Io(_)
                    | FirmwareModeError::Parse(_)
                    | FirmwareModeError::ReadError(_) => FirmwareModeStateStatus::ReadError,
                };
                Err(e.to_string())
            }
        }
    }
}

impl<D: FirmwareModeDriver> fmt::Debug for FirmwareModeController<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FirmwareModeController")
            .field("config", &self.config)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}
