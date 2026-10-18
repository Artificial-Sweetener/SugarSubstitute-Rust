// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the bounded production collector through a controlled raw-counter boundary.

use super::*;
use std::{cell::Cell, error::Error, rc::Rc};
use sugarsubstitute_contracts::execution::{Generation, SurfaceId};

type TestResult = Result<(), Box<dyn Error>>;

struct Counter {
    stamp: Rc<Cell<Stamp>>,
    frequency: i64,
    thread: u32,
    failure: Rc<Cell<bool>>,
}
impl TimingClock for Counter {
    fn frequency(&self) -> i64 {
        self.frequency
    }
    fn thread(&self) -> u32 {
        self.thread
    }
    fn read(&mut self) -> Result<Stamp, DesktopError> {
        if self.failure.get() {
            return Err(DesktopError::new(
                "injected_counter",
                std::io::Error::other("counter unavailable"),
            ));
        }
        let stamp = self.stamp.get();
        self.stamp.set(Stamp { ticks: stamp.ticks + 1, ..stamp });
        Ok(stamp)
    }
}
fn counter(frequency: i64, thread: u32) -> (Counter, Rc<Cell<Stamp>>, Rc<Cell<bool>>) {
    let stamp = Rc::new(Cell::new(Stamp { ticks: 10, thread: 7, source: QPC_SOURCE }));
    let failure = Rc::new(Cell::new(false));
    (Counter { stamp: stamp.clone(), frequency, thread, failure: failure.clone() }, stamp, failure)
}
fn audit() -> Result<TimingAudit, DesktopError> {
    let mut audit = TimingAudit::new(
        Box::new(counter(1_000, 7).0),
        crate::timing::NativeStartupOrder::SurfaceFirst,
    )?;
    for stage in [
        NativeStartupStage::EventLoopReady,
        NativeStartupStage::WindowCreated,
        NativeStartupStage::PresenterReady,
        NativeStartupStage::ControlsReady,
        NativeStartupStage::ExecutionReady,
        NativeStartupStage::FirstPreparationRequest,
        NativeStartupStage::ShowFocusReturned,
    ] {
        audit.startup(stage)?;
    }
    Ok(audit)
}
fn identity(state: u64, sequence: u64) -> Result<FrameIdentity, Box<dyn Error>> {
    Ok(FrameIdentity {
        surface: SurfaceId::new(1)?,
        state_generation: Generation::new(state)?,
        sequence: Generation::new(sequence)?,
    })
}

#[test]
fn both_buffers_are_preallocated_and_never_grow_when_observations_overflow() -> TestResult {
    let mut audit = audit()?;
    let capacities = (audit.receipt.callbacks.capacity(), audit.receipt.submissions.capacity());
    assert!(capacities.0 >= SAMPLE_LIMIT && capacities.1 >= SAMPLE_LIMIT);
    for index in 0..SAMPLE_LIMIT + 3 {
        let begin = audit.mark()?;
        audit.callback(begin, NativeCallbackKind::LoopProgress, None)?;
        let begin = audit.mark()?;
        audit.submission(begin, identity(1, index as u64 + 1)?)?;
    }
    assert_eq!(
        capacities,
        (audit.receipt.callbacks.capacity(), audit.receipt.submissions.capacity())
    );
    let receipt = audit.finish()?;
    assert_eq!(receipt.callbacks.len(), SAMPLE_LIMIT);
    assert_eq!(receipt.submissions.len(), SAMPLE_LIMIT);
    assert_eq!(receipt.callbacks_observed, SAMPLE_LIMIT as u64 + 3);
    assert_eq!(receipt.submissions_observed, SAMPLE_LIMIT as u64 + 3);
    assert_eq!(receipt.callback_samples_omitted, 3);
    assert_eq!(receipt.submission_samples_omitted, 3);
    assert_eq!(receipt.callbacks[SAMPLE_LIMIT - 1].sequence, SAMPLE_LIMIT as u64);
    Ok(())
}

#[test]
fn nested_callbacks_keep_raw_overlap_and_exclude_only_parent_modal_segment() -> TestResult {
    let (clock, stamp, _) = counter(1_000, 7);
    let observer = TimingObserver::new(TimingAudit::new(
        Box::new(clock),
        crate::timing::NativeStartupOrder::SurfaceFirst,
    )?);
    let parent = observer.mark()?;
    let delegated_begin = observer.mark()?;
    // No collector borrow spans the delegated operation: nested native/Tao calls remain observable.
    stamp.set(Stamp { ticks: 1_000, ..stamp.get() });
    let child = observer.mark()?;
    observer.callback(child, NativeCallbackKind::WindowsChild, None)?;
    let tao = observer.mark()?;
    observer.callback(tao, NativeCallbackKind::CompletionDrain, None)?;
    stamp.set(Stamp { ticks: 2_012, ..stamp.get() });
    let delegated_end = observer.mark()?;
    observer.callback(
        parent,
        NativeCallbackKind::WindowsParent,
        Some(NativeTickInterval {
            begin_ticks: delegated_begin.ticks,
            end_ticks: delegated_end.ticks,
        }),
    )?;
    let receipt = observer.finish()?;
    assert_eq!(receipt.callbacks.len(), 3);
    assert_eq!(receipt.callbacks[0].authored_ticks, 1);
    assert_eq!(receipt.callbacks[1].authored_ticks, 1);
    let parent = receipt.callbacks[2];
    assert_eq!(parent.sequence, 3);
    assert_eq!(parent.begin_ticks, 11);
    assert_eq!(parent.end_ticks, 2_013);
    assert_eq!(parent.authored_ticks, 2);
    assert_eq!(receipt.frequency, 1_000);
    Ok(())
}

#[test]
fn successful_submissions_compare_complete_identity_and_keep_last_success_after_omission()
-> TestResult {
    let mut audit = audit()?;
    for frame in [identity(1, 1)?, identity(1, 1)?, identity(1, 2)?, identity(2, 2)?] {
        let begin = audit.mark()?;
        audit.submission(begin, frame)?;
    }
    assert_eq!(
        audit.receipt.submissions.iter().map(|sample| sample.retained).collect::<Vec<_>>(),
        [false, true, false, false]
    );
    let last = identity(3, 1)?;
    for _ in 4..SAMPLE_LIMIT + 1 {
        let begin = audit.mark()?;
        audit.submission(begin, last)?;
    }
    assert_eq!(audit.last_identity, Some(last));
    assert_eq!(audit.receipt.submission_samples_omitted, 1);
    Ok(())
}

#[test]
fn wrong_source_thread_negative_ticks_and_regression_reject_before_sample_append() -> TestResult {
    for invalid in [
        Stamp { ticks: 12, thread: 7, source: 99 },
        Stamp { ticks: 12, thread: 8, source: QPC_SOURCE },
        Stamp { ticks: -1, thread: 7, source: QPC_SOURCE },
        Stamp { ticks: 9, thread: 7, source: QPC_SOURCE },
    ] {
        let (clock, stamp, _) = counter(1_000, 7);
        let mut audit =
            TimingAudit::new(Box::new(clock), crate::timing::NativeStartupOrder::SurfaceFirst)?;
        stamp.set(invalid);
        assert!(audit.mark().is_err());
        assert_eq!(audit.last_tick, 10);
        assert_eq!(audit.receipt.callbacks_observed, 0);
        assert_eq!(audit.receipt.submissions_observed, 0);
    }
    for (frequency, thread) in [(0, 7), (-1, 7), (1_000, 0)] {
        assert!(
            TimingAudit::new(
                Box::new(counter(frequency, thread).0),
                crate::timing::NativeStartupOrder::SurfaceFirst
            )
            .is_err()
        );
    }
    Ok(())
}

#[test]
fn malformed_segments_and_overflow_do_not_partially_commit_counts_or_identity() -> TestResult {
    let mut audit = audit()?;
    let begin = audit.mark()?;
    assert!(
        audit
            .callback(
                begin,
                NativeCallbackKind::WindowsParent,
                Some(NativeTickInterval { begin_ticks: begin.ticks - 1, end_ticks: begin.ticks })
            )
            .is_err()
    );
    assert!(
        audit
            .callback(
                begin,
                NativeCallbackKind::WindowsParent,
                Some(NativeTickInterval { begin_ticks: begin.ticks + 1, end_ticks: begin.ticks })
            )
            .is_err()
    );
    assert_eq!(audit.receipt.callbacks_observed, 0);
    audit.receipt.callbacks_observed = u64::MAX;
    assert!(audit.callback(begin, NativeCallbackKind::Other, None).is_err());
    assert!(audit.receipt.callbacks.is_empty());
    audit.receipt.submissions_observed = u64::MAX;
    assert!(audit.submission(begin, identity(1, 1)?).is_err());
    assert_eq!(audit.last_identity, None);
    assert!(audit.receipt.submissions.is_empty());
    Ok(())
}

#[test]
fn omission_overflow_is_observed_before_mutating_sequence() -> TestResult {
    let mut audit = audit()?;
    let frame = identity(1, 1)?;
    for _ in 0..SAMPLE_LIMIT {
        let begin = audit.mark()?;
        audit.callback(begin, NativeCallbackKind::Other, None)?;
        let begin = audit.mark()?;
        audit.submission(begin, frame)?;
    }
    audit.receipt.callback_samples_omitted = u64::MAX;
    audit.receipt.submission_samples_omitted = u64::MAX;
    let begin = audit.mark()?;
    assert!(audit.callback(begin, NativeCallbackKind::Other, None).is_err());
    assert!(audit.submission(begin, identity(2, 1)?).is_err());
    assert_eq!(audit.receipt.callbacks_observed, SAMPLE_LIMIT as u64);
    assert_eq!(audit.receipt.submissions_observed, SAMPLE_LIMIT as u64);
    assert_eq!(audit.last_identity, Some(frame));
    Ok(())
}

#[test]
fn native_failure_survives_callback_owner_release_and_consumption_is_exactly_once() -> TestResult {
    let observer = TimingObserver::new(audit()?);
    let retained = observer.clone();
    drop(observer);
    let begin = retained.mark()?;
    retained.callback(begin, NativeCallbackKind::WindowsChild, None)?;
    let late = retained.clone();
    assert_eq!(retained.finish()?.callbacks.len(), 1);
    assert!(late.mark().is_err());
    assert!(late.finish().is_err());
    let observer = TimingObserver::new(audit()?);
    observer.fail(DesktopError::new("native_counter", std::io::Error::other("counter cause")));
    observer.fail(DesktopError::message("secondary", "later failure"));
    assert!(observer.has_failure());
    let failure = observer.finish().err().ok_or("Native callback failure lost")?;
    assert!(failure.to_string().contains("native_counter"));
    assert_eq!(failure.source().ok_or("Cause lost")?.to_string(), "counter cause");
    Ok(())
}

#[test]
fn clock_boundary_failure_preserves_cause_and_disabling_audit_avoids_collector() -> TestResult {
    assert!(
        TimingObserver::native(false, crate::timing::NativeStartupOrder::SurfaceFirst)?.is_none()
    );
    let (clock, _, failure) = counter(1_000, 7);
    let mut audit =
        TimingAudit::new(Box::new(clock), crate::timing::NativeStartupOrder::SurfaceFirst)?;
    failure.set(true);
    let error = audit.mark().err().ok_or("Counter failure accepted")?;
    assert!(error.to_string().contains("injected_counter"));
    assert_eq!(error.source().ok_or("Cause lost")?.to_string(), "counter unavailable");
    assert_eq!(audit.receipt.callbacks_observed, 0);
    Ok(())
}

#[cfg(target_os = "windows")]
#[test]
fn actual_qpc_adapter_preserves_source_frequency_and_creating_thread() -> TestResult {
    let clock = crate::windows_timing::WindowsClock::new()?;
    let mut audit =
        TimingAudit::new(Box::new(clock), crate::timing::NativeStartupOrder::SurfaceFirst)?;
    let begin = audit.mark()?;
    audit.callback(begin, NativeCallbackKind::Other, None)?;
    let receipt = audit.finish()?;
    assert!(receipt.frequency > 0 && receipt.ui_thread_id > 0);
    assert_eq!(receipt.source, NativeTimingSource::WindowsQpc);
    assert!(receipt.end_ticks >= receipt.begin_ticks);
    Ok(())
}

#[test]
fn authored_callback_early_return_still_finishes_full_boundary() -> TestResult {
    let observer = TimingObserver::new(audit()?);
    let begin = observer.mark()?;
    let result = observe_callback(
        Some(&observer),
        Some(begin),
        NativeCallbackKind::CompletionDrain,
        || {
            if true {
                return 32;
            }
            0
        },
    )?;
    assert_eq!(result, 32);
    let receipt = observer.finish()?;
    assert_eq!(receipt.callbacks.len(), 1);
    assert_eq!(receipt.callbacks[0].kind, NativeCallbackKind::CompletionDrain);
    assert_eq!(receipt.callbacks[0].authored_ticks, 1);
    Ok(())
}

#[test]
fn failed_native_or_control_submission_never_changes_actual_success_identity() -> TestResult {
    let observer = TimingObserver::new(audit()?);
    let first = identity(1, 1)?;
    let failed = identity(1, 2)?;
    assert!(
        observe_submission(Some(&observer), first, || Err::<(), _>(DesktopError::message(
            "native_submit",
            "failed"
        )))
        .is_err()
    );
    observe_submission(Some(&observer), first, || Ok(()))?;
    assert!(
        observe_submission(Some(&observer), failed, || Err::<(), _>(DesktopError::message(
            "native_controls",
            "failed"
        )))
        .is_err()
    );
    observe_submission(Some(&observer), first, || Ok(()))?;
    let receipt = observer.finish()?;
    assert_eq!(receipt.submissions_observed, 2);
    assert_eq!(receipt.submissions.len(), 2);
    assert!(!receipt.submissions[0].retained);
    assert!(receipt.submissions[1].retained);
    assert!(receipt.submissions.iter().all(|sample| sample.identity == first));
    Ok(())
}

#[cfg(not(target_os = "windows"))]
#[test]
fn enabling_qpc_audit_on_another_platform_is_observable_before_native_construction() {
    assert!(TimingObserver::native(true, crate::timing::NativeStartupOrder::SurfaceFirst).is_err());
}

#[test]
fn native_failure_wake_is_bounded_until_tao_takes_the_pending_failure() -> TestResult {
    let observer = TimingObserver::new(audit()?);
    assert!(!observer.request_failure_wake());
    observer.fail(DesktopError::message("native_counter", "failed"));
    assert!(observer.request_failure_wake());
    let child = observer.clone();
    assert!(!child.request_failure_wake());
    assert!(observer.take_failure().is_some());
    assert!(!child.request_failure_wake());
    child.fail(DesktopError::message("next_native_counter", "failed"));
    assert!(child.request_failure_wake());
    assert!(!observer.request_failure_wake());
    assert!(child.finish().is_err());
    Ok(())
}
