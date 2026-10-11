// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Qualify nonblocking public cancellation callbacks on owned workers.

use crate::{runtime_test_support::*, scope::lock};
use std::{
    sync::{Arc, Mutex},
    task::{Context, Wake, Waker},
};
use sugarsubstitute_contracts::execution::*;

struct ControlledWake {
    entered: Arc<Gate>,
    release: Arc<Gate>,
    thread: Arc<Mutex<Option<std::thread::ThreadId>>>,
    panics: bool,
}
impl Wake for ControlledWake {
    fn wake(self: Arc<Self>) {
        *lock(&self.thread) = Some(std::thread::current().id());
        self.entered.open();
        assert!(self.release.wait().is_ok());
        if self.panics {
            std::panic::resume_unwind(Box::new("Cancellation wake failed"));
        }
    }
}

struct ObservedWake(Arc<Gate>);
impl Wake for ObservedWake {
    fn wake(self: Arc<Self>) {
        self.0.open();
    }
}

#[test]
fn control_requests_return_before_blocking_cancel_waker_and_observe_worker_panic() -> TestResult {
    let mut rig = Rig::new()?;
    let mut job = rig.client.submit_async(
        &rig.scope,
        spec(1, JobClass::IoOrdinary, 0)?,
        |context| async { Ok::<_, ()>(context) },
    )?;
    let completion = rig.completion(&mut job)?;
    let context = match completion.outcome {
        JobOutcome::Success(context) => context,
        _ => return Err(std::io::Error::other("Expected retained context").into()),
    };
    let sibling =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let mut sibling_job =
        rig.client.submit_async(&sibling, spec(2, JobClass::IoOrdinary, 0)?, |context| async {
            Ok::<_, ()>(context)
        })?;
    let sibling_completion = rig.completion(&mut sibling_job)?;
    let sibling_context = match sibling_completion.outcome {
        JobOutcome::Success(context) => context,
        _ => return Err(std::io::Error::other("Expected sibling context").into()),
    };
    let delivered = Arc::new(Gate::default());
    let sibling_waker = Waker::from(Arc::new(ObservedWake(delivered.clone())));
    let mut sibling_cancelled = sibling_context.cancelled();
    assert!(sibling_cancelled.as_mut().poll(&mut Context::from_waker(&sibling_waker)).is_pending());
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let thread = Arc::new(Mutex::new(None));
    let waker = Waker::from(Arc::new(ControlledWake {
        entered: entered.clone(),
        release: release.clone(),
        thread: thread.clone(),
        panics: true,
    }));
    let mut cancelled = context.cancelled();
    assert!(cancelled.as_mut().poll(&mut Context::from_waker(&waker)).is_pending());
    let same_target = Arc::new(Gate::default());
    let same_waker = Waker::from(Arc::new(ObservedWake(same_target.clone())));
    let mut same_cancelled = context.cancelled();
    assert!(same_cancelled.as_mut().poll(&mut Context::from_waker(&same_waker)).is_pending());
    job.cancel();
    assert!(!context.cancellation_requested());
    rig.scope.cancel();
    entered.wait()?;
    assert!(lock(&thread).is_some_and(|id| id != std::thread::current().id()));
    sibling.cancel();
    drop(job);
    rig.owner.request_shutdown(Moment::from_nanos(1));
    assert!(!rig.owner.poll_shutdown()?.settled);
    release.open();
    delivered.wait()?;
    same_target.wait()?;
    drop(rig.owner);
    let failure = rig
        .client
        .drain_completions(std::num::NonZeroUsize::MIN)
        .err()
        .ok_or_else(|| std::io::Error::other("Notification failure was lost"))?;
    assert!(failure.to_string().contains("Cancellation wake failed"));
    assert!(cancelled.as_mut().poll(&mut Context::from_waker(&waker)).is_ready());
    Ok(())
}

#[test]
fn shutdown_retains_untransferred_terminal_then_accepts_late_abandonment() -> TestResult {
    let mut rig = Rig::new()?;
    let stamp = rig.wake.count();
    let job =
        rig.client.submit_async(&rig.scope, spec(1, JobClass::IoOrdinary, 64)?, |_| async {
            Ok::<_, ()>(17)
        })?;
    rig.wake.wait_after(stamp)?;
    rig.owner.request_shutdown(Moment::from_nanos(1));
    let progress = rig.owner.poll_shutdown()?;
    assert!(!progress.settled);
    assert_eq!(progress.unresolved_job_ids, [JobId::new(1)?]);
    assert!(rig.owner.finish_shutdown().is_err());
    drop(job);
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    assert_eq!(rig.scope.snapshot().terminal_unobserved, 0);
    Ok(())
}

struct InspectClock {
    client: Mutex<Option<crate::runtime::ExecutionClient>>,
}
impl Clock for InspectClock {
    fn now(&self) -> Moment {
        if let Some(client) = &*lock(&self.client) {
            let core = client.core_for_test();
            drop(lock(&core.state));
        }
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}

#[test]
fn injected_clock_can_inspect_owned_state_during_admission_and_shutdown() -> TestResult {
    let clock = Arc::new(InspectClock { client: Mutex::new(None) });
    let wake = Arc::new(crate::runtime_test_support::Wake::default());
    let mut owner = crate::runtime::ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    *lock(&clock.client) = Some(client.clone());
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let stamp = wake.count();
    let mut job = client
        .submit_async(&scope, spec(1, JobClass::IoOrdinary, 0)?, |_| async { Ok::<_, ()>(()) })?;
    wake.wait_after(stamp)?;
    assert!(job.try_take_completion().is_some());
    owner.request_shutdown(Moment::from_nanos(1));
    let _progress = owner.poll_shutdown()?;
    drop(owner);
    *lock(&clock.client) = None;
    Ok(())
}

struct InspectScope {
    client: crate::runtime::ExecutionClient,
    id: ScopeId,
}
impl ScopeControl for InspectScope {
    fn owner_id(&self) -> ExecutionId {
        let core = self.client.core_for_test();
        drop(lock(&core.state));
        core.id
    }
    fn cancel(&self) {}
    fn snapshot(&self) -> ScopeSnapshot {
        self.client.core_for_test().scope_snapshot(self.id)
    }
    fn abandon(&self) {}
}

#[test]
fn supplied_scope_identity_can_inspect_state_before_admission_locks() -> TestResult {
    let rig = Rig::new()?;
    let supplied = Scope::new(
        rig.scope.spec(),
        Box::new(InspectScope { client: rig.client.clone(), id: rig.scope.id() }),
    );
    let child = rig
        .client
        .create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, Some(&supplied))?;
    let mut job =
        rig.client.submit_async(&supplied, spec(1, JobClass::IoOrdinary, 0)?, |_| async {
            Ok::<_, ()>(())
        })?;
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::Success(())));
    assert_eq!(child.snapshot().id, ScopeId::new(2)?);
    Ok(())
}

struct DestructorWake(Arc<Gate>);
impl Wake for DestructorWake {
    fn wake(self: Arc<Self>) {
        self.0.open();
    }
}
impl Drop for DestructorWake {
    fn drop(&mut self) {
        std::panic::resume_unwind(Box::new("Cancellation waker destructor failed"));
    }
}

#[test]
fn last_registered_waker_destructor_is_supervised_and_sibling_delivery_continues() -> TestResult {
    let rig = Rig::new()?;
    let core = rig.client.core_for_test();
    let cancellation = lock(&core.state)
        .scopes
        .get(&rig.scope.id())
        .map(|scope| scope.cancel.clone())
        .ok_or_else(|| std::io::Error::other("Missing actual scope cancellation"))?;
    let first_delivered = Arc::new(Gate::default());
    let first_waker = Waker::from(Arc::new(DestructorWake(first_delivered.clone())));
    let mut first = cancellation.cancelled();
    assert!(first.as_mut().poll(&mut Context::from_waker(&first_waker)).is_pending());
    drop(first_waker);
    let sibling_delivered = Arc::new(Gate::default());
    let sibling_waker = Waker::from(Arc::new(ObservedWake(sibling_delivered.clone())));
    let mut sibling = cancellation.cancelled();
    assert!(sibling.as_mut().poll(&mut Context::from_waker(&sibling_waker)).is_pending());
    rig.scope.cancel();
    first_delivered.wait()?;
    sibling_delivered.wait()?;
    drop(rig.owner);
    let error = rig
        .client
        .drain_completions(std::num::NonZeroUsize::MIN)
        .err()
        .ok_or_else(|| std::io::Error::other("Waker destructor failure was lost"))?;
    assert!(error.to_string().contains("Cancellation waker destructor failed"));
    Ok(())
}
