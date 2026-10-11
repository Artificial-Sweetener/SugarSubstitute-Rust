// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Separate actual desktop exit attribution, supervisor cleanup and fixed deadlines.
use std::error::Error;
use sugarsubstitute_application::supervision::DesktopExitTransaction;
use sugarsubstitute_contracts::{execution::Moment, lifecycle::*};
type TestResult = Result<(), Box<dyn Error>>;
fn identities() -> Result<(RunKey, ProcessKey, ShutdownId), IdentityError> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    Ok((
        RunKey { owner, instance: InstanceId::new(1)?, generation: RunGeneration::new(1)? },
        ProcessKey { owner, id: ProcessId::new(1)? },
        ShutdownId::new(1)?,
    ))
}
fn observe(
    transaction: &mut DesktopExitTransaction,
    observation: DesktopObservation,
) -> Result<(), LifecycleRejection> {
    transaction.observe(observation).map_err(|error| error.reason)
}

#[test]
fn normal_exit_requires_current_intent_completion_and_actual_resource_settlement() -> TestResult {
    let (run, process, shutdown) = identities()?;
    let mut transaction = DesktopExitTransaction::new(run, process)?;
    transaction.begin(run, shutdown, CompletionIntent::Normal, 0, Moment::from_nanos(100))?;
    observe(&mut transaction, DesktopObservation::Completed { run, shutdown })?;
    observe(&mut transaction, DesktopObservation::Exited { process, code: 0 })?;
    let before = transaction.poll(Moment::from_nanos(100))?;
    assert_eq!(before.exit, Some(DesktopExitKind::Normal));
    assert!(!before.settled);
    observe(&mut transaction, DesktopObservation::ResourcesSettled { run, shutdown })?;
    assert!(transaction.poll(Moment::from_nanos(100))?.settled);
    Ok(())
}

#[test]
fn hard_zero_exit_can_complete_supervisor_cleanup_without_becoming_normal() -> TestResult {
    let (run, process, shutdown) = identities()?;
    let mut transaction = DesktopExitTransaction::new(run, process)?;
    observe(&mut transaction, DesktopObservation::Exited { process, code: 0 })?;
    assert_eq!(
        transaction.begin(run, shutdown, CompletionIntent::Normal, 0, Moment::from_nanos(0)),
        Err(LifecycleRejection::WrongPhase)
    );
    let budget = transaction.begin_cleanup(run, shutdown, Moment::from_nanos(0))?;
    assert_eq!(transaction.begin_cleanup(run, shutdown, Moment::from_nanos(100))?, budget);
    assert_eq!(
        observe(&mut transaction, DesktopObservation::Completed { run, shutdown }),
        Err(LifecycleRejection::WrongPhase)
    );
    observe(&mut transaction, DesktopObservation::ResourcesSettled { run, shutdown })?;
    let snapshot = transaction.poll(Moment::from_nanos(100))?;
    assert_eq!(snapshot.exit, Some(DesktopExitKind::Unexpected));
    assert!(snapshot.settled);
    Ok(())
}

#[test]
fn cancellation_failure_and_wrong_native_status_keep_distinct_attribution() -> TestResult {
    let (run, process, shutdown) = identities()?;
    for (intent, code, expected) in [
        (CompletionIntent::StartupCancelled, 0, DesktopExitKind::StartupCancelled),
        (CompletionIntent::StartupFailed, 0, DesktopExitKind::StartupFailed),
        (CompletionIntent::Normal, 1, DesktopExitKind::Unexpected),
    ] {
        let mut transaction = DesktopExitTransaction::new(run, process)?;
        transaction.begin(run, shutdown, intent, 0, Moment::from_nanos(0))?;
        observe(&mut transaction, DesktopObservation::Completed { run, shutdown })?;
        observe(&mut transaction, DesktopObservation::Exited { process, code })?;
        assert_eq!(transaction.poll(Moment::from_nanos(0))?.exit, Some(expected));
    }
    Ok(())
}

#[test]
fn deadlines_never_reset_or_fabricate_native_settlement_and_regression_rejects() -> TestResult {
    let (run, process, shutdown) = identities()?;
    let mut transaction = DesktopExitTransaction::new(run, process)?;
    let budget =
        transaction.begin(run, shutdown, CompletionIntent::Normal, 0, Moment::from_nanos(123))?;
    assert_eq!(budget.graceful_until.as_nanos(), 10_000_000_123);
    assert_eq!(budget.settle_until.as_nanos(), 15_000_000_123);
    assert_eq!(
        transaction.begin(run, shutdown, CompletionIntent::Normal, 0, Moment::from_nanos(999))?,
        budget
    );
    assert_eq!(
        transaction.begin(
            run,
            ShutdownId::new(2)?,
            CompletionIntent::Normal,
            0,
            Moment::from_nanos(999)
        ),
        Err(LifecycleRejection::ChangedDuplicate)
    );
    let snapshot = transaction.poll(budget.graceful_until)?;
    assert!(snapshot.graceful_expired);
    assert!(!snapshot.deadline_expired);
    assert!(!snapshot.settled);
    assert!(transaction.poll(budget.settle_until)?.deadline_expired);
    assert_eq!(transaction.poll(Moment::from_nanos(0)), Err(LifecycleRejection::InvalidClock));
    assert!(!transaction.poll(budget.settle_until)?.settled);
    Ok(())
}

#[test]
fn foreign_proofs_and_second_failure_return_owned_causes_without_state_changes() -> TestResult {
    let (run, process, shutdown) = identities()?;
    let mut transaction = DesktopExitTransaction::new(run, process)?;
    transaction.begin(run, shutdown, CompletionIntent::Normal, 0, Moment::from_nanos(0))?;
    let mut foreign = run;
    foreign.generation = RunGeneration::new(2)?;
    assert_eq!(
        observe(&mut transaction, DesktopObservation::Completed { run: foreign, shutdown }),
        Err(LifecycleRejection::StaleIdentity)
    );
    let mut foreign_process = process;
    foreign_process.id = ProcessId::new(2)?;
    assert_eq!(
        observe(&mut transaction, DesktopObservation::Exited { process: foreign_process, code: 0 }),
        Err(LifecycleRejection::StaleIdentity)
    );
    observe(
        &mut transaction,
        DesktopObservation::Failed {
            run,
            shutdown,
            failure: LifecycleFailure::new(
                LifecycleFailureCode::Cleanup,
                Box::new(std::io::Error::other("first")),
            ),
        },
    )?;
    let rejected = transaction
        .observe(DesktopObservation::Failed {
            run,
            shutdown,
            failure: LifecycleFailure::new(
                LifecycleFailureCode::Cleanup,
                Box::new(std::io::Error::other("second")),
            ),
        })
        .err()
        .ok_or("Second failure accepted")?;
    if let DesktopObservation::Failed { failure, .. } = rejected.observation {
        assert_eq!(failure.source().ok_or("Source missing")?.to_string(), "second");
    } else {
        return Err("Cause lost".into());
    }
    observe(&mut transaction, DesktopObservation::Completed { run, shutdown })?;
    observe(&mut transaction, DesktopObservation::Exited { process, code: 0 })?;
    observe(&mut transaction, DesktopObservation::ResourcesSettled { run, shutdown })?;
    assert!(!transaction.poll(Moment::from_nanos(0))?.settled);
    assert_eq!(
        transaction
            .failure()
            .ok_or("Failure missing")?
            .source()
            .ok_or("Source missing")?
            .to_string(),
        "first"
    );
    Ok(())
}
