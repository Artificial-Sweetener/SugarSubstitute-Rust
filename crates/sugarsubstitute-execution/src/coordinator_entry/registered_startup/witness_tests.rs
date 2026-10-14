// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Match actual terminal issuer and original completion custody after record acknowledgement.
use super::*;
use std::time::Duration;

type ProjectionCompletion = JobCompletion<PreparedStartupSnapshot, AdmissionError>;
fn issue(
    rig: &Rig,
    source: &RegisteredStartupSource,
    id: u64,
    previous: RunKey,
) -> TestResult<ProjectionCompletion> {
    let original = EntrySetupBudget::new(rig.client.now()?)?;
    let run = next(previous)?;
    let client = rig.client.clone();
    let retained = source.clone();
    let mut job = rig.client.submit_blocking(
        &rig.scope,
        projection_spec(id, original.until())?,
        move |context| {
            let snapshot =
                client.restart_startup_snapshot(&retained, previous, run, original, &context)?;
            // A matching consumer Success while this same actual issuer is Running is insufficient.
            let early = JobCompletion {
                scope_id: context.scope_id(),
                spec: context.spec(),
                queue_time: Duration::ZERO,
                execution_time: Duration::ZERO,
                outcome: JobOutcome::<(), AdmissionError>::Success(()),
                memory: context.memory().clone(),
                native_handles: None,
            };
            assert!(
                client.borrow_registered_restart_admission(&snapshot, &retained, &early).is_err()
            );
            assert!(
                client
                    .restart_startup_snapshot(&retained, previous, run, original, &context,)
                    .is_err()
            );
            Ok::<_, AdmissionError>(snapshot)
        },
    )?;
    rig.completion(&mut job)
}
fn snapshot(completed: &ProjectionCompletion) -> TestResult<&PreparedStartupSnapshot> {
    match &completed.outcome {
        JobOutcome::Success(snapshot) => Ok(snapshot),
        _ => Err("Actual registered projection failed".into()),
    }
}
fn close(rig: &Rig, session: &mut CoordinatorEntryServerSession) -> TestResult {
    session.control.close();
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(_)));
    Ok(())
}
#[test]
fn registered_restart_witness_reborrows_same_actual_terminal_after_core_ack_without_new_credit()
-> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    let (source, memory) = registered(&rig, request_snapshot(&rig, &boundary, &session)?)?;
    drop(memory);
    let previous = run()?;
    let completed = issue(&rig, &source, 3, previous)?;
    let snapshot = snapshot(&completed)?;
    assert!(!lock(&rig.client.core.state).jobs.contains_key(&completed.spec.id));
    let original_bytes = rig.scope.snapshot().reserved_bytes;
    let first = rig.client.borrow_registered_restart_admission(snapshot, &source, &completed)?;
    let second = rig.client.borrow_registered_restart_admission(snapshot, &source, &completed)?;
    first.check()?;
    second.check()?;
    assert_eq!(first.previous(), previous);
    assert_eq!(first.run(), next(previous)?);
    assert!(first.source().same_source(second.source()));
    assert_eq!(first.scope_id(), completed.scope_id);
    assert_eq!(first.job_id(), completed.spec.id);
    assert_eq!(first.spec(), completed.spec);
    assert_eq!(first.original_budget(), second.original_budget());
    assert_eq!(first.effective_until(), second.effective_until());
    assert_eq!(first.original_budget().until(), completed.spec.deadline.ok_or("Cutoff missing")?);
    assert_eq!(rig.scope.snapshot().reserved_bytes, original_bytes);
    drop(completed);
    drop(source);
    close(&rig, &mut session)?;
    let core = rig.client.core.clone();
    drop(session);
    drop(boundary);
    drop(rig);
    assert_eq!(lock(&core.state).bytes, 0);
    Ok(())
}
#[test]
fn registered_restart_witness_rejects_foreign_source_core_and_equal_metadata_foreign_memory()
-> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    let (source, memory) = registered(&rig, request_snapshot(&rig, &boundary, &session)?)?;
    drop(memory);
    let completed = issue(&rig, &source, 3, run()?)?;
    let snapshot = snapshot(&completed)?;
    let foreign = Rig::new()?;
    let foreign_boundary = Boundary::new(&foreign, true)?;
    let (_foreign_scope, mut foreign_session) = server(&foreign, &foreign_boundary)?;
    let (foreign_source, foreign_memory) =
        registered(&foreign, request_snapshot(&foreign, &foreign_boundary, &foreign_session)?)?;
    let counterfeit = JobCompletion {
        scope_id: completed.scope_id,
        spec: completed.spec,
        queue_time: completed.queue_time,
        execution_time: completed.execution_time,
        outcome: JobOutcome::<(), AdmissionError>::Success(()),
        memory: foreign_memory.clone(),
        native_handles: None,
    };
    assert!(
        rig.client.borrow_registered_restart_admission(snapshot, &source, &counterfeit).is_err()
    );
    assert!(
        rig.client
            .borrow_registered_restart_admission(snapshot, &foreign_source, &completed)
            .is_err()
    );
    assert!(
        foreign.client.borrow_registered_restart_admission(snapshot, &source, &completed).is_err()
    );
    let failure = JobCompletion {
        scope_id: completed.scope_id,
        spec: completed.spec,
        queue_time: Duration::ZERO,
        execution_time: Duration::ZERO,
        outcome: JobOutcome::<(), ()>::Failure(()),
        memory: completed.memory.clone(),
        native_handles: None,
    };
    assert!(rig.client.borrow_registered_restart_admission(snapshot, &source, &failure).is_err());
    let wrong_scope = JobCompletion {
        scope_id: ScopeId::new(999)?,
        spec: completed.spec,
        queue_time: Duration::ZERO,
        execution_time: Duration::ZERO,
        outcome: JobOutcome::<(), AdmissionError>::Success(()),
        memory: completed.memory.clone(),
        native_handles: None,
    };
    assert!(
        rig.client.borrow_registered_restart_admission(snapshot, &source, &wrong_scope).is_err()
    );
    let mut wrong_spec = completed.spec;
    wrong_spec.deadline = Some(Moment::from_nanos(u64::MAX));
    let wrong_cutoff = JobCompletion {
        scope_id: completed.scope_id,
        spec: wrong_spec,
        queue_time: Duration::ZERO,
        execution_time: Duration::ZERO,
        outcome: JobOutcome::<(), AdmissionError>::Success(()),
        memory: completed.memory.clone(),
        native_handles: None,
    };
    assert!(
        rig.client.borrow_registered_restart_admission(snapshot, &source, &wrong_cutoff).is_err()
    );
    drop(wrong_scope);
    drop(wrong_cutoff);
    let valid = rig.client.borrow_registered_restart_admission(snapshot, &source, &completed)?;
    valid.check()?;
    drop(failure);
    drop(counterfeit);
    drop(foreign_memory);
    drop(foreign_source);
    close(&foreign, &mut foreign_session)?;
    drop(completed);
    drop(source);
    close(&rig, &mut session)
}
#[test]
fn registered_restart_witness_rejects_actual_current_generation_change_with_original_completion_held()
-> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    let (source, memory) = registered(&rig, request_snapshot(&rig, &boundary, &session)?)?;
    drop(memory);
    let original = run()?;
    let first_completion = issue(&rig, &source, 3, original)?;
    let first = rig.client.borrow_registered_restart_admission(
        snapshot(&first_completion)?,
        &source,
        &first_completion,
    )?;
    let first_until = first.effective_until();
    let second_completion = issue(&rig, &source, 4, first.run())?;
    assert!(first.check().is_err());
    assert_eq!(first.effective_until(), first_until);
    assert!(
        rig.client
            .borrow_registered_restart_admission(
                snapshot(&first_completion)?,
                &source,
                &first_completion,
            )
            .is_err()
    );
    let second = rig.client.borrow_registered_restart_admission(
        snapshot(&second_completion)?,
        &source,
        &second_completion,
    )?;
    assert_eq!(second.previous(), next(original)?);
    second.check()?;
    drop(first_completion);
    drop(second_completion);
    drop(source);
    close(&rig, &mut session)
}
#[test]
fn registered_restart_witness_preserves_same_input_on_original_clock_late_cancel_and_close()
-> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (entry_scope, mut session) = server(&rig, &boundary)?;
    let (source, memory) = registered(&rig, request_snapshot(&rig, &boundary, &session)?)?;
    drop(memory);
    let completed = issue(&rig, &source, 3, run()?)?;
    let snapshot = snapshot(&completed)?;
    let before = rig.client.borrow_registered_restart_admission(snapshot, &source, &completed)?;
    let cutoff = before.effective_until();
    let client = rig.client.clone();
    let original_scope = entry_scope.id();
    rig.clock.on_next_current_thread_observation(Box::new(move || {
        client.core.cancel_scope(original_scope);
    }));
    assert!(rig.client.borrow_registered_restart_admission(snapshot, &source, &completed).is_err());
    assert!(before.check().is_err());
    assert_eq!(before.effective_until(), cutoff);
    assert_eq!(
        snapshot.registered.as_ref().ok_or("Original registration missing")?.previous,
        run()?
    );
    assert!(before.source().same_source(&source));
    drop(completed);
    drop(source);
    session.control.close();
    let terminal = rig.completion(&mut session.completion)?;
    assert!(matches!(terminal.outcome, JobOutcome::Cancelled));
    Ok(())
}
#[test]
fn registered_restart_witness_original_deadline_expiry_cannot_be_reborrowed_or_renewed()
-> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    let (source, memory) = registered(&rig, request_snapshot(&rig, &boundary, &session)?)?;
    drop(memory);
    let completed = issue(&rig, &source, 3, run()?)?;
    let snapshot = snapshot(&completed)?;
    let witness = rig.client.borrow_registered_restart_admission(snapshot, &source, &completed)?;
    let cutoff = witness.effective_until();
    rig.clock.advance(30_000_000_000);
    assert!(witness.check().is_err());
    assert!(rig.client.borrow_registered_restart_admission(snapshot, &source, &completed).is_err());
    assert_eq!(witness.effective_until(), cutoff);
    assert_eq!(
        witness.original_budget().until(),
        completed.spec.deadline.ok_or("Original cutoff missing")?
    );
    drop(completed);
    drop(source);
    close(&rig, &mut session)
}

#[test]
fn registered_restart_witness_rejects_actual_closed_owner_and_keeps_original_completion()
-> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    let (source, memory) = registered(&rig, request_snapshot(&rig, &boundary, &session)?)?;
    drop(memory);
    let completed = issue(&rig, &source, 3, run()?)?;
    let snapshot = snapshot(&completed)?;
    let witness = rig.client.borrow_registered_restart_admission(snapshot, &source, &completed)?;
    let original = witness.original_budget();
    session.control.close();
    assert!(witness.check().is_err());
    assert!(rig.client.borrow_registered_restart_admission(snapshot, &source, &completed).is_err());
    assert_eq!(witness.original_budget(), original);
    assert!(witness.source().same_source(&source));
    assert_eq!(completed.spec.deadline, Some(original.until()));
    drop(completed);
    drop(source);
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(_)));
    Ok(())
}
