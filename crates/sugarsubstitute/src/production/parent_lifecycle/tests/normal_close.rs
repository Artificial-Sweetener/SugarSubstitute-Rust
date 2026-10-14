// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow exact deliberate intent without manufacturing native registration or settlement proof.
use super::*;
use sugarsubstitute_application::supervision::DesktopExitTransaction;
use sugarsubstitute_contracts::{configuration::ProfileId, lifecycle::*};

fn actor() -> Result<ParentLifecycle> {
    actor_for(1)
}
fn actor_for(instance: u64) -> Result<ParentLifecycle> {
    let mut id = identity()?;
    id.run.instance = InstanceId::new(instance)?;
    let storage = crate::production::coordinator_backend::tests::storage_tests::claim(
        ParentLifecycle::storage_bytes(),
    )
    .map_err(|cause| cause as Box<dyn Error>)?;
    Ok(ParentLifecycle::new(
        id.run,
        id.launch,
        ProfileId::new("Close_1".into())?,
        ConnectionId::new(1)?,
        sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeDeadline::new(
            100,
            1_000_000_000,
        )?,
        storage,
    )?)
}
fn intent(parent: &mut ParentLifecycle, kind: CompletionIntent) -> Result<ProcessKey> {
    intent_at(parent, kind, Moment::from_nanos(1))
}
fn intent_at(
    parent: &mut ParentLifecycle,
    kind: CompletionIntent,
    at: Moment,
) -> Result<ProcessKey> {
    let process =
        ProcessKey { owner: parent.run.owner, id: ProcessId::new(parent.run.instance.get())? };
    let mut exit = DesktopExitTransaction::new(parent.run, process)?;
    parent.cleanup = Some(exit.begin(parent.run, ShutdownId::new(1)?, kind, 0, at)?);
    parent.exit = Some(exit);
    Ok(process)
}
#[test]
fn normal_parent_intent_is_exact_passive_and_does_not_require_close_produced_settlement()
-> Result<()> {
    let mut parent = actor()?;
    let process = intent(&mut parent, CompletionIntent::Normal)?;
    assert!(parent.normal_close_request().is_none());
    // Registration is an external owner fact for this inert query. This creates no native peer/proof.
    parent.registered = true;
    let original = parent.cleanup.ok_or("Original budget missing")?;
    assert!(!parent.normal_cleanup_ready());
    for _ in 0..64 {
        let request = parent.normal_close_request().ok_or("Accepted normal intent missing")?;
        assert_eq!(request.run(), parent.run);
        assert_eq!(request.process(), process);
        assert_eq!(request.budget(), original);
        assert_eq!(request.intent(), CompletionIntent::Normal);
    }
    assert!(!parent.native_retired());
    assert!(!parent.settled());
    assert!(parent.retired_registration(parent.run, parent.connection)?.is_none());
    assert_eq!(
        parent.exit.as_mut().ok_or("Exit missing")?.poll(Moment::from_nanos(1))?.budget,
        Some(original)
    );
    Ok(())
}
#[test]
fn parent_close_query_rejects_foreign_run_changed_budget_and_original_failure() -> Result<()> {
    let mut parent = actor()?;
    intent(&mut parent, CompletionIntent::Normal)?;
    parent.registered = true;
    let original = parent.cleanup;
    parent.cleanup = Some(ShutdownBudget::new(Moment::from_nanos(2))?);
    assert!(parent.normal_close_request().is_none());
    parent.cleanup = original;
    let original_run = parent.run;
    parent.run.generation = RunGeneration::new(2)?;
    assert!(parent.normal_close_request().is_none());
    parent.run = original_run;
    parent.failure = Some(Box::new(std::io::Error::other("Original owned failure")));
    assert!(parent.normal_close_request().is_none());
    assert_eq!(parent.cleanup, original);
    Ok(())
}
#[test]
fn parent_close_query_preserves_hard_exit_and_startup_failure_attribution() -> Result<()> {
    for kind in [CompletionIntent::Normal, CompletionIntent::StartupFailed] {
        let mut parent = actor()?;
        let process = intent(&mut parent, kind)?;
        parent.registered = true;
        parent
            .exit
            .as_mut()
            .ok_or("Exit missing")?
            .observe(DesktopObservation::Exited { process, code: 0 })
            .map_err(|e| e.reason)?;
        let fact = parent.exit.as_ref().ok_or("Exit missing")?.failed_incident()?;
        assert_eq!(fact.attribution(), DesktopExitKind::Unexpected);
        assert!(parent.normal_close_request().is_none());
        assert_eq!(parent.exit.as_ref().ok_or("Exit missing")?.failed_incident()?, fact);
    }
    Ok(())
}

#[test]
fn normal_parent_cleanup_waits_for_exact_final_written_receipt_or_matching_deliberate_exit()
-> Result<()> {
    let mut parent = actor()?;
    let process = intent(&mut parent, CompletionIntent::Normal)?;
    parent.registered = true;
    let (index, _) = parent.requests.admit(header(1)?, b"original ExitCompleted request")?;
    parent.close_after_reply = Some(index);
    parent.closed = true;
    assert!(!parent.normal_cleanup_ready());
    // Written/sent is external transport metadata; no native lease or retirement is manufactured.
    parent.requests.get_mut(index)?.sent = true;
    assert!(parent.normal_cleanup_ready());
    parent.close_after_reply = Some(index + 1);
    assert!(!parent.normal_cleanup_ready());
    parent.close_after_reply = None;
    assert!(!parent.normal_cleanup_ready());
    let run = parent.run;
    let exit = parent.exit.as_mut().ok_or("Exit missing")?;
    exit.observe(DesktopObservation::Completed { run, shutdown: ShutdownId::new(1)? })
        .map_err(|e| e.reason)?;
    exit.observe(DesktopObservation::Exited { process, code: 0 }).map_err(|e| e.reason)?;
    assert!(parent.normal_cleanup_ready());
    assert!(!parent.native_retired());
    assert!(!parent.settled());
    Ok(())
}

#[test]
fn normal_parent_distinct_original_role_budgets_survive_tighter_global_cleanup_without_changed_duplicate()
-> Result<()> {
    let mut first = actor_for(1)?;
    let mut second = actor_for(2)?;
    intent_at(&mut first, CompletionIntent::Normal, Moment::from_nanos(10))?;
    intent_at(&mut second, CompletionIntent::Normal, Moment::from_nanos(30))?;
    let original_first = first.shutdown().ok_or("First original missing")?;
    let original_second = second.shutdown().ok_or("Second original missing")?;
    let global = original_first;
    assert!(matches!(second.validate_cleanup_budget(global), Err(ParentError::ChangedDuplicate)));
    for requested in [
        global,
        ShutdownBudget::new(Moment::from_nanos(1))?,
        ShutdownBudget::new(Moment::from_nanos(60))?,
    ] {
        let first_argument = first.cleanup_budget(requested);
        let second_argument = second.cleanup_budget(requested);
        first.validate_cleanup_budget(first_argument)?;
        second.validate_cleanup_budget(second_argument)?;
        assert_eq!(first_argument, original_first);
        assert_eq!(second_argument, original_second);
        assert_eq!(first.shutdown(), Some(original_first));
        assert_eq!(second.shutdown(), Some(original_second));
    }
    assert_eq!(global, original_first);
    assert!(global.settle_until < original_second.settle_until);
    assert!(!first.native_retired());
    assert!(!second.native_retired());
    Ok(())
}
