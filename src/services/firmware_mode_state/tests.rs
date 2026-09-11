use super::*;
use crate::services::firmware_mode::{self, FirmwareMode, FirmwareModeError};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const ALL_EVENTS: [EventKind; 5] = [
    EventKind::Startup,
    EventKind::ProfileChanged,
    EventKind::AcBatteryChanged,
    EventKind::AsusdRestarted,
    EventKind::Resume,
];

// ── derive_status ────────────────────────────────────────────────────────

#[test]
fn status_disabled_without_request() {
    assert_eq!(
        derive_status(None, Some(FirmwareMode::High)),
        FirmwareModeStateStatus::Disabled
    );
    assert_eq!(derive_status(None, None), FirmwareModeStateStatus::Disabled);
}

#[test]
fn status_unknown_without_actual() {
    assert_eq!(
        derive_status(Some(FirmwareMode::High), None),
        FirmwareModeStateStatus::Unknown
    );
}

#[test]
fn status_sync_when_equal() {
    assert_eq!(
        derive_status(Some(FirmwareMode::High), Some(FirmwareMode::High)),
        FirmwareModeStateStatus::Sync
    );
}

#[test]
fn status_overridden_when_different() {
    assert_eq!(
        derive_status(Some(FirmwareMode::Quiet), Some(FirmwareMode::Full)),
        FirmwareModeStateStatus::Overridden(FirmwareMode::Full)
    );
}

// ── decide_action matrix ─────────────────────────────────────────────────

#[test]
fn decide_always_read_only_without_request() {
    for event in ALL_EVENTS {
        for auto in [false, true] {
            for used in [false, true] {
                assert_eq!(
                    decide_action(None, FirmwareModeStateStatus::Disabled, event, auto, used),
                    Action::ReadOnly,
                    "event={event:?} auto={auto} used={used}"
                );
            }
        }
    }
}

#[test]
fn decide_never_reapplies_when_auto_disabled() {
    for event in ALL_EVENTS {
        assert_eq!(
            decide_action(
                Some(FirmwareMode::High),
                FirmwareModeStateStatus::Overridden(FirmwareMode::Quiet),
                event,
                false,
                false
            ),
            Action::ReadOnly,
            "event={event:?}"
        );
    }
}

#[test]
fn decide_startup_never_reapplies_even_with_auto() {
    assert_eq!(
        decide_action(
            Some(FirmwareMode::High),
            FirmwareModeStateStatus::Overridden(FirmwareMode::Quiet),
            EventKind::Startup,
            true,
            false
        ),
        Action::ReadOnly
    );
}

#[test]
fn decide_reapplies_on_overridden_event_with_auto() {
    for event in [
        EventKind::ProfileChanged,
        EventKind::AcBatteryChanged,
        EventKind::AsusdRestarted,
        EventKind::Resume,
    ] {
        assert_eq!(
            decide_action(
                Some(FirmwareMode::High),
                FirmwareModeStateStatus::Overridden(FirmwareMode::Quiet),
                event,
                true,
                false
            ),
            Action::Reapply,
            "event={event:?}"
        );
    }
}

#[test]
fn decide_reapply_capped_once_per_event() {
    assert_eq!(
        decide_action(
            Some(FirmwareMode::High),
            FirmwareModeStateStatus::Overridden(FirmwareMode::Quiet),
            EventKind::ProfileChanged,
            true,
            true
        ),
        Action::ReadOnly
    );
}

#[test]
fn decide_never_reapplies_on_rejected() {
    for event in ALL_EVENTS {
        assert_eq!(
            decide_action(
                Some(FirmwareMode::High),
                FirmwareModeStateStatus::Rejected,
                event,
                true,
                false
            ),
            Action::ReadOnly,
            "event={event:?}"
        );
    }
}

#[test]
fn decide_never_reapplies_on_read_error() {
    for event in ALL_EVENTS {
        assert_eq!(
            decide_action(
                Some(FirmwareMode::High),
                FirmwareModeStateStatus::ReadError,
                event,
                true,
                false
            ),
            Action::ReadOnly,
            "event={event:?}"
        );
    }
}

#[test]
fn decide_never_reapplies_on_unknown() {
    for event in ALL_EVENTS {
        assert_eq!(
            decide_action(
                Some(FirmwareMode::High),
                FirmwareModeStateStatus::Unknown,
                event,
                true,
                false
            ),
            Action::ReadOnly,
            "event={event:?}"
        );
    }
}

#[test]
fn decide_never_reapplies_on_sync() {
    for event in [EventKind::ProfileChanged, EventKind::Resume] {
        assert_eq!(
            decide_action(
                Some(FirmwareMode::High),
                FirmwareModeStateStatus::Sync,
                event,
                true,
                false
            ),
            Action::ReadOnly,
            "event={event:?}"
        );
    }
}

// ── controller integration (fake ECU driver) ─────────────────────────────

struct FakeEcu {
    mode: Mutex<FirmwareMode>,
    reads: AtomicUsize,
    write_calls: AtomicUsize,
    writes: Mutex<Vec<FirmwareMode>>,
    fail_reads: AtomicUsize,
    reject_writes: AtomicUsize,
}

impl Default for FakeEcu {
    fn default() -> Self {
        Self {
            mode: Mutex::new(FirmwareMode::Balanced),
            reads: AtomicUsize::new(0),
            write_calls: AtomicUsize::new(0),
            writes: Mutex::new(Vec::new()),
            fail_reads: AtomicUsize::new(0),
            reject_writes: AtomicUsize::new(0),
        }
    }
}

#[derive(Clone)]
struct FakeDriver {
    ecu: Arc<FakeEcu>,
}

impl FakeDriver {
    fn new(mode: FirmwareMode) -> Self {
        Self {
            ecu: Arc::new(FakeEcu {
                mode: Mutex::new(mode),
                ..Default::default()
            }),
        }
    }
    fn read_calls(&self) -> usize {
        self.ecu.reads.load(Ordering::SeqCst)
    }
    fn write_calls(&self) -> usize {
        self.ecu.write_calls.load(Ordering::SeqCst)
    }
    fn written(&self) -> Vec<FirmwareMode> {
        self.ecu.writes.lock().unwrap().clone()
    }
    fn poke(&self, mode: FirmwareMode) {
        *self.ecu.mode.lock().unwrap() = mode;
    }
    fn fail_next_reads(&self, n: usize) {
        self.ecu.fail_reads.store(n, Ordering::SeqCst);
    }
    fn reject_next_writes(&self, n: usize) {
        self.ecu.reject_writes.store(n, Ordering::SeqCst);
    }
}

fn fake_write_result(mode: FirmwareMode) -> Result<firmware_mode::WriteResult, FirmwareModeError> {
    Ok(firmware_mode::WriteResult {
        requested: mode,
        ctrl_param: mode.write_param().unwrap_or(0),
        devs_retval: 1,
        dsts: u32::from(mode.nibble()),
        actual: mode,
        status: firmware_mode::FirmwareModeStatus::Sync,
    })
}

impl FirmwareModeDriver for FakeDriver {
    async fn read(&self) -> Result<FirmwareMode, String> {
        self.ecu.reads.fetch_add(1, Ordering::SeqCst);
        if self.ecu.fail_reads.load(Ordering::SeqCst) > 0 {
            self.ecu.fail_reads.fetch_sub(1, Ordering::SeqCst);
            return Err("fake read failure".to_string());
        }
        Ok(*self.ecu.mode.lock().unwrap())
    }
    async fn write(
        &self,
        mode: FirmwareMode,
    ) -> Result<firmware_mode::WriteResult, FirmwareModeError> {
        self.ecu.write_calls.fetch_add(1, Ordering::SeqCst);
        if self.ecu.reject_writes.load(Ordering::SeqCst) > 0 {
            self.ecu.reject_writes.fetch_sub(1, Ordering::SeqCst);
            return Err(FirmwareModeError::Rejected("fake rejection".to_string()));
        }
        self.ecu.writes.lock().unwrap().push(mode);
        *self.ecu.mode.lock().unwrap() = mode;
        fake_write_result(mode)
    }
}

#[tokio::test]
async fn never_writes_when_not_managed() {
    let driver = FakeDriver::new(FirmwareMode::High);
    let mut ctl = FirmwareModeController::from_config(
        FirmwareModeConfig {
            requested: None,
            auto_reapply: true, // even with auto on
        },
        driver.clone(),
    );

    for event in ALL_EVENTS {
        ctl.handle_event(event).await;
    }

    assert_eq!(driver.write_calls(), 0, "disabled must never write");
    assert_eq!(ctl.state().status, FirmwareModeStateStatus::Disabled);
    assert_eq!(ctl.state().actual, Some(FirmwareMode::High));
}

#[tokio::test]
async fn startup_never_writes_even_with_auto() {
    let driver = FakeDriver::new(FirmwareMode::Quiet);
    let mut ctl = FirmwareModeController::from_config(
        FirmwareModeConfig {
            requested: Some(FirmwareMode::High),
            auto_reapply: true,
        },
        driver.clone(),
    );

    let handled = ctl.handle_event(EventKind::Startup).await;

    assert_eq!(handled, EventHandled::Refreshed);
    assert_eq!(driver.write_calls(), 0);
    assert_eq!(
        ctl.state().status,
        FirmwareModeStateStatus::Overridden(FirmwareMode::Quiet)
    );
}

#[tokio::test]
async fn set_requested_writes_once() {
    let driver = FakeDriver::new(FirmwareMode::Balanced);
    let mut ctl = FirmwareModeController::new(driver.clone());

    let applied = ctl.set_requested(Some(FirmwareMode::High)).await.unwrap();

    assert_eq!(applied, FirmwareMode::High);
    assert_eq!(driver.write_calls(), 1);
    assert_eq!(driver.written(), vec![FirmwareMode::High]);
    assert_eq!(ctl.state().status, FirmwareModeStateStatus::Sync);
}

#[tokio::test]
async fn read_only_event_reflects_external_change() {
    let driver = FakeDriver::new(FirmwareMode::High);
    let mut ctl = FirmwareModeController::from_config(
        FirmwareModeConfig {
            requested: Some(FirmwareMode::High),
            auto_reapply: false,
        },
        driver.clone(),
    );
    ctl.set_requested(Some(FirmwareMode::High)).await.unwrap();

    driver.poke(FirmwareMode::Quiet); // something else changed the mode
    let handled = ctl.handle_event(EventKind::ProfileChanged).await;

    assert_eq!(handled, EventHandled::Refreshed);
    // auto off: event must only re-read, never write.
    assert_eq!(driver.write_calls(), 1);
    assert_eq!(
        ctl.state().status,
        FirmwareModeStateStatus::Overridden(FirmwareMode::Quiet)
    );
}

#[tokio::test]
async fn auto_reapply_restores_override_once() {
    let driver = FakeDriver::new(FirmwareMode::High);
    let mut ctl = FirmwareModeController::from_config(
        FirmwareModeConfig {
            requested: Some(FirmwareMode::High),
            auto_reapply: true,
        },
        driver.clone(),
    );
    ctl.set_requested(Some(FirmwareMode::High)).await.unwrap();

    driver.poke(FirmwareMode::Quiet);
    // First event only detects the divergence (read-only recapture)...
    assert_eq!(
        ctl.handle_event(EventKind::ProfileChanged).await,
        EventHandled::Refreshed
    );
    assert_eq!(
        ctl.state().status,
        FirmwareModeStateStatus::Overridden(FirmwareMode::Quiet)
    );
    // ...the next event is allowed to re-apply, exactly once.
    let before = driver.write_calls();
    let handled = ctl.handle_event(EventKind::AcBatteryChanged).await;

    assert_eq!(handled, EventHandled::Reapplied);
    assert_eq!(driver.write_calls(), before + 1, "exactly one write");
    assert_eq!(ctl.state().status, FirmwareModeStateStatus::Sync);
}

#[tokio::test]
async fn failed_write_locks_auto_and_never_retries_on_its_own() {
    let driver = FakeDriver::new(FirmwareMode::Quiet);
    let mut ctl = FirmwareModeController::from_config(
        FirmwareModeConfig {
            requested: Some(FirmwareMode::High),
            auto_reapply: true,
        },
        driver.clone(),
    );
    driver.poke(FirmwareMode::Quiet);
    ctl.handle_event(EventKind::ProfileChanged).await; // detect override
    assert_eq!(
        ctl.state().status,
        FirmwareModeStateStatus::Overridden(FirmwareMode::Quiet)
    );
    driver.reject_next_writes(99);
    ctl.handle_event(EventKind::AcBatteryChanged).await; // reapply fails

    let calls_after_failure = driver.write_calls();
    assert!(matches!(
        ctl.state().status,
        FirmwareModeStateStatus::Rejected
    ));
    assert!(ctl.state().auto_locked);

    // Subsequent events never auto-write again.
    for event in ALL_EVENTS {
        ctl.handle_event(event).await;
    }
    assert_eq!(
        driver.write_calls(),
        calls_after_failure,
        "auto path must stay locked"
    );
}

#[tokio::test]
async fn explicit_reapply_unlocks_and_writes_once() {
    let driver = FakeDriver::new(FirmwareMode::Quiet);
    let mut ctl = FirmwareModeController::from_config(
        FirmwareModeConfig {
            requested: Some(FirmwareMode::High),
            auto_reapply: true,
        },
        driver.clone(),
    );
    driver.poke(FirmwareMode::Quiet);
    ctl.handle_event(EventKind::ProfileChanged).await; // detect override
    driver.reject_next_writes(1);
    ctl.handle_event(EventKind::AcBatteryChanged).await; // auto reapply fails
    let calls_before = driver.write_calls();
    assert!(ctl.state().auto_locked);

    let result = ctl.reapply().await; // user explicitly acts again

    assert_eq!(driver.write_calls(), calls_before + 1);
    assert!(!ctl.state().auto_locked);
    assert_eq!(ctl.state().status, FirmwareModeStateStatus::Sync);
    assert!(result.is_ok());
}

#[tokio::test]
async fn read_error_marks_state_and_read_is_retried_on_later_events() {
    let driver = FakeDriver::new(FirmwareMode::High);
    let mut ctl = FirmwareModeController::from_config(
        FirmwareModeConfig {
            requested: None,
            auto_reapply: false,
        },
        driver.clone(),
    );
    driver.fail_next_reads(1);

    let handled = ctl.handle_event(EventKind::AsusdRestarted).await;

    assert_eq!(handled, EventHandled::Refreshed);
    assert!(ctl.state().last_error.is_some());

    // Next event succeeds again and refreshes actual.
    ctl.handle_event(EventKind::ProfileChanged).await;
    assert_eq!(ctl.state().actual, Some(FirmwareMode::High));
    assert_eq!(ctl.state().last_error, None);
}

#[tokio::test]
async fn set_requested_none_turns_read_only() {
    let driver = FakeDriver::new(FirmwareMode::Full);
    let mut ctl = FirmwareModeController::from_config(
        FirmwareModeConfig {
            requested: Some(FirmwareMode::High),
            auto_reapply: true,
        },
        driver.clone(),
    );
    ctl.set_requested(Some(FirmwareMode::High)).await.unwrap();
    let after_write = driver.write_calls();

    let actual = ctl.set_requested(None).await.unwrap();

    assert_eq!(actual, FirmwareMode::High);
    assert_eq!(driver.write_calls(), after_write, "no write for None");
    assert_eq!(ctl.state().status, FirmwareModeStateStatus::Disabled);

    for event in ALL_EVENTS {
        ctl.handle_event(event).await;
    }
    assert_eq!(driver.write_calls(), after_write);
}

#[tokio::test]
async fn pending_state_is_produced_during_write() {
    let driver = FakeDriver::new(FirmwareMode::Balanced);
    let mut ctl = FirmwareModeController::new(driver.clone());

    ctl.set_requested(Some(FirmwareMode::Full)).await.unwrap();

    // Post-write the state must be terminal (Sync), never left Pending.
    assert_ne!(ctl.state().status, FirmwareModeStateStatus::Pending);
    assert_eq!(ctl.state().status, FirmwareModeStateStatus::Sync);
}

// ── read-only event-loop sync (Phase 3C: refresh never writes) ───────────

#[tokio::test]
async fn refresh_one_read_per_event_and_zero_writes() {
    let driver = FakeDriver::new(FirmwareMode::High);
    let mut ctl = FirmwareModeController::from_config(
        FirmwareModeConfig {
            requested: Some(FirmwareMode::High),
            auto_reapply: true, // the no-fight guarantee must hold even so
        },
        driver.clone(),
    );

    // One fresh read per event, exactly, and never a write.
    for event in ALL_EVENTS {
        let before = driver.read_calls();
        ctl.refresh().await.unwrap();
        assert_eq!(
            driver.read_calls(),
            before + 1,
            "event {event:?} must trigger exactly one read"
        );
    }
    assert_eq!(driver.write_calls(), 0, "no event may cause a write");
    assert_eq!(ctl.state().status, FirmwareModeStateStatus::Sync);
}

#[tokio::test]
async fn refresh_sync_when_requested_matches_actual() {
    let driver = FakeDriver::new(FirmwareMode::High);
    let mut ctl = FirmwareModeController::from_config(
        FirmwareModeConfig {
            requested: Some(FirmwareMode::High),
            auto_reapply: true,
        },
        driver.clone(),
    );

    let actual = ctl.refresh().await.unwrap();

    assert_eq!(actual, FirmwareMode::High);
    assert_eq!(ctl.state().actual, Some(FirmwareMode::High));
    assert_eq!(ctl.state().status, FirmwareModeStateStatus::Sync);
    assert_eq!(driver.write_calls(), 0);
}

#[tokio::test]
async fn refresh_overridden_when_actual_differs_without_writes() {
    let driver = FakeDriver::new(FirmwareMode::Quiet);
    let mut ctl = FirmwareModeController::from_config(
        FirmwareModeConfig {
            requested: Some(FirmwareMode::High),
            auto_reapply: true, // even with auto on, refresh never writes
        },
        driver.clone(),
    );

    return {
        let actual = ctl.refresh().await.unwrap();
        assert_eq!(actual, FirmwareMode::Quiet);
        assert_eq!(
            ctl.state().status,
            FirmwareModeStateStatus::Overridden(FirmwareMode::Quiet)
        );
        assert_eq!(driver.write_calls(), 0, "override must not auto-write");
    };
}

#[tokio::test]
async fn refresh_disabled_without_request_still_displays_actual() {
    let driver = FakeDriver::new(FirmwareMode::Full);
    let mut ctl = FirmwareModeController::from_config(
        FirmwareModeConfig {
            requested: None,
            auto_reapply: true,
        },
        driver.clone(),
    );

    let actual = ctl.refresh().await.unwrap();

    assert_eq!(actual, FirmwareMode::Full);
    assert_eq!(ctl.state().status, FirmwareModeStateStatus::Disabled);
    assert_eq!(ctl.state().actual, Some(FirmwareMode::Full));
    assert_eq!(driver.write_calls(), 0);
}

#[tokio::test]
async fn refresh_unknown_actual_state_before_first_read() {
    let driver = FakeDriver::new(FirmwareMode::High);
    let ctl = FirmwareModeController::from_config(
        FirmwareModeConfig {
            requested: Some(FirmwareMode::High),
            auto_reapply: false,
        },
        driver,
    );

    // Nothing read yet: the managed-but-unread state is Unknown.
    assert_eq!(ctl.state().actual, None);
    assert_eq!(ctl.state().status, FirmwareModeStateStatus::Unknown);
}

#[tokio::test]
async fn refresh_read_failure_marks_state_and_recovers() {
    let driver = FakeDriver::new(FirmwareMode::High);
    let mut ctl = FirmwareModeController::from_config(
        FirmwareModeConfig {
            requested: Some(FirmwareMode::High),
            auto_reapply: false,
        },
        driver.clone(),
    );
    driver.fail_next_reads(1);

    let result = ctl.refresh().await;
    assert!(result.is_err());
    assert_eq!(ctl.state().status, FirmwareModeStateStatus::ReadError);
    assert!(ctl.state().last_error.is_some());
    assert_eq!(driver.write_calls(), 0);

    // A later event re-reads and recovers the actual mode.
    let actual = ctl.refresh().await.unwrap();
    assert_eq!(actual, FirmwareMode::High);
    assert_eq!(ctl.state().status, FirmwareModeStateStatus::Sync);
    assert_eq!(ctl.state().last_error, None);
}

#[tokio::test]
async fn refresh_read_failure_unmanaged_stays_disabled() {
    let driver = FakeDriver::new(FirmwareMode::High);
    let mut ctl = FirmwareModeController::new(driver.clone());
    driver.fail_next_reads(1);

    let result = ctl.refresh().await;

    assert!(result.is_err());
    assert_eq!(ctl.state().status, FirmwareModeStateStatus::Disabled);
    assert_eq!(driver.write_calls(), 0);
}

#[tokio::test]
async fn refresh_repeated_events_never_create_write_loop() {
    let driver = FakeDriver::new(FirmwareMode::Balanced);
    let mut ctl = FirmwareModeController::from_config(
        FirmwareModeConfig {
            requested: Some(FirmwareMode::High),
            auto_reapply: true,
        },
        driver.clone(),
    );

    // Simulate a burst of events with the actual persistently diverged.
    for _ in 0..50 {
        ctl.refresh().await.unwrap();
    }

    assert_eq!(driver.read_calls(), 50, "one read per event");
    assert_eq!(driver.write_calls(), 0, "no retry/write loop, ever");
    assert_eq!(
        ctl.state().status,
        FirmwareModeStateStatus::Overridden(FirmwareMode::Balanced)
    );
}

#[tokio::test]
async fn refresh_asusd_restart_recovers_with_fresh_read() {
    let driver = FakeDriver::new(FirmwareMode::High);
    let mut ctl = FirmwareModeController::from_config(
        FirmwareModeConfig {
            requested: Some(FirmwareMode::High),
            auto_reapply: false,
        },
        driver.clone(),
    );

    // asusd restarts while the firmware is running a different mode.
    driver.poke(FirmwareMode::Quiet);
    let actual = ctl.refresh().await.unwrap();
    assert_eq!(actual, FirmwareMode::Quiet);
    assert_eq!(
        ctl.state().status,
        FirmwareModeStateStatus::Overridden(FirmwareMode::Quiet)
    );

    // asusd becomes available again and the scheduler re-reads: Sync.
    driver.poke(FirmwareMode::High);
    let actual = ctl.refresh().await.unwrap();
    assert_eq!(actual, FirmwareMode::High);
    assert_eq!(ctl.state().status, FirmwareModeStateStatus::Sync);
    assert_eq!(driver.write_calls(), 0);
}

#[tokio::test]
async fn refresh_startup_is_read_only() {
    let driver = FakeDriver::new(FirmwareMode::Full);
    let mut ctl = FirmwareModeController::from_config(
        FirmwareModeConfig {
            requested: Some(FirmwareMode::Full),
            auto_reapply: true, // must still not write at startup
        },
        driver.clone(),
    );

    let handled = { ctl.refresh().await.unwrap() };

    assert_eq!(handled, FirmwareMode::Full);
    assert_eq!(driver.write_calls(), 0, "startup stays read-only");
    assert_eq!(driver.read_calls(), 1);
    assert_eq!(ctl.state().status, FirmwareModeStateStatus::Sync);
}
