// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Preserve unconsumed bindings and registered leases across cancellation and fatal failures.

use super::*;

pub(super) fn terminal_with_failures(
    rig: &Rig,
    job: &mut OwnedJob<ProcessExit, NativeObservationError>,
) -> TestResult<(JobCompletion<ProcessExit, NativeObservationError>, Vec<String>)> {
    let mut failures = Vec::new();
    loop {
        let observed = rig.wake.count();
        if let Some(completion) = job.try_take_completion() {
            return Ok((completion, failures));
        }
        if let Err(error) = rig.client.drain_completions(std::num::NonZeroUsize::MIN) {
            failures.push(error.to_string());
        }
        if let Some(completion) = job.try_take_completion() {
            return Ok((completion, failures));
        }
        rig.wake.wait_after(observed)?;
    }
}

fn ordinary_barrier(rig: &Rig, id: u64) -> TestResult {
    let mut job =
        rig.client.submit_async(&rig.scope, spec(id, JobClass::IoLifecycle, 0)?, |_| async {
            Ok::<_, String>(())
        })?;
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::Success(())));
    Ok(())
}

#[test]
fn explicit_unbound_retirement_preserves_delivery_and_retained_memory() -> TestResult {
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let mut job = permit(&rig, &scope, 1)?.cancel_input();
    let completion = rig.completion(&mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::Cancelled));
    assert_eq!(completion.scope_id, scope.id());
    assert_eq!(completion.spec.id, JobId::new(1)?);
    assert_eq!(scope.snapshot().reserved_bytes, 4096);
    assert!(job.try_take_completion().is_none());
    drop(completion.memory);
    drop(rig.owner);
    assert_eq!(scope.snapshot().terminal_unobserved, 0);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn explicit_unbound_retirement_resolves_an_already_requested_shutdown() -> TestResult {
    let mut rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let reserved = permit(&rig, &scope, 1)?;
    rig.owner.request_shutdown(Moment::from_nanos(0));
    assert!(rig.owner.poll_shutdown()?.unresolved_job_ids.contains(&JobId::new(1)?));
    let mut job = reserved.cancel_input();
    let completion = rig.completion(&mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::Cancelled));
    drop(completion.memory);
    drop(rig.owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    assert_eq!(scope.snapshot().terminal_unobserved, 0);
    Ok(())
}

#[test]
fn explicit_unbound_retirement_preserves_an_expired_original_deadline() -> TestResult {
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let mut request = native_spec(1)?;
    request.deadline = Some(Moment::from_nanos(10));
    let reserved = reserve(&rig.client.core, &scope, request, NativeCorrelation::new(1)?)?;
    rig.clock.advance(10);
    let mut job = reserved.cancel_input();
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::TimedOut));
    Ok(())
}

#[test]
fn cancelled_unbound_permit_keeps_credit_until_consumption_and_actual_handle_close() -> TestResult {
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let reserved = permit(&rig, &scope, 1)?;
    reserved.job.cancel();
    ordinary_barrier(&rig, 2)?;
    assert_eq!(reserved.job.status().phase, JobPhase::Queued);
    assert_eq!(scope.snapshot().reserved_bytes, 4096);
    let boundary = Boundary::new(true, false);
    let mut job = reserved.bind_input(Box::new(Input(boundary.clone())));
    boundary.retiring.wait()?;
    assert_eq!(boundary.starts.load(Ordering::Acquire), 0);
    assert!(job.try_take_completion().is_none());
    assert_eq!(boundary.drops.load(Ordering::Acquire), 0);
    boundary.release.open();
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::Cancelled));
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    assert_eq!(boundary.drops.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn expired_unbound_permit_remains_owned_and_late_bind_never_registers() -> TestResult {
    let mut rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let deadline = Moment::from_nanos(100);
    let mut request = native_spec(1)?;
    request.deadline = Some(deadline);
    let reserved = reserve(&rig.client.core, &scope, request, NativeCorrelation::new(1)?)?;
    rig.clock.wait_until_registered(deadline)?;
    rig.clock.advance(100);
    ordinary_barrier(&rig, 2)?;
    rig.owner.request_shutdown(deadline);
    assert_eq!(rig.owner.poll_shutdown()?.unresolved_job_ids, vec![JobId::new(1)?]);
    assert_eq!(scope.snapshot().queued, 1);
    let boundary = Boundary::new(false, false);
    let mut job = reserved.bind_input(Box::new(Input(boundary.clone())));
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::TimedOut));
    assert_eq!(boundary.starts.load(Ordering::Acquire), 0);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn dropped_unbound_permit_is_abandoned_and_worker_shutdown_actually_joins() -> TestResult {
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let reserved = permit(&rig, &scope, 1)?;
    drop(reserved);
    drop(rig.owner);
    assert_eq!(scope.snapshot().queued, 0);
    assert_eq!(scope.snapshot().running, 0);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn binding_during_another_retirement_preserves_cancelled_input_ownership() -> TestResult {
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let first = Boundary::new(true, false);
    let mut old = permit(&rig, &scope, 1)?.bind_input(Box::new(Input(first.clone())));
    first.started.wait()?;
    old.cancel();
    first.retiring.wait()?;
    let reserved = permit(&rig, &scope, 2)?;
    reserved.job.cancel();
    let second = Boundary::new(false, false);
    let mut job = reserved.bind_input(Box::new(Input(second.clone())));
    assert_eq!(second.closes.load(Ordering::Acquire), 0);
    assert_eq!(scope.snapshot().reserved_bytes, 8192);
    first.release.open();
    assert!(matches!(rig.completion(&mut old)?.outcome, JobOutcome::Cancelled));
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::Cancelled));
    assert_eq!(second.starts.load(Ordering::Acquire), 0);
    assert_eq!(second.closes.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn kernel_start_panic_keeps_other_registration_until_quiescence() -> TestResult {
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let existing = Boundary::new(true, false);
    let mut old = permit(&rig, &scope, 1)?.bind_input(Box::new(Input(existing.clone())));
    existing.started.wait()?;
    let failing = Boundary::new(false, false);
    failing.panic_start.store(true, Ordering::Release);
    let mut job = permit(&rig, &scope, 2)?.bind_input(Box::new(Input(failing.clone())));
    existing.retiring.wait()?;
    assert_eq!(existing.closes.load(Ordering::Acquire), 0);
    assert_eq!(existing.drops.load(Ordering::Acquire), 0);
    assert!(old.try_take_completion().is_none());
    assert_eq!(scope.snapshot().reserved_bytes, 8192);
    existing.release.open();
    let (completion, failures) = terminal_with_failures(&rig, &mut old)?;
    let JobOutcome::Panicked(cause) = completion.outcome else {
        return Err("Other registration lost fatal cause".into());
    };
    assert_eq!(cause.as_str(), "native start panic");
    let (completion, more_failures) = terminal_with_failures(&rig, &mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::Panicked(_)));
    let mut observed = failures.into_iter().chain(more_failures).collect::<Vec<_>>();
    if observed.is_empty() {
        let error = rig
            .client
            .drain_completions(std::num::NonZeroUsize::MIN)
            .err()
            .ok_or("Owner panic cause was not retained")?;
        observed.push(error.to_string());
    }
    assert!(observed.iter().any(|error| error.contains("native start panic")));
    assert_eq!(existing.closes.load(Ordering::Acquire), 1);
    assert_eq!(failing.closes.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn parent_cancellation_retires_native_child_without_losing_scope_ancestry() -> TestResult {
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let boundary = Boundary::new(true, false);
    let mut job = permit(&rig, &scope, 1)?.bind_input(Box::new(Input(boundary.clone())));
    boundary.started.wait()?;
    rig.scope.cancel();
    boundary.retiring.wait()?;
    assert!(scope.snapshot().cancellation_requested);
    assert_eq!(rig.scope.snapshot().children_live, 1);
    assert!(job.try_take_completion().is_none());
    boundary.release.open();
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::Cancelled));
    Ok(())
}

#[test]
fn shutdown_retains_untransferred_native_result_and_accepts_late_abandon() -> TestResult {
    let mut rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let boundary = Boundary::new(false, false);
    let job = permit(&rig, &scope, 1)?.bind_input(Box::new(Input(boundary.clone())));
    boundary.started.wait()?;
    boundary.complete()?;
    super::mailbox::wait_terminal(&rig, 1)?;
    rig.owner.request_shutdown(Moment::from_nanos(0));
    let progress = rig.owner.poll_shutdown()?;
    assert!(!progress.settled);
    assert_eq!(progress.unresolved_job_ids, vec![JobId::new(1)?]);
    assert!(rig.owner.finish_shutdown().is_err());
    assert_eq!(scope.snapshot().reserved_bytes, 4096);
    drop(job);
    drop(rig.owner);
    assert_eq!(scope.snapshot().terminal_unobserved, 0);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn kernel_read_and_retirement_panic_do_not_free_callback_storage() -> TestResult {
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let boundary = Boundary::new(true, false);
    boundary.panic_read.store(true, Ordering::Release);
    boundary.panic_retire.store(true, Ordering::Release);
    let mut job = permit(&rig, &scope, 1)?.bind_input(Box::new(Input(boundary.clone())));
    boundary.started.wait()?;
    boundary.complete()?;
    boundary.retiring.wait()?;
    assert_eq!(boundary.drops.load(Ordering::Acquire), 0);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 0);
    assert!(job.try_take_completion().is_none());
    boundary.release.open();
    rig.client.core.changed();
    drop(rig.owner);
    // Defensive owner teardown abandons the cell but observes native cleanup.
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    assert_eq!(boundary.drops.load(Ordering::Acquire), 1);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
