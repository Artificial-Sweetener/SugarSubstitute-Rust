// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove passive intent observation without replacing exit attribution or original deadlines.
use super::*;
fn fixture() -> Result<(DesktopExitTransaction, RunKey, ProcessKey), Box<dyn std::error::Error>> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let run = RunKey { owner, instance: InstanceId::new(1)?, generation: RunGeneration::new(1)? };
    let process = ProcessKey { owner, id: ProcessId::new(2)? };
    Ok((DesktopExitTransaction::new(run, process)?, run, process))
}
#[test]
fn passive_normal_close_query_preserves_original_budget_and_does_not_advance_clock()
-> Result<(), Box<dyn std::error::Error>> {
    let (mut owner, run, process) = fixture()?;
    assert!(owner.normal_close_request().is_none());
    let shutdown = ShutdownId::new(1)?;
    let at = Moment::from_nanos(100);
    let original = owner.begin(run, shutdown, CompletionIntent::Normal, 0, at)?;
    for _ in 0..64 {
        let request = owner.normal_close_request().ok_or("Accepted intent lost")?;
        assert_eq!(request.run(), run);
        assert_eq!(request.process(), process);
        assert_eq!(request.intent(), CompletionIntent::Normal);
        assert_eq!(request.budget(), original);
    }
    assert_eq!(
        owner.begin(run, shutdown, CompletionIntent::Normal, 0, Moment::from_nanos(101))?,
        original
    );
    assert!(owner.poll(Moment::from_nanos(100)).is_err());
    assert_eq!(owner.poll(Moment::from_nanos(101))?.budget, Some(original));
    assert!(owner.failed_incident().is_err());
    assert!(!owner.poll(Moment::from_nanos(101))?.settled);
    Ok(())
}
#[test]
fn passive_close_query_retains_clean_normal_and_cancelled_actual_exit_without_claiming_settlement()
-> Result<(), Box<dyn std::error::Error>> {
    for intent in [CompletionIntent::Normal, CompletionIntent::StartupCancelled] {
        let (mut owner, run, process) = fixture()?;
        let shutdown = ShutdownId::new(1)?;
        let budget = owner.begin(run, shutdown, intent, 0, Moment::from_nanos(1))?;
        owner.observe(DesktopObservation::Completed { run, shutdown }).map_err(|e| e.reason)?;
        owner.observe(DesktopObservation::Exited { process, code: 0 }).map_err(|e| e.reason)?;
        assert_eq!(owner.normal_close_request().map(DesktopCloseRequest::budget), Some(budget));
        assert!(owner.failed_incident().is_err());
        assert!(!owner.poll(Moment::from_nanos(1))?.resources_settled);
    }
    Ok(())
}
#[test]
fn passive_close_query_never_reclassifies_failed_hard_or_wrong_status_exits()
-> Result<(), Box<dyn std::error::Error>> {
    for (intent, completed, code) in [
        (CompletionIntent::Normal, false, 0),
        (CompletionIntent::Normal, true, 7),
        (CompletionIntent::StartupFailed, true, 0),
    ] {
        let (mut owner, run, process) = fixture()?;
        let shutdown = ShutdownId::new(1)?;
        owner.begin(run, shutdown, intent, 0, Moment::from_nanos(1))?;
        if completed {
            owner.observe(DesktopObservation::Completed { run, shutdown }).map_err(|e| e.reason)?;
        }
        owner.observe(DesktopObservation::Exited { process, code }).map_err(|e| e.reason)?;
        let original = owner.failed_incident()?;
        assert!(owner.normal_close_request().is_none());
        assert_eq!(owner.failed_incident()?, original);
        assert!(owner.observe(DesktopObservation::Completed { run, shutdown }).is_err());
        assert!(owner.normal_close_request().is_none());
    }
    Ok(())
}
#[test]
fn supervisor_cleanup_without_authenticated_deliberate_intent_never_requests_normal_idle()
-> Result<(), Box<dyn std::error::Error>> {
    let (mut owner, run, process) = fixture()?;
    owner.observe(DesktopObservation::Exited { process, code: 0 }).map_err(|e| e.reason)?;
    let fact = owner.failed_incident()?;
    owner.begin_cleanup(run, ShutdownId::new(1)?, Moment::from_nanos(20))?;
    assert!(owner.normal_close_request().is_none());
    assert_eq!(owner.failed_incident()?, fact);
    Ok(())
}
