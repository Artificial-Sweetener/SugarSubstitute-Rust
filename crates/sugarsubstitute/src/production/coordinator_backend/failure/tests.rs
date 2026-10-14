// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify closed metadata while preserving actual source and completion custody.
use super::*;
use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::execution::{AdmissionError, PanicCause};

struct Uninspectable(Arc<AtomicUsize>);
impl fmt::Debug for Uninspectable {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        assert_eq!(self.0.load(Ordering::SeqCst), usize::MAX, "unknown debug called");
        Ok(())
    }
}
impl fmt::Display for Uninspectable {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        assert_eq!(self.0.load(Ordering::SeqCst), usize::MAX, "unknown display called");
        Ok(())
    }
}
impl Error for Uninspectable {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        assert_eq!(self.0.load(Ordering::SeqCst), usize::MAX, "unknown source called");
        None
    }
}
impl Drop for Uninspectable {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
fn retained(cause: impl Error + Send + Sync + 'static) -> LifecycleFailure {
    LifecycleFailure::new(LifecycleFailureCode::Preparation, Box::new(cause))
}

#[test]
fn root_exit_diagnostic_keeps_exact_original_code_and_cause() {
    for code in [2, 0xc000_00fd] {
        let cause = Box::new(RootExit(code));
        let original = std::ptr::from_ref(cause.as_ref());
        let failure = LifecycleFailure::new(LifecycleFailureCode::Preparation, cause);
        let observation = FailureObservation::retained(&failure);
        assert_eq!(observation.kind, FailureKind::RootExit);
        assert_eq!(observation.stage, "root_exit");
        assert_eq!(observation.exit_code, Some(code));
        assert_eq!(observation.terminal, None);
        assert_eq!(
            failure
                .source()
                .and_then(|cause| cause.downcast_ref::<RootExit>())
                .map(std::ptr::from_ref),
            Some(original)
        );
    }
}

#[test]
fn stopped_diagnostic_preserves_exact_terminal_without_panic_text() {
    for kind in [
        TerminalKind::Cancelled,
        TerminalKind::TimedOut,
        TerminalKind::Coalesced,
        TerminalKind::Panicked,
    ] {
        let failure = retained(Stopped(kind));
        let observation = FailureObservation::retained(&failure);
        assert_eq!(observation.kind, FailureKind::Stopped);
        assert_eq!(observation.terminal, Some(kind));
        assert_eq!(observation.stage, "preparation_stopped");
        assert_eq!(
            failure.source().and_then(|cause| cause.downcast_ref::<Stopped>()).map(|cause| cause.0),
            Some(kind)
        );
    }
    let outcome = JobOutcome::Panicked(PanicCause::new("private/path/panic/secret".into()));
    let observation = FailureObservation::output(&outcome);
    assert_eq!(observation.terminal, Some(TerminalKind::Panicked));
    assert!(!format!("{observation:?}").contains("secret"));
}

#[test]
fn output_diagnostic_borrows_actual_admitted_completion_and_native_cause() -> Result<()> {
    let completion = completion::native_failure()?;
    let original = match &completion.outcome {
        JobOutcome::Failure(cause) => cause.source().map(std::ptr::from_ref),
        _ => return Err("Actual admitted output-shaped failure did not retain native cause".into()),
    };
    let spec = completion.spec;
    let failure = retained(OutputTerminalFailure(completion));
    let observation = FailureObservation::retained(&failure);
    assert_eq!(observation.kind, FailureKind::Output);
    assert_eq!(observation.terminal, Some(TerminalKind::Failure));
    assert_eq!(observation.native_operation, Some("output_read"));
    assert_eq!(observation.native_code, Some(5));
    let actual = failure
        .source()
        .and_then(|cause| cause.downcast_ref::<OutputTerminalFailure>())
        .ok_or(LifecycleRejection::WrongPhase)?;
    assert_eq!(actual.0.spec, spec);
    assert!(
        matches!(&actual.0.outcome, JobOutcome::Failure(cause) if cause.source().map(std::ptr::from_ref) == original)
    );
    assert_eq!(FailureObservation::retained(&failure), observation);
    Ok(())
}

#[test]
fn unknown_error_callbacks_and_custody_are_untouched_directly_and_inside_native_owner() {
    let drops = Arc::new(AtomicUsize::new(0));
    let failure = retained(Uninspectable(drops.clone()));
    let observation = FailureObservation::retained(&failure);
    assert_eq!(observation, FailureObservation::classified(FailureKind::Other, "unclassified"));
    assert!(!format!("{observation:?}").contains("secret"));
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(failure);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    let failure =
        retained(NativeObservationError::new("output_read", Uninspectable(drops.clone())));
    let observation = FailureObservation::retained(&failure);
    assert_eq!(observation.kind, FailureKind::Native);
    assert_eq!(observation.native_operation, Some("output_read"));
    assert_eq!(observation.native_code, None);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    drop(failure);
    assert_eq!(drops.load(Ordering::SeqCst), 2);
}

#[test]
fn exact_admission_and_lifecycle_variants_do_not_require_error_sources() {
    let failure = retained(ClaimAdmissionError::Registry(RegistryAdmissionError::Execution(
        AdmissionError::DeadlineExpired {
            deadline: Moment::from_nanos(1),
            now: Moment::from_nanos(2),
        },
    )));
    assert_eq!(
        FailureObservation::retained(&failure),
        FailureObservation::classified(FailureKind::Admission, "admission_deadline_expired")
    );
    let failure =
        retained(AdmissionError::QueueSaturated { class: JobClass::InfrastructureOrdinary });
    assert_eq!(FailureObservation::retained(&failure).stage, "admission_queue_saturated");
    let failure = retained(RegistryAdmissionError::Busy);
    assert_eq!(FailureObservation::retained(&failure).stage, "registry_busy");
    let failure = retained(LifecycleRejection::WrongPhase);
    assert_eq!(
        FailureObservation::retained(&failure),
        FailureObservation::classified(FailureKind::Lifecycle, "lifecycle_wrong_phase")
    );
    let drops = Arc::new(AtomicUsize::new(0));
    let failure = retained(AdmissionError::Unexpected(
        sugarsubstitute_contracts::execution::ExecutionError::new(
            Operation::Infrastructure,
            Box::new(Uninspectable(drops.clone())),
        ),
    ));
    assert_eq!(FailureObservation::retained(&failure).stage, "admission_unexpected");
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(failure);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[path = "tests/completion.rs"]
mod completion;
