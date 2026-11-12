// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve the existing Parent exit transaction through its shared adapter transition.
//! These tests supply external exit facts; they construct no authenticated recovery member.
use super::*;
use std::error::Error;
use sugarsubstitute_contracts::{
    configuration::ProfileId,
    execution::Moment,
    lifecycle::{CompletionIntent, DesktopExitKind, ProcessId, ShutdownId},
};
type TestResult<T = ()> = Result<T, Box<dyn Error>>;
fn actor() -> TestResult<(ParentLifecycle, ProcessKey)> {
    let identity = super::super::tests::identity()?;
    let storage = crate::production::coordinator_backend::tests::storage_tests::claim(
        ParentLifecycle::storage_bytes(),
    )
    .map_err(|cause| cause as Box<dyn Error>)?;
    let parent = ParentLifecycle::new(
        identity.run,
        identity.launch,
        ProfileId::new("Exit_1".into())?,
        ConnectionId::new(1)?,
        super::super::LifecyclePipeDeadline::new(100, 1_000_000_000)?,
        &mut Some(super::super::causes::BackendCauses::new(storage)?),
    )?;
    Ok((parent, ProcessKey { owner: identity.run.owner, id: ProcessId::new(7)? }))
}
#[test]
fn parent_shared_exit_transition_preserves_original_family_full_status_and_duplicate_custody()
-> TestResult {
    let (mut parent, process) = actor()?;
    let code = 0xc000_0005;
    parent.observe_desktop_exit(process, code)?;
    let fact =
        parent.failed_incident(parent.run, process)?.ok_or("Original failed exit missing")?;
    assert_eq!(fact.actual_status(), i64::from(code));
    assert_eq!(fact.attribution(), DesktopExitKind::Unexpected);
    assert_eq!(fact.process(), process);
    assert_eq!(fact.run(), parent.run);
    assert!(matches!(parent.observe_desktop_exit(process, 0), Err(ParentError::WrongPhase)));
    let changed = ProcessKey { id: ProcessId::new(8)?, ..process };
    assert!(matches!(parent.observe_desktop_exit(changed, 0), Err(ParentError::WrongPhase)));
    assert_eq!(parent.failed_incident(parent.run, process)?, Some(fact));
    assert!(parent.failed_incident(parent.run, changed).is_err());
    assert!(parent.normal_close_request().is_none());
    assert!(!parent.native_retired());
    assert!(!parent.settled());
    assert_eq!(parent.shutdown(), None);
    Ok(())
}
#[test]
fn parent_shared_exit_transition_retains_completed_intent_budget_and_independent_settlement()
-> TestResult {
    for (intent, kind) in [
        (CompletionIntent::Normal, DesktopExitKind::Normal),
        (CompletionIntent::StartupCancelled, DesktopExitKind::StartupCancelled),
        (CompletionIntent::StartupFailed, DesktopExitKind::StartupFailed),
    ] {
        let (mut parent, process) = actor()?;
        let mut exit = sugarsubstitute_application::supervision::DesktopExitTransaction::new(
            parent.run, process,
        )?;
        let shutdown = ShutdownId::new(1)?;
        let original = exit.begin(parent.run, shutdown, intent, 0, Moment::from_nanos(10))?;
        exit.observe(DesktopObservation::Completed { run: parent.run, shutdown })
            .map_err(|rejected| rejected.reason)?;
        parent.exit = Some(exit);
        parent.cleanup = Some(original);
        parent.observe_desktop_exit(process, 0)?;
        let snapshot = parent
            .exit
            .as_mut()
            .ok_or("Actual exit transaction missing")?
            .poll(Moment::from_nanos(11))?;
        assert_eq!(snapshot.exit, Some(kind));
        assert_eq!(snapshot.budget, Some(original));
        assert!(!snapshot.resources_settled);
        assert!(!snapshot.settled);
        assert_eq!(parent.shutdown(), Some(original));
        if intent == CompletionIntent::StartupFailed {
            assert_eq!(
                parent
                    .failed_incident(parent.run, process)?
                    .ok_or("Failed startup missing")?
                    .attribution(),
                kind
            );
        } else {
            let close = parent
                .exit
                .as_ref()
                .ok_or("Actual exit missing")?
                .normal_close_request()
                .ok_or("Original deliberate close lost")?;
            assert_eq!(close.process(), process);
            assert_eq!(close.run(), parent.run);
            assert_eq!(close.budget(), original);
            assert!(close.exited());
        }
        assert!(!parent.native_retired());
        assert!(!parent.settled());
    }
    Ok(())
}
