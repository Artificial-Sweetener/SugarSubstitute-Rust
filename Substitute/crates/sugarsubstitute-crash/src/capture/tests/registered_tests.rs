// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real execution delivery with explicitly fake external process observation.
use super::*;
use sugarsubstitute_contracts::incident::{CaptureAbsence, CaptureEvidence, DumpId};
use sugarsubstitute_execution::windows::{NativeObservationError, ProcessExit};

fn assert_open_projection(source: &Shared, absence: CaptureAbsence) -> TestResult {
    use crate::incident::{IncidentStoreError, collection::opened_evidence};
    assert_eq!(opened_evidence(source, 1, None)?, CaptureEvidence::Unavailable(absence));
    let dump = DumpId::new([5; 16])?;
    assert_eq!(opened_evidence(source, 0, Some(dump))?, CaptureEvidence::Finished(dump));
    assert!(matches!(opened_evidence(source, 0, None), Err(IncidentStoreError::MissingPayload)));
    for (status, expected) in [
        (2, CaptureAbsence::Busy),
        (3, CaptureAbsence::Corrupt),
        (4, CaptureAbsence::WrongIdentity),
        (5, CaptureAbsence::Oversized),
        (6, CaptureAbsence::DeadlineElapsed),
    ] {
        assert_eq!(opened_evidence(source, status, None)?, CaptureEvidence::Unavailable(expected));
    }
    assert!(matches!(opened_evidence(source, 7, None), Err(IncidentStoreError::RecoveryConflict)));
    Ok(())
}

fn run(outcome: u8, closing: bool) -> TestResult {
    let rig = Rig::new()?;
    let port = Arc::new(FakePort::default());
    port.healthy.store(1, Ordering::Release);
    port.release.open();
    let observation_entered = Arc::new(Gate::default());
    let observation_release = Arc::new(Gate::default());
    let entered = observation_entered.clone();
    let release = observation_release.clone();
    let permit = rig.watch(1)?;
    let mut spec = close_spec(2, Moment::from_nanos(60_000_000_000))?;
    spec.class = JobClass::InfrastructureOrdinary;
    spec.operation = Operation::Infrastructure;
    let correlation = NativeCorrelation::new(1)?;
    let job = rig.client.submit_blocking(&rig.scope, spec, move |_| {
        entered.open();
        if let Err(cause) = release.wait() {
            std::panic::resume_unwind(Box::new(cause.to_string()));
        }
        if outcome == 1 {
            std::panic::resume_unwind(Box::new("controlled-native-watch-cause".to_owned()));
        }
        Ok::<_, NativeObservationError>(ProcessExit {
            correlation,
            process_id: if outcome == 2 { 99 } else { 17 },
            creation_time_100ns: 23,
            exit_code: 259,
        })
    })?;
    *lock(&port.watch_job) = Some(job);
    observation_entered.wait()?;
    let mut capture = WindowsCrashCapture::with_port(coordinator()?, port.clone());
    let budget = CaptureBudget::new(Moment::from_nanos(0))?;
    capture
        .submit_start(
            &rig.client,
            &rig.scope,
            work_spec(3, budget.until())?,
            budget,
            permit,
            input()?,
        )
        .map_err(|_| "Start rejected")?;
    rig.observe_until(&mut capture, |owner| owner.snapshot().state == CaptureState::Registered)?;
    assert_eq!(
        capture.snapshot().handler,
        Some(CaptureHandlerIdentity { process_id: 17, creation_time_100ns: 23 })
    );
    assert!(capture.snapshot().handler_exit.is_none());
    assert_eq!(rig.scope.snapshot().reserved_bytes, crate::input::STORAGE_BYTES as u64 + 4096);
    let source = capture.shared.as_ref().ok_or("Registered source missing")?.clone();
    let finished = capture.finished_source()?;
    assert_open_projection(&source, CaptureAbsence::Missing)?;
    if closing {
        capture.submit_close(&rig.client, &rig.scope, close_spec(4, budget.until())?)?;
    }
    observation_release.open();
    rig.observe_until(&mut capture, |owner| owner.watch.is_none())?;
    if !closing {
        assert_eq!(capture.snapshot().state, CaptureState::Degraded);
    }
    let absence = if outcome == 0 && !closing {
        CaptureAbsence::HandlerLost
    } else {
        CaptureAbsence::Missing
    };
    assert_eq!(capture.handler_lost(), outcome == 0 && !closing);
    assert_open_projection(&source, absence)?;
    let retained_exit = lock(&source.native).unexpected_handler_exit;
    if outcome == 0 && !closing {
        assert_eq!(
            retained_exit,
            Some(ProcessExit {
                correlation,
                process_id: 17,
                creation_time_100ns: 23,
                exit_code: 259,
            })
        );
        assert!(capture.errors().next().is_none());
    } else {
        assert!(retained_exit.is_none());
    }
    match outcome {
        0 => assert_eq!(capture.snapshot().handler_exit, Some(259)),
        1 => {
            assert!(capture.snapshot().handler_exit.is_none());
            assert!(capture.errors().any(|cause| std::error::Error::source(cause).is_some_and(
                |source| source.to_string().contains("controlled-native-watch-cause")
            )));
        }
        _ => assert!(capture.snapshot().handler_exit.is_none()),
    }
    for job in lock(&port.cancelled_permits).iter_mut() {
        let completion = rig.completion(job)?;
        assert!(matches!(completion.outcome, JobOutcome::Cancelled));
    }
    if !closing {
        capture.submit_close(&rig.client, &rig.scope, close_spec(4, budget.until())?)?;
    }
    rig.observe_until(&mut capture, |owner| owner.snapshot().state == CaptureState::Closed)?;
    assert_open_projection(&source, absence)?;
    assert_eq!(port.dropped.load(Ordering::Acquire), 0);
    drop(finished);
    drop(source);
    assert_eq!(port.dropped.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn registered_fake_handler_reports_matching_actual_typed_exit_including_259() -> TestResult {
    run(0, false)
}
#[test]
fn registered_fake_handler_late_watch_failure_retains_cause_and_degrades() -> TestResult {
    run(1, false)
}
#[test]
fn registered_fake_handler_mismatched_typed_identity_never_reports_exit() -> TestResult {
    run(2, false)
}

#[test]
fn registered_handler_exit_during_requested_close_keeps_missing_and_original_custody() -> TestResult
{
    run(0, true)
}
