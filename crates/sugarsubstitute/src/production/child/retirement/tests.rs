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

#[test]
fn whole_close_intent_waits_for_actual_registration_but_not_capture_clock_measurement() -> Result<()>
{
    let mut state = ChildShutdown::new();
    state.intent = Some(CompletionIntent::Normal);
    for _ in 0..4 {
        state.request_exit_intent(false)?;
    }
    assert!(state.shutdown.is_none());
    assert!(state.queued.iter().all(Option::is_none));
    assert_eq!(
        state.observe_capture(Some(CaptureCleanupSnapshot::Requested))?,
        ShutdownInitiation::CapturePending
    );
    state.request_exit_intent(true)?;
    let id = state.shutdown.ok_or("Original shutdown identity absent")?;
    assert!(state.budget.is_none());
    assert!(matches!(take_first(&mut state.queued), Some(Control::ExitIntent)));
    // A copied exact intent-ACK disposition still supplies no measured capture budget.
    state.intent_done = true;
    assert!(!state.completion_ready(false));
    assert!(!state.completion_ready(true));
    // Cached preparation is an inert external observation here; native admission remains false.
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    };
    let header = WireHeader {
        run,
        connection: Some(ConnectionId::new(1)?),
        request: Some(RequestId::new(1)?),
        deadline_qpc: 300,
    };
    let control_until = Moment::from_nanos(30_000_000_000);
    state.pending = Some(Pending {
        header,
        original: control_until,
        control: Control::ExitIntent,
        admitted: false,
    });
    assert_eq!(state.deadline(), Some(control_until));
    for _ in 0..4 {
        state.request_exit_intent(true)?;
    }
    assert!(state.queued.iter().all(Option::is_none));
    let original = ShutdownBudget::new(Moment::from_nanos(50))?;
    let original_counter_failure = sugarsubstitute_execution::windows::NativeObservationError::new(
        "capture_cleanup_counter",
        std::io::Error::other("retained original counter fixture"),
    );
    assert!(
        state
            .observe_capture(Some(CaptureCleanupSnapshot::Latched {
                budget: original,
                counter: Err(original_counter_failure),
            }))
            .is_err()
    );
    assert_eq!(state.observe_capture(None)?, ShutdownInitiation::Captured(original));
    state.request_exit_intent(true)?;
    assert_eq!(state.shutdown, Some(id));
    assert_eq!(state.budget, Some(original));
    assert!(state.completion_ready(false));
    assert!(!state.completion_ready(true));
    state.released = true;
    assert!(state.completion_ready(true));
    let retained = state.pending.ok_or("Original prepared intent disappeared")?;
    assert_eq!(retained.header, header);
    assert_eq!(retained.original, control_until);
    assert!(!retained.admitted);
    assert!(original.settle_until < control_until);
    assert_eq!(state.deadline(), Some(original.settle_until));
    let earlier_cutoff = Moment::from_nanos(1);
    let mut earlier_pending = ChildShutdown::new();
    earlier_pending.pending = Some(Pending { original: earlier_cutoff, ..retained });
    earlier_pending.retain_budget(original);
    assert_eq!(earlier_pending.deadline(), Some(earlier_cutoff));
    assert_eq!(earlier_pending.budget, Some(original));
    assert_eq!(earlier_pending.pending.ok_or("Earlier prepared request absent")?.header, header);
    assert!(state.queued.iter().all(Option::is_none));
    assert!(state.intent_done);
    assert!(!state.completed_done);
    assert!(!state.native_retired);
    Ok(())
}
#[test]
fn bounded_early_intent_rejection_keeps_queue_and_unassigned_identity_for_actual_retry()
-> Result<()> {
    let mut state = ChildShutdown::new();
    state.intent = Some(CompletionIntent::Normal);
    for value in 1..=4 {
        state.queue(Control::Activity {
            revision: NonZeroU64::new(value).ok_or("Revision absent")?,
            activity: UiWatchdogActivity::VisibleActive,
        })?;
    }
    assert!(state.request_exit_intent(true).is_err());
    assert!(state.shutdown.is_none());
    assert!(
        matches!(take_first(&mut state.queued),Some(Control::Activity{revision,..}) if revision.get()==1)
    );
    state.request_exit_intent(true)?;
    for value in 2..=4 {
        assert!(
            matches!(take_first(&mut state.queued),Some(Control::Activity{revision,..}) if revision.get()==value)
        );
    }
    assert!(matches!(take_first(&mut state.queued), Some(Control::ExitIntent)));
    assert!(state.budget.is_none());
    assert!(!state.intent_done);
    assert!(!state.native_retired);
    Ok(())
}

/// Keep native execution unopened while exercising the actual Child whole-close entrypoints.
fn inert_whole_close_child()
-> Result<(ProductionChild, std::sync::Arc<std::sync::atomic::AtomicUsize>)> {
    use sugarsubstitute_desktop::MainCaptionLabels;
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    };
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let bootstrap = ChildBootstrap::new(
        super::super::channel::ChildChannel::new(
            InheritedLifecycleBootstrap::new(1)?,
            LifecyclePipeDeadline::new(100, 100)?,
            run,
        ),
        super::super::capture::ChildCapture::new(Box::new(
            super::super::test_support::InertDriver(calls.clone()),
        )),
        ScopeSpec { id: ScopeId::new(3)?, deadline: None },
        (LaunchId::new(1)?, Generation::new(1)?),
    )?;
    let child = ProductionChild::assemble(
        bootstrap,
        run,
        (LaunchId::new(1)?, Generation::new(1)?),
        MainWindowSettings {
            recovery: sugarsubstitute_desktop::MainRecoveryLabels {
                retry: sugarsubstitute_contracts::localization::messages("en").retry,
                retry_ready: sugarsubstitute_contracts::localization::messages("en").retry_ready,
            },
            logical_width: 1280,
            logical_height: 720,
            dark: true,
            labels: MainCaptionLabels {
                minimize: "minimize",
                maximize: "maximize",
                restore: "restore",
                close: "close",
            },
        },
        crate::diagnostics::DiagnosticHealth::fixture(),
    );
    Ok((child, calls))
}
#[test]
fn first_whole_close_intent_survives_phase_failure_registration_and_queue_delay_in_actual_payload()
-> Result<()> {
    use sugarsubstitute_application::startup::StartupPhase;
    for (phase, failed, expected, direct_request) in [
        (StartupPhase::Running, false, CompletionIntent::Normal, false),
        (StartupPhase::Running, false, CompletionIntent::Normal, true),
        (StartupPhase::PreparingConnection, false, CompletionIntent::StartupCancelled, false),
        (StartupPhase::PreparingConnection, false, CompletionIntent::StartupCancelled, true),
        (StartupPhase::Running, true, CompletionIntent::StartupFailed, false),
        (StartupPhase::Running, true, CompletionIntent::StartupFailed, true),
    ] {
        let (mut child, calls) = inert_whole_close_child()?;
        child.phase = Some(phase);
        child.failed = failed;
        let original = ShutdownBudget::new(Moment::from_nanos(50))?;
        if direct_request {
            child.request_shutdown(original)?;
        } else {
            assert_eq!(child.begin_shutdown()?, ShutdownInitiation::NoCapture);
            assert!(child.shutdown_budget().is_none());
        }
        assert_eq!(child.shutdown.intent, Some(expected));
        assert!(child.shutdown.shutdown.is_none());
        // Registration is still absent; subsequent real entrypoint calls must keep the first intent.
        child.phase = Some(StartupPhase::Cancelling);
        child.failed = true;
        child.begin_shutdown()?;
        child.request_shutdown(original)?;
        child.phase = Some(StartupPhase::Running);
        child.failed = false;
        child.begin_shutdown()?;
        assert_eq!(child.shutdown.intent, Some(expected));
        // These copied boundary facts exercise the real bounded queue, without a native peer.
        for value in 1..=4 {
            child.shutdown.queue(Control::Activity {
                revision: NonZeroU64::new(value).ok_or("Revision absent")?,
                activity: UiWatchdogActivity::VisibleActive,
            })?;
        }
        assert!(child.shutdown.request_exit_intent(true).is_err());
        assert_eq!(child.shutdown.intent, Some(expected));
        assert!(child.shutdown.shutdown.is_none());
        let _removed = take_first(&mut child.shutdown.queued);
        child.shutdown.request_exit_intent(true)?;
        let id = child.shutdown.shutdown.ok_or("Original shutdown identity absent")?;
        let header = WireHeader {
            run: child.run,
            connection: Some(ConnectionId::new(1)?),
            request: Some(RequestId::new(1)?),
            deadline_qpc: 300,
        };
        for next_phase in [StartupPhase::Cancelling, StartupPhase::Stopped, StartupPhase::Running] {
            child.phase = Some(next_phase);
            child.failed = true;
            let payload = child.shutdown.exit_intent_payload()?;
            let mut bytes = [0; 512];
            let count = FrameCodec::encode(&WireMessage { header, payload }, &mut bytes)?;
            assert!(
                matches!(FrameCodec::decode(&bytes[..count])?.payload,DecodedPayload::ExitIntent {shutdown,intent,expected_code:0} if shutdown==id && intent==expected)
            );
            assert_eq!(child.shutdown.intent, Some(expected));
        }
        assert_eq!(child.shutdown_budget(), Some(original));
        assert!(!child.shutdown.intent_done);
        assert!(!child.shutdown.native_retired);
        assert_eq!(calls.load(std::sync::atomic::Ordering::Acquire), 0);
    }
    Ok(())
}
