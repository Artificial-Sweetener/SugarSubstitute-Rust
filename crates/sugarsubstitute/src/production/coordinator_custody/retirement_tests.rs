// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the retirement owner with typed external terminal/family/Finished boundary data.
//! These policy facts create no native acknowledgement, family exit or pipe retirement proof.
use super::*;
use std::error::Error;
use sugarsubstitute_execution::{
    backend_custody::BackendCustodyPipeReport, windows::NativeObservationError,
};
type TestResult = std::result::Result<(), Box<dyn Error>>;
fn key() -> std::result::Result<BackendCustodyKey, Box<dyn Error>> {
    use sugarsubstitute_contracts::lifecycle::{CoordinatorEpoch, CoordinatorKey, NamespaceId};
    Ok(BackendCustodyKey::new(
        CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        BackendCustodySession::new(1)?,
    ))
}
fn success()
-> std::result::Result<JobOutcome<BackendCustodyPipeReport, NativeObservationError>, Box<dyn Error>>
{
    Ok(JobOutcome::Success(BackendCustodyPipeReport {
        key: Some(key()?),
        received_messages: 1,
        completed_writes: 1,
    }))
}
#[test]
fn accepted_finished_does_not_replace_actual_family_retirement_or_matching_terminal() -> TestResult
{
    assert!(inspect_terminal(&success()?, key()?, true, true, true, true).is_ok());
    assert!(inspect_terminal(&success()?, key()?, true, true, true, false).is_err());
    let foreign = BackendCustodyKey::new(key()?.owner(), BackendCustodySession::new(2)?);
    assert!(inspect_terminal(&success()?, foreign, true, true, true, true).is_err());
    Ok(())
}
#[test]
fn terminal_presence_and_local_cancellation_do_not_replace_finished() -> TestResult {
    let cancelled: JobOutcome<BackendCustodyPipeReport, NativeObservationError> =
        JobOutcome::Cancelled;
    assert!(inspect_terminal(&success()?, key()?, true, false, true, true).is_err());
    assert!(inspect_terminal(&cancelled, key()?, true, false, true, true).is_err());
    assert!(inspect_terminal(&cancelled, key()?, true, true, false, true).is_err());
    assert!(inspect_terminal(&cancelled, key()?, true, true, true, true).is_ok());
    assert!(inspect_terminal(&cancelled, key()?, false, false, true, true).is_ok());
    Ok(())
}
#[test]
fn original_native_terminal_failure_survives_finished_and_transition_attribution() -> TestResult {
    let cause = NativeObservationError::new(
        "original_pipe_failure",
        std::io::Error::other("Bounded external failure"),
    );
    let source = std::ptr::from_ref(cause.source().ok_or("Original cause missing")?);
    let outcome = JobOutcome::Failure(cause);
    for finished in [false, true] {
        let error = inspect_terminal(&outcome, key()?, true, finished, true, true)
            .err()
            .ok_or("Failure accepted")?;
        let native = error.downcast_ref::<NativeObservationError>().ok_or("Cause type lost")?;
        assert_eq!(std::ptr::from_ref(native.source().ok_or("Retained source missing")?), source);
        let attributed = diagnostics::failed("pipe_terminal", native.clone());
        assert!(attributed.source().is_some());
    }
    Ok(())
}
#[test]
fn expired_coalesced_and_panicked_outcomes_remain_failures() -> TestResult {
    use sugarsubstitute_contracts::execution::{JobId, PanicCause};
    let outcomes: [JobOutcome<BackendCustodyPipeReport, NativeObservationError>; 3] = [
        JobOutcome::TimedOut,
        JobOutcome::Coalesced { replacement: JobId::new(2)? },
        JobOutcome::Panicked(PanicCause::new("Bounded original panic".into())),
    ];
    for outcome in outcomes {
        assert!(inspect_terminal(&outcome, key()?, true, true, true, true).is_err());
    }
    Ok(())
}
