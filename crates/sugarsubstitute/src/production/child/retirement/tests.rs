// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify the real transaction queue and immutable cleanup budget without native authority fakes.
use super::*;
#[test]
fn passive_startup_capture_diagnostic_classifies_original_snapshots_without_a_new_budget()
-> Result<()> {
    use sugarsubstitute_desktop::StartupCaptureState as State;
    let budget = ShutdownBudget::new(Moment::from_nanos(50))?;
    let cause = sugarsubstitute_execution::windows::NativeObservationError::new(
        "counter_deadline_conversion",
        std::io::Error::other("original private counter"),
    );
    for (snapshot, expected) in [
        (None, State::Absent),
        (Some(CaptureCleanupSnapshot::Dormant), State::Dormant),
        (Some(CaptureCleanupSnapshot::Requested), State::Requested),
        (
            Some(CaptureCleanupSnapshot::Latched { budget, counter: Err(cause.clone()) }),
            State::CounterFailed,
        ),
        (Some(CaptureCleanupSnapshot::ClockUnavailable(cause)), State::ClockUnavailable),
    ] {
        assert_eq!(diagnostic::capture_category(snapshot), expected);
    }
    assert_eq!(budget, ShutdownBudget::new(Moment::from_nanos(50))?);
    Ok(())
}

#[test]
fn direct_startup_close_causes_keep_sealed_native_and_request_categories_without_formatting()
-> Result<()> {
    use sugarsubstitute_desktop::{
        StartupCauseCode as Code, StartupNativeCause as Native, StartupRequestCause as Request,
    };
    let known = sugarsubstitute_execution::windows::NativeObservationError::new(
        "execution_qpc_counter",
        std::io::Error::from_raw_os_error(233),
    );
    assert_eq!(diagnostic::cause_code(&known), Code::Native(Native::CounterQuery, Some(233)));
    assert_eq!(
        diagnostic::cause_code(&bootstrap::ChildBootstrapIssue::Transport),
        Code::Bootstrap(sugarsubstitute_desktop::StartupBootstrapCause::Transport)
    );
    let private = sugarsubstitute_execution::windows::NativeObservationError::new(
        "PRIVATE_UNREGISTERED_STAGE",
        std::io::Error::other("PRIVATE_PAYLOAD"),
    );
    assert_eq!(diagnostic::cause_code(&private), Code::Native(Native::Other, None));
    assert_eq!(
        diagnostic::cause_code(&requests::RequestError::Deadline),
        Code::Request(Request::Deadline)
    );
    assert_eq!(
        diagnostic::cause_code(&std::io::Error::from(std::io::ErrorKind::TimedOut)),
        Code::TimedOut
    );
    assert_eq!(known.stage(), "execution_qpc_counter");
    Ok(())
}
#[test]
fn earlier_native_budget_keeps_complete_start_grace_and_total() -> Result<()> {
    let first = ShutdownBudget::new(Moment::from_nanos(10))?;
    let later = ShutdownBudget::new(Moment::from_nanos(20))?;
    let mut state = ChildShutdown::new();
    state.retain_budget(later);
    state.retain_budget(first);
    state.retain_budget(later);
    assert_eq!(state.budget, Some(first));
    assert!(!state.native_retired);
    assert!(!state.intent_done);
    assert!(!state.completed_done);
    Ok(())
}
#[test]
fn four_control_slots_reject_overload_without_erasing_original_fifo() -> Result<()> {
    let mut state = ChildShutdown::new();
    for value in 1..=4 {
        state.queue(Control::Activity {
            revision: NonZeroU64::new(value).ok_or("Revision missing")?,
            activity: UiWatchdogActivity::VisibleActive,
        })?;
    }
    assert!(state.queue(Control::ExitCompleted).is_err());
    for value in 1..=4 {
        assert!(
            matches!(take_first(&mut state.queued),Some(Control::Activity{revision,..}) if revision.get()==value)
        );
    }
    assert!(take_first(&mut state.queued).is_none());
    assert!(!state.native_retired);
    Ok(())
}
#[test]
fn queued_exit_intent_and_completion_never_prove_acknowledgement_or_retirement() -> Result<()> {
    let mut state = ChildShutdown::new();
    state.queue(Control::ExitIntent)?;
    state.queue(Control::ExitCompleted)?;
    assert!(matches!(take_first(&mut state.queued), Some(Control::ExitIntent)));
    assert!(matches!(take_first(&mut state.queued), Some(Control::ExitCompleted)));
    assert!(!state.intent_done);
    assert!(!state.completed_done);
    assert!(!state.native_retired);
    assert!(state.budget.is_none());
    Ok(())
}

#[test]
fn pending_capture_is_unmeasured_and_actual_counter_failure_keeps_original_budget() -> Result<()> {
    let mut shutdown = ChildShutdown::new();
    assert_eq!(
        shutdown.observe_capture(Some(CaptureCleanupSnapshot::Requested))?,
        ShutdownInitiation::CapturePending
    );
    assert!(shutdown.budget.is_none());
    let budget = ShutdownBudget::new(Moment::from_nanos(50))?;
    let cause = sugarsubstitute_execution::windows::NativeObservationError::new(
        "capture_cleanup_counter",
        std::io::Error::other("original counter failure"),
    );
    let failure = shutdown
        .observe_capture(Some(CaptureCleanupSnapshot::Latched { budget, counter: Err(cause) }))
        .err()
        .ok_or("Actual counter failure lost")?;
    assert_eq!(shutdown.budget, Some(budget));
    assert_eq!(
        failure
            .downcast_ref::<sugarsubstitute_execution::windows::NativeObservationError>()
            .ok_or("Actual native type lost")?
            .stage(),
        "capture_cleanup_counter"
    );
    assert_eq!(shutdown.observe_capture(None)?, ShutdownInitiation::Captured(budget));
    Ok(())
}
