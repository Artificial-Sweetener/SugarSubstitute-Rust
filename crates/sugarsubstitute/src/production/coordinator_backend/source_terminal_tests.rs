// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive source terminal adoption and explicit cleanup through real execution and policy owners.
use super::{consume_source, *};
use crate::production::coordinator_backend::{cutoff_tests, tests as backend_tests};
use std::{
    cell::Cell,
    num::NonZeroUsize,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::{Clock, CompletionWake, ExecutionError, WaitFuture};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};
use sugarsubstitute_supervision::installed_frontend::boundary_fixture;

thread_local! { static CALLER: Cell<bool> = const { Cell::new(false) }; }
#[derive(Default)]
struct HeldClock {
    now: AtomicU64,
    held: Mutex<bool>,
    released: Condvar,
}
impl Clock for HeldClock {
    fn now(&self) -> Moment {
        if !CALLER.with(Cell::get) {
            let guard = self.held.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            drop(
                self.released
                    .wait_while(guard, |held| *held)
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            );
        }
        Moment::from_nanos(self.now.load(Ordering::Acquire))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
struct Hold {
    clock: Arc<HeldClock>,
    previous: bool,
}
impl Hold {
    fn new(clock: Arc<HeldClock>) -> Self {
        let previous = CALLER.with(|caller| caller.replace(true));
        *clock.held.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = true;
        Self { clock, previous }
    }
}
impl Drop for Hold {
    fn drop(&mut self) {
        *self.clock.held.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = false;
        self.clock.released.notify_all();
        CALLER.with(|caller| caller.set(self.previous));
    }
}
#[derive(Default)]
struct Wake {
    revision: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> std::result::Result<(), ExecutionError> {
        *self.revision.lock().unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
impl Wake {
    fn until(&self, client: &ExecutionClient, mut ready: impl FnMut() -> bool) -> Result<()> {
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            let before = *self.revision.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
            if ready() {
                return Ok(());
            }
            let remaining = until
                .checked_duration_since(Instant::now())
                .ok_or("Source terminal wake absent")?;
            let guard = self.revision.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let (guard, timeout) = self
                .changed
                .wait_timeout_while(guard, remaining, |at| *at == before)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            drop(guard);
            if timeout.timed_out() {
                return Err("Source terminal wake absent".into());
            }
        }
    }
}
fn exercise_source_stop(
    shutdown_first: bool,
    timed_out: bool,
    admission_fails: bool,
) -> Result<()> {
    let clock = Arc::new(HeldClock::default());
    let wake = Arc::new(Wake::default());
    let execution = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = execution.client();
    let scopes = ScopeSequence::new();
    let native =
        client.create_native_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, None)?;
    let cleanup =
        client.create_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, None)?;
    let source_scope =
        client.create_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, Some(&native))?;
    let jobs = JobSequence::new();
    let mut authority = CoordinatorAuthority::new(
        sugarsubstitute_contracts::coordinator_entry::EntrySetupBudget::new(Moment::from_nanos(0))?,
        None,
    )?;
    let mut held = backend_tests::unadmitted_policy()?;
    let backend = held.backend.as_mut();
    let run = RunKey {
        owner: backend_tests::key()?,
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    };
    let connection = ConnectionId::new(1)?;
    let registration = ClientRegistration {
        run,
        connection,
        launch: sugarsubstitute_contracts::startup::LaunchId::new(1)?,
        profile: sugarsubstitute_contracts::configuration::ProfileId::new("SourceStop_1".into())?,
    };
    backend.policy.enroll_run(RunEnrollment {
        run,
        launch: registration.launch,
        profile: registration.profile.clone(),
    })?;
    backend.register_run(registration)?;
    let target = BackendTarget::Attached(LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?);
    let transition = backend.policy.request(
        ClientBinding { run, connection },
        ResourceRequest {
            run,
            connection,
            id: RequestId::new(1)?,
            command: ResourceCommand::Acquire(target.clone()),
        },
    )?;
    let Some(ResourceEffect::Prepare { resource, .. }) = transition.effects.first() else {
        return Err("Original source generation absent".into());
    };
    let key = *resource;
    let mut record = cutoff_tests::creator(
        key,
        target,
        ConnectionBudget::new(ConnectionBudgetKind::Attached, Moment::from_nanos(0))?,
        source_scope,
    )?;
    record.source = boundary_fixture::owner()?;
    let hold = Hold::new(clock.clone());
    record.source.submit_inspection(
        &client,
        &record.scope,
        JobSpec {
            id: jobs.allocate()?,
            operation: Operation::Readiness,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(1000)),
            reservation_bytes: FRONTEND_SOURCE_BYTES,
            replacement: None,
        },
    )?;
    record.source_job = true;
    record.source_closed = false;
    if admission_fails {
        record.output = Some(client.reserve_windows_output(
            &native,
            JobSpec {
                id: jobs.allocate()?,
                operation: Operation::NativeLifecycle,
                class: JobClass::NativeObservation,
                reservation_bytes: MIN_WINDOWS_OUTPUT_BYTES,
                deadline: None,
                replacement: None,
            },
            sugarsubstitute_execution::windows_output::OutputIdentity::Managed(key),
        )?);
    }
    backend.records[0] = Some(record);
    let mut resources = BackendResources {
        client: &client,
        native: &native,
        cleanup: &cleanup,
        jobs: &jobs,
        scopes: &scopes,
        authority: &mut authority,
        capture: None,
    };
    let original = Box::new(std::io::Error::from_raw_os_error(122));
    let pointer = std::ptr::from_ref(original.as_ref());
    let mut original = Some(original);
    if timed_out {
        clock.now.store(1000, Ordering::Release);
        drop(hold);
        wake.until(&client, || {
            backend.records[0]
                .as_ref()
                .is_some_and(|record| record.scope.snapshot().terminal_unobserved == 1)
        })?;
    } else {
        if !shutdown_first {
            backend.record(key)?.scope.cancel();
        }
        if shutdown_first {
            backend.begin_delivery()?;
            backend.failed(
                key,
                original.take().ok_or("Original campaign cause absent")?,
                &mut resources,
            )?;
        }
        drop(hold);
        wake.until(&client, || {
            backend.records[0]
                .as_ref()
                .is_some_and(|record| record.scope.snapshot().terminal_unobserved == 1)
        })?;
    }
    if shutdown_first && timed_out {
        backend.begin_delivery()?;
        backend.failed(
            key,
            original.take().ok_or("Original campaign cause absent")?,
            &mut resources,
        )?;
    }
    let budget = backend.record(key)?.budget;
    let shutdown = backend.record(key)?.shutdown;
    let record = backend.records[0].as_mut().ok_or("Original source record absent")?;
    let result = consume_source(record, Moment::from_nanos(0));
    assert!(!record.source_job);
    assert!(!record.source_closed);
    assert!(record.source_lease.is_none());
    assert_eq!(record.scope.snapshot().reserved_bytes, FRONTEND_SOURCE_BYTES);
    if shutdown_first {
        result?;
        assert!(backend.cleanup_failure(key).is_none());
        assert_eq!(
            backend
                .failure(key)
                .and_then(Error::source)
                .and_then(|cause| cause.downcast_ref::<std::io::Error>())
                .map(std::ptr::from_ref),
            Some(pointer)
        );
    } else {
        let cause = result.err().ok_or("Source stop before cleanup was ignored")?;
        assert_eq!(
            cause
                .downcast_ref::<FrontendSourceError>()
                .and_then(FrontendSourceError::terminal_kind),
            Some(TerminalKind::Cancelled)
        );
        backend.begin_delivery()?;
        backend.failed(key, cause, &mut resources)?;
        assert_eq!(
            FailureObservation::retained(
                backend.failure(key).ok_or("Original source failure absent")?
            )
            .terminal,
            Some(TerminalKind::Cancelled)
        );
    }
    assert_eq!(backend.record(key)?.budget, budget);
    if shutdown_first {
        assert_eq!(backend.record(key)?.shutdown, shutdown);
    }
    assert!(!backend.attempt_settled(key));
    backend.cleanup(0, &mut resources)?;
    assert!(!backend.record(key)?.source_closed);
    assert_eq!(
        backend
            .record(key)?
            .source_close
            .as_ref()
            .ok_or("Source cleanup absent")?
            .spec()
            .reservation_bytes,
        FRONTEND_SOURCE_CLOSE_BYTES,
    );
    wake.until(&client, || {
        backend.records[0].as_ref().and_then(|record| record.source_close.as_ref()).is_some_and(
            |job| {
                matches!(
                    job.status().phase,
                    sugarsubstitute_contracts::execution::JobPhase::Terminal(_)
                )
            },
        )
    })?;
    if admission_fails {
        wake.until(&client, || {
            backend.records[0].as_ref().and_then(|record| record.output.as_ref()).is_some_and(
                |output| {
                    matches!(
                        output.completion.status().phase,
                        sugarsubstitute_contracts::execution::JobPhase::Terminal(_)
                    )
                },
            )
        })?;
        assert!(!backend.record(key)?.output_observed);
        assert!(backend.record(key)?.source_close.is_some());
        let stopped =
            client.create_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, None)?;
        stopped.cancel();
        let mut input = Some(());
        let admission = client
            .submit_blocking_owned_input::<(), std::io::Error, _, _>(
                &stopped,
                JobSpec {
                    id: jobs.allocate()?,
                    operation: Operation::Readiness,
                    class: JobClass::InfrastructureLifecycle,
                    deadline: Some(Moment::from_nanos(2000)),
                    reservation_bytes: 4096,
                    replacement: None,
                },
                &mut input,
                |(), _| Ok(()),
            )
            .err()
            .ok_or("Stopped scope admitted fresh work")?;
        assert!(input.is_some());
        let cause: Box<dyn Error + Send + Sync> = Box::new(admission);
        let first = std::ptr::from_ref(cause.as_ref());
        let batch = client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        let returned = backend
            .consume_existing(0, &batch, &resources, Err(cause))
            .err()
            .ok_or("Fresh admission failure was hidden")?;
        assert!(std::ptr::eq(std::ptr::from_ref(returned.as_ref()), first));
        assert!(returned.downcast_ref::<sugarsubstitute_execution::OwnedInputError>().is_some());
        assert_eq!(stopped.snapshot().reserved_bytes, 0);
        assert!(backend.record(key)?.output_observed);
        assert!(matches!(
            backend.record(key)?.output_terminal.as_ref().map(|done| &done.outcome),
            Some(JobOutcome::Cancelled)
        ));
        assert!(
            backend
                .record(key)?
                .output
                .as_ref()
                .is_some_and(|output| output.control.snapshot().retired)
        );
        assert!(native.snapshot().reserved_bytes >= MIN_WINDOWS_OUTPUT_BYTES);
        assert_eq!(backend.record(key)?.shutdown, shutdown);
        assert_eq!(backend.record(key)?.budget, budget);
        assert!(!backend.attempt_settled(key));
        assert_eq!(
            backend
                .failure(key)
                .and_then(Error::source)
                .and_then(|cause| cause.downcast_ref::<std::io::Error>())
                .map(std::ptr::from_ref),
            Some(pointer),
        );
    } else {
        consume_source(
            backend.records[0].as_mut().ok_or("Original source record absent")?,
            Moment::from_nanos(0),
        )?;
    }
    assert!(backend.record(key)?.source_closed);
    assert!(backend.record(key)?.source_terminal.is_none());
    assert!(backend.cleanup_failure(key).is_none());
    backend.cleanup(0, &mut resources)?;
    assert!(backend.record(key)?.settled);
    // Delivery does not join the executor's final context aliases. Keep the actual
    // settled Backend alive while proving its obsolete close credit is released.
    drop(execution);
    assert_eq!(cleanup.snapshot().reserved_bytes, 0);
    assert!(backend.record(key)?.source_closed);
    assert!(backend.record(key)?.source_terminal.is_none());
    if admission_fails {
        assert!(backend.record(key)?.output.is_none());
        assert!(backend.record(key)?.output_terminal.is_none());
        assert_eq!(native.snapshot().reserved_bytes, 0);
    }
    if shutdown_first {
        assert_eq!(
            backend
                .failure(key)
                .and_then(Error::source)
                .and_then(|cause| cause.downcast_ref::<std::io::Error>())
                .map(std::ptr::from_ref),
            Some(pointer),
        );
    }
    let Backend { scope: retained_scope, .. } =
        backend.records[0].take().ok_or("Original closed source record absent")?;
    drop(held);
    assert_eq!(retained_scope.snapshot().reserved_bytes, 0);
    Ok(())
}

/// The first failed campaign remains authoritative while its source stops and really closes.
#[test]
fn backend_source_terminal_cancel_and_timeout_during_cleanup_keep_original_failure() -> Result<()> {
    for timed_out in [false, true] {
        exercise_source_stop(true, timed_out, false)?;
    }
    Ok(())
}

/// A stopped source without an existing cleanup remains an actionable preparation failure.
#[test]
fn backend_source_terminal_cancel_before_cleanup_remains_failure_until_explicit_close() -> Result<()>
{
    exercise_source_stop(false, false, false)
}

/// A genuine admission rejection cannot prevent queued source/output closes from being adopted.
#[test]
fn custody_admission_failure_still_adopts_real_source_and_output_cleanup_terminals() -> Result<()> {
    exercise_source_stop(true, false, true)
}

#[path = "source_terminal_tests/output_retirement.rs"]
mod output_retirement;
