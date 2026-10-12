// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep independent cleanup boundaries consumable without repeated failure-driven work.
use super::{Result, evidence::Stop, parent::Parent};
use sugarsubstitute_contracts::{
    execution::{CompletionBatch, CompletionNotification, JobId, Moment, ScopeId, TerminalKind},
    lifecycle::{ProcessId, ProcessKey, ShutdownId},
};
use sugarsubstitute_supervision::{FamilyBinding, RegistrySnapshot};
fn snapshot(parent: &Parent) -> Result<RegistrySnapshot> {
    let case = &parent.cases[0];
    Ok(RegistrySnapshot {
        process: ProcessKey { owner: case.resource.owner, id: ProcessId::new(1)? },
        binding: FamilyBinding::Managed(case.resource),
        creation_pending: false,
        started: true,
        launch_closed: true,
        root_exit: Some(0),
        last_member_exit: None,
        forced: false,
        retained_members: 1,
        cleanup_pending: false,
        member_watch_required: false,
        settled: false,
        failed: false,
        shutdown: None,
    })
}
#[test]
fn failed_duplicate_and_cancelled_unused_watch_do_not_rearm_unchanged_cleanup() -> Result<()> {
    let mut parent = Parent::new()?;
    let mut state = snapshot(&parent)?;
    state.failed = true;
    state.member_watch_required = true;
    let case = &mut parent.cases[0];
    case.cleanup_job = Some(JobId::new(10)?);
    case.member_job = Some(JobId::new(9)?);
    case.capture(&CompletionBatch {
        notifications: vec![
            CompletionNotification {
                scope_id: ScopeId::new(1)?,
                job_id: JobId::new(10)?,
                kind: TerminalKind::Failure,
                replacement: None,
            },
            CompletionNotification {
                scope_id: ScopeId::new(2)?,
                job_id: JobId::new(9)?,
                kind: TerminalKind::Cancelled,
                replacement: None,
            },
        ],
        remaining: 0,
    })?;
    for _ in 0..32 {
        case.reconsider_cleanup(state);
        assert!(!case.cleanup_requested);
    }
    assert!(case.cleanup_job.is_none() && case.member_job.is_none());
    case.cleanup_success = true;
    case.reconsider_cleanup(state);
    assert!(case.cleanup_requested);
    case.cleanup_requested = false;
    case.reconsider_cleanup(state);
    assert!(!case.cleanup_requested);
    Ok(())
}
#[test]
fn partial_settlement_remains_retirable_without_admitting_a_new_stop() -> Result<()> {
    let mut parent = Parent::new()?;
    let mut state = snapshot(&parent)?;
    state.started = false;
    state.failed = true;
    state.settled = true;
    let case = &mut parent.cases[0];
    case.process = Some(state.process);
    assert!(!case.needs_stop(state));
    assert!(case.stop.is_none());
    assert!(!case.complete());
    // Failed qualification does not retract the independent native settlement fact.
    state.settled = false;
    assert!(case.needs_stop(state));
    Ok(())
}
#[test]
fn settled_family_consumes_total_boundary_while_independent_output_is_held() -> Result<()> {
    let mut stop =
        Stop::new(ShutdownId::new(1)?, Moment::from_nanos(100), std::time::Instant::now())?;
    assert_eq!(stop.due(stop.budget.graceful_until), (true, false));
    assert_eq!(stop.next(), Some(stop.total));
    assert_eq!(stop.due(stop.budget.settle_until), (false, true));
    assert_eq!(stop.next(), None);
    assert_eq!(stop.due(stop.budget.settle_until), (false, false));
    Ok(())
}
#[test]
fn retired_family_does_not_schedule_past_cleanup_timers_for_held_output() -> Result<()> {
    use sugarsubstitute_desktop::HeadlessLifetime;
    let mut parent = Parent::new()?;
    let case = &mut parent.cases[0];
    case.retired = true;
    case.drained = false;
    case.stop =
        Some(Stop::new(ShutdownId::new(1)?, Moment::from_nanos(0), std::time::Instant::now())?);
    assert_eq!(parent.next_deadline(), Some(parent.deadline));
    parent.deadline_observed = true;
    assert_eq!(parent.next_deadline(), None);
    Ok(())
}
#[test]
fn pending_descendant_watch_allows_due_finite_cleanup_without_duplicate_admission() -> Result<()> {
    let mut parent = Parent::new()?;
    let mut state = snapshot(&parent)?;
    state.cleanup_pending = true;
    let case = &mut parent.cases[0];
    case.stop =
        Some(Stop::new(ShutdownId::new(1)?, Moment::from_nanos(100), std::time::Instant::now())?);
    case.member_job = Some(JobId::new(9)?);
    case.cleanup_requested = true;
    assert!(case.can_submit_cleanup(state));
    case.cleanup_job = Some(JobId::new(10)?);
    assert!(!case.can_submit_cleanup(state));
    case.cleanup_job = None;
    case.cleanup_requested = false;
    assert!(!case.can_submit_cleanup(state));
    case.cleanup_requested = true;
    state.settled = true;
    assert!(!case.can_submit_cleanup(state));
    Ok(())
}

#[test]
fn force_step_preserves_member_notice_and_rearms_actual_exit_settlement() -> Result<()> {
    let mut parent = Parent::new()?;
    let mut state = snapshot(&parent)?;
    state.cleanup_pending = true;
    let case = &mut parent.cases[0];
    let member = JobId::new(9)?;
    let force = JobId::new(10)?;
    case.member_job = Some(member);
    case.record_cleanup_admission(force, None);
    assert_eq!(case.member_job, Some(member));
    case.capture(&CompletionBatch {
        notifications: vec![CompletionNotification {
            scope_id: ScopeId::new(1)?,
            job_id: force,
            kind: TerminalKind::Success,
            replacement: None,
        }],
        remaining: 0,
    })?;
    case.reconsider_cleanup(state);
    assert!(!case.cleanup_requested);
    case.capture(&CompletionBatch {
        notifications: vec![CompletionNotification {
            scope_id: ScopeId::new(2)?,
            job_id: member,
            kind: TerminalKind::Success,
            replacement: None,
        }],
        remaining: 0,
    })?;
    assert!(case.member_success && case.member_job.is_none());
    state.cleanup_pending = false;
    // The exact native exit is retained independently by the registry consumer.
    state.last_member_exit = Some(sugarsubstitute_execution::windows::ProcessExit {
        correlation: sugarsubstitute_execution::windows::NativeCorrelation::new(1)?,
        process_id: 123,
        creation_time_100ns: 456,
        exit_code: 1,
    });
    case.reconsider_cleanup(state);
    assert!(case.cleanup_requested);
    case.cleanup_requested = false;
    case.reconsider_cleanup(state);
    assert!(!case.cleanup_requested);
    Ok(())
}
