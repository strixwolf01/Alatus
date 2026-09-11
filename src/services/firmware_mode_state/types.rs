// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::services::firmware_mode::FirmwareMode;

/// External event that may have changed the EC thermal mode out from under us.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    /// Program start / first read-only snapshot.
    Startup,
    /// External profile changed.
    ProfileChanged,
    /// Power source changed (AC <-> battery per UPower device signals).
    AcBatteryChanged,
    /// The `xyz.ljones.Asusd` service (re)appeared or disappeared.
    AsusdRestarted,
    /// The system resumed from suspend.
    Resume,
}

/// Action deduced from the current state for a given event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Re-read the actual mode and refresh the displayed state.
    ReadOnly,
    /// Write the requested mode once and re-verify.
    Reapply,
}

/// Result of handling one event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventHandled {
    /// State was refreshed from a read.
    Refreshed,
    /// The requested mode was written once and verified.
    Reapplied,
    /// A re-apply write was attempted but failed.
    ReapplyFailed(String),
}

/// UI-facing status of the firmware thermal mode feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FirmwareModeStateStatus {
    /// Not managed: no requested mode configured (pure read-only reporting).
    #[default]
    Disabled,
    /// Requested mode matches what the firmware reports.
    Sync,
    /// A write is in flight.
    Pending,
    /// The firmware is in a mode that differs from the requested one.
    Overridden(FirmwareMode),
    /// The last write was refused by the firmware/kernel or denied by pkexec.
    Rejected,
    /// The last read or readback failed (e.g. no root / debugfs missing).
    ReadError,
    /// We have never successfully read a mode yet.
    Unknown,
}

/// Persisted/configured request for the feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FirmwareModeConfig {
    /// Desired mode; `None` = read-only ("not managed").
    pub requested: Option<FirmwareMode>,
    /// Allow at most one re-apply write per relevant event.
    pub auto_reapply: bool,
}

/// Snapshot of the feature's runtime state for the UI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FirmwareModeState {
    /// The mode the user asked for (`None` = read-only).
    pub requested: Option<FirmwareMode>,
    /// Last trustworthy readback from the EC.
    pub actual: Option<FirmwareMode>,
    /// Derived UI label.
    pub status: FirmwareModeStateStatus,
    /// True once a write/read failed; the auto path stays disabled until the
    /// user explicitly acts again (see no-fight rules).
    pub auto_locked: bool,
    /// Human-readable reason for the last `Rejected` / `ReadError`.
    pub last_error: Option<String>,
}

/// Pure derivation of the status from in/out state.
pub fn derive_status(
    requested: Option<FirmwareMode>,
    actual: Option<FirmwareMode>,
) -> FirmwareModeStateStatus {
    match (requested, actual) {
        (None, _) => FirmwareModeStateStatus::Disabled,
        (Some(_), None) => FirmwareModeStateStatus::Unknown,
        (Some(req), Some(act)) if req == act => FirmwareModeStateStatus::Sync,
        (Some(_), Some(act)) => FirmwareModeStateStatus::Overridden(act),
    }
}

/// Pure decision matrix: what one event may do.
///
/// Every branch ends in either `ReadOnly` or `Reapply`; there is no
/// "do nothing" case because a read-only refresh is always the safe fallback.
/// `reapply_used` implements the "at most one re-apply per event" cap.
pub fn decide_action(
    requested: Option<FirmwareMode>,
    status: FirmwareModeStateStatus,
    event: EventKind,
    auto_reapply: bool,
    reapply_used: bool,
) -> Action {
    if requested.is_none() {
        return Action::ReadOnly;
    }
    if !auto_reapply {
        return Action::ReadOnly;
    }
    if matches!(event, EventKind::Startup) {
        return Action::ReadOnly;
    }
    match status {
        FirmwareModeStateStatus::Overridden(_) if !reapply_used => Action::Reapply,
        _ => Action::ReadOnly,
    }
}
