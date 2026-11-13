// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual qualification decisions without creating a process or native loop.
use super::*;
mod construction;
use sugarsubstitute_contracts::lifecycle::{ProcessId, ProcessKey, ShutdownBudget};
use sugarsubstitute_desktop::HeadlessLifetime;
use sugarsubstitute_execution::qualification_time::anchor;
use sugarsubstitute_supervision::RegistryObservation;

fn key(fixture: &Fixture, id: u64) -> Result<ProcessKey> {
    Ok(ProcessKey { owner: fixture.owner_key, id: ProcessId::new(id)? })
}

#[test]
fn case259_needs_actual_start_exit_settlement_and_retirement() -> Result<()> {
    let fixture = Fixture::new()?;
    let process = key(&fixture, 1)?;
    let mut case = CaseProgress::new(259);
    case.bind(process)?;
    case.observe(RegistryObservation::RootExited { process, code: 259 })?;
    assert!(!case.complete());
    case.observe(RegistryObservation::ContainedStarted { process })?;
    assert!(!case.complete());
    let shutdown = ShutdownId::new(1)?;
    case.stop = Some((shutdown, ShutdownBudget::new(Moment::from_nanos(7))?));
    case.observe(RegistryObservation::FamilySettled { process, shutdown: Some(shutdown) })?;
    assert!(!case.complete());
    case.retired = true;
    assert!(case.complete());
    Ok(())
}

#[test]
fn foreign_settlement_and_stop_mismatch_cannot_complete_case() -> Result<()> {
    let fixture = Fixture::new()?;
    let process = key(&fixture, 1)?;
    let mut case = CaseProgress::new(0);
    case.bind(process)?;
    assert!(case.bind(key(&fixture, 2)?).is_err());
    assert!(
        case.observe(RegistryObservation::ContainedStarted { process: key(&fixture, 2)? }).is_err()
    );
    assert!(!case.started);
    let shutdown = ShutdownId::new(1)?;
    case.stop = Some((shutdown, ShutdownBudget::new(Moment::from_nanos(0))?));
    assert!(case.observe(RegistryObservation::FamilySettled { process, shutdown: None }).is_err());
    assert!(
        case.observe(RegistryObservation::FamilySettled {
            process,
            shutdown: Some(ShutdownId::new(2)?)
        })
        .is_err()
    );
    assert!(!case.settled);
    Ok(())
}

#[test]
fn unexpected_actual_code_is_retained_without_claiming_success() -> Result<()> {
    let fixture = Fixture::new()?;
    let process = key(&fixture, 1)?;
    let mut case = CaseProgress::new(37);
    case.bind(process)?;
    assert!(case.observe(RegistryObservation::RootExited { process, code: 0 }).is_err());
    assert_eq!(case.exited, Some(0));
    assert!(!case.complete());
    Ok(())
}

#[test]
fn observation_retirement_cannot_replace_immediate_fixture_exit_evidence() -> Result<()> {
    let fixture = Fixture::new()?;
    let process = key(&fixture, 1)?;
    let mut case = CaseProgress::new(0);
    case.bind(process)?;
    assert!(
        case.observe(RegistryObservation::RootObservationRetired {
            process,
            job: JobId::new(1)?,
            outcome: sugarsubstitute_supervision::RootObservationOutcome::Cancelled,
        })
        .is_err()
    );
    assert_eq!(case.exited, None);
    assert!(!case.complete());
    Ok(())
}

#[test]
fn grace_and_total_are_consumed_once_without_resetting_original_budget() -> Result<()> {
    let origin = anchor();
    let mut plan = StopPlan::new(Moment::from_nanos(17), origin)?;
    let budget = plan.budget;
    assert_eq!(plan.next_deadline(), origin.checked_add(Duration::from_secs(10)));
    assert!(!plan.due(Moment::from_nanos(10_000_000_016), false).cleanup);
    assert!(plan.due(budget.graceful_until, false).cleanup);
    assert!(!plan.due(budget.graceful_until, false).cleanup);
    assert_eq!(plan.next_deadline(), origin.checked_add(Duration::from_secs(15)));
    assert!(plan.due(budget.settle_until, false).cleanup);
    assert!(!plan.due(Moment::from_nanos(u64::MAX), false).cleanup);
    assert_eq!(plan.next_deadline(), None);
    assert_eq!(plan.budget, budget);
    assert!(StopPlan::new(Moment::from_nanos(u64::MAX), origin).is_err());
    Ok(())
}

#[test]
fn unsettled_total_expiry_is_one_failure_and_still_allows_late_actual_cleanup() -> Result<()> {
    let fixture = Fixture::new()?;
    let process = key(&fixture, 1)?;
    let mut case = CaseProgress::new(0);
    case.bind(process)?;
    case.observe(RegistryObservation::ContainedStarted { process })?;
    case.observe(RegistryObservation::RootExited { process, code: 0 })?;
    let mut plan = StopPlan::new(Moment::from_nanos(0), anchor())?;
    let shutdown = ShutdownId::new(1)?;
    case.stop = Some((shutdown, plan.budget));
    let due = plan.due(plan.budget.settle_until, case.settled);
    assert!(due.cleanup);
    assert!(due.expired);
    assert!(!case.settled);
    assert!(!case.complete());
    assert!(!plan.due(Moment::from_nanos(20_000_000_000), false).expired);
    case.observe(RegistryObservation::FamilySettled { process, shutdown: Some(shutdown) })?;
    assert!(case.settled);
    assert_eq!(case.stop.map(|(_, budget)| budget), Some(plan.budget));
    assert!(!plan.due(Moment::from_nanos(20_000_000_000), true).expired);
    let mut settled_on_observation = StopPlan::new(Moment::from_nanos(0), anchor())?;
    assert!(!settled_on_observation.due(Moment::from_nanos(20_000_000_000), true).expired);
    Ok(())
}

#[test]
fn the_earlier_wake_wins_and_closure_does_not_reset_or_launch() -> Result<()> {
    let mut fixture = Fixture::new()?;
    let custody = fixture.execution_deadline;
    let stall = fixture.watch.next_deadline().ok_or("Stall wake missing")?;
    let earlier = anchor();
    let stop = StopPlan::new(Moment::from_nanos(0), earlier)?;
    let grace = stop.next_deadline().ok_or("Grace wake missing")?;
    fixture.stop = Some(stop);
    assert_eq!(fixture.next_deadline(), Some(grace.min(stall)));
    fixture.request_shutdown()?;
    fixture.request_shutdown()?;
    fixture.launch_next()?;
    assert_eq!(fixture.execution_deadline, custody);
    assert!(fixture.cases.iter().all(|case| case.process.is_none()));
    assert_eq!(fixture.current, 0);
    assert!(fixture.owner.is_none());
    Ok(())
}

#[test]
fn job_identities_advance_watch_before_step_and_reject_overflow() -> Result<()> {
    let mut fixture = Fixture::new()?;
    let watch = fixture.spec(JobClass::NativeObservation, 4096)?;
    let step = fixture.spec(JobClass::InfrastructureLifecycle, 4096)?;
    assert!(watch.id < step.id);
    assert_eq!(watch.operation, Operation::NativeLifecycle);
    assert_eq!(watch.deadline, None);
    fixture.next_job = u64::MAX;
    assert!(fixture.spec(JobClass::NativeObservation, 4096).is_err());
    Ok(())
}

#[test]
fn receipt_refuses_requested_shutdown_without_actual_case_and_worker_evidence() -> Result<()> {
    let mut fixture = Fixture::new()?;
    fixture.request_shutdown()?;
    assert!(fixture.poll_shutdown()?);
    fixture.finish_shutdown()?;
    assert!(fixture.receipt().is_err());
    assert!(fixture.workers.is_none());
    Ok(())
}

#[test]
fn a_stalled_campaign_reports_once_and_does_not_create_new_cases() -> Result<()> {
    let mut fixture = Fixture::new()?;
    fixture.watch = crate::campaign_watch::CampaignWatch::silent("Registry qualification");
    assert!(fixture.advance(NonZeroUsize::MIN).is_err());
    assert!(fixture.watch.reported());
    fixture.request_shutdown()?;
    assert_eq!(fixture.next_deadline(), None);
    assert_eq!(
        fixture.advance(NonZeroUsize::MIN)?,
        sugarsubstitute_desktop::HeadlessStep::Shutdown
    );
    assert!(fixture.cases.iter().all(|case| case.process.is_none()));
    Ok(())
}

#[test]
fn registry_failure_preserves_original_typed_source_through_case_observation() -> Result<()> {
    use sugarsubstitute_contracts::execution::{
        CompletionBatch, CompletionNotification, TerminalKind,
    };
    let mut fixture = Fixture::new()?;
    construction::prepare_registry(&mut fixture)?;
    let notification = CompletionNotification {
        scope_id: ScopeId::new(1)?,
        job_id: JobId::new(1)?,
        kind: TerminalKind::Cancelled,
        replacement: None,
    };
    let error = fixture
        .registry
        .as_mut()
        .ok_or("Missing actual admitted registry")?
        .consume(&CompletionBatch { notifications: vec![notification; 33], remaining: 0 })
        .err()
        .ok_or("Missing real registry boundary failure")?;
    let process = key(&fixture, 1)?;
    let mut case = CaseProgress::new(0);
    case.bind(process)?;
    let transferred = case
        .observe(RegistryObservation::Failed { process, error })
        .err()
        .ok_or("Missing transferred registry cause")?;
    let native = transferred
        .downcast_ref::<sugarsubstitute_supervision::RegistryError>()
        .ok_or("Original registry error type was lost")?;
    assert_eq!(native.stage(), "completion_batch_bound");
    assert!(!native.contains_owner_panic());
    assert!(native.source().and_then(|cause| cause.downcast_ref::<std::io::Error>()).is_some());
    assert!(!case.settled);
    assert!(!case.retired);
    Ok(())
}
