// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual execution owner while faking only its external clock/wake.
use super::*;
use sugarsubstitute_contracts::execution::{ScopeId, WaitFuture};
struct ClockBoundary;
impl Clock for ClockBoundary {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, deadline: Moment) -> WaitFuture {
        if deadline == Moment::from_nanos(0) {
            Box::pin(std::future::ready(()))
        } else {
            Box::pin(std::future::pending())
        }
    }
}
struct WakeBoundary;
impl CompletionWake for WakeBoundary {
    fn wake(&self) -> Result<(), sugarsubstitute_contracts::execution::ExecutionError> {
        Ok(())
    }
}
#[test]
fn inert_inputs_cannot_access_client_or_original_execution_clock_before_real_start()
-> Result<(), Box<dyn Error>> {
    let execution = StartupExecution::new(Arc::new(ClockBoundary));
    assert!(!execution.has_owner());
    assert!(execution.client().is_err());
    assert!(execution.now().is_err());
    assert!(execution.scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }).is_err());
    assert!(execution.drain(NonZeroUsize::MIN).is_err());
    Ok(())
}
#[test]
fn real_start_is_once_and_cleanup_keeps_same_owner_admission_open_until_actual_owners_settle()
-> Result<(), Box<dyn Error>> {
    let mut execution = StartupExecution::new(Arc::new(ClockBoundary));
    execution.start(Arc::new(WakeBoundary))?;
    assert!(execution.has_owner());
    let render = execution.scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None })?;
    assert!(execution.start(Arc::new(WakeBoundary)).is_err());
    let child = execution.scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None })?;
    assert_eq!(render.owner_id(), child.owner_id());
    assert_eq!(execution.now()?, Moment::from_nanos(0));
    execution.retain_shutdown_cutoff(Moment::from_nanos(100));
    assert!(!execution.poll_shutdown(false)?);
    assert!(!execution.shutting_down);
    let cleanup = execution.scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None })?;
    assert_eq!(cleanup.owner_id(), child.owner_id());
    assert!(execution.drain(NonZeroUsize::MIN)?.notifications.is_empty());
    assert!(execution.finish().is_err());
    assert!(execution.has_owner());
    // Zero-work real owner Drop cancels and joins its own workers; it is no child/native settlement proof.
    drop((render, child, cleanup));
    drop(execution);
    Ok(())
}
#[test]
fn executor_cutoff_can_only_tighten_and_unsettled_children_cannot_close_admission()
-> Result<(), Box<dyn Error>> {
    let mut execution = StartupExecution::new(Arc::new(ClockBoundary));
    execution.retain_shutdown_cutoff(Moment::from_nanos(100));
    execution.retain_shutdown_cutoff(Moment::from_nanos(200));
    assert_eq!(execution.shutdown_until, Some(Moment::from_nanos(100)));
    execution.retain_shutdown_cutoff(Moment::from_nanos(50));
    assert_eq!(execution.shutdown_until, Some(Moment::from_nanos(50)));
    assert!(!execution.poll_shutdown(false)?);
    assert!(!execution.shutting_down);
    Ok(())
}
