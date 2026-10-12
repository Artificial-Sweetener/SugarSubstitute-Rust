// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Synchronize actual admitted execution producers around the primitive handoff boundary.
use super::*;
use std::{
    error::Error,
    num::NonZeroUsize,
    sync::{Condvar, Mutex},
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::ExecutionOwner;
struct ClockPort;
impl Clock for ClockPort {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
struct GateState {
    entered: bool,
    attempting: bool,
    released: bool,
    active: bool,
    closed: bool,
    signals: usize,
}
#[derive(Default)]
struct Gate {
    state: Mutex<GateState>,
    changed: Condvar,
}
fn cause(message: &'static str) -> ExecutionError {
    ExecutionError::new(Operation::NativeLifecycle, Box::new(std::io::Error::other(message)))
}
impl Gate {
    fn await_entered(&self) -> Result<(), ExecutionError> {
        let state = self.state.lock().map_err(|_| cause("Gate poisoned"))?;
        drop(self.changed.wait_while(state, |s| !s.entered).map_err(|_| cause("Gate poisoned"))?);
        Ok(())
    }
    fn mark_attempt(&self) -> Result<(), ExecutionError> {
        self.state.lock().map_err(|_| cause("Gate poisoned"))?.attempting = true;
        self.changed.notify_all();
        Ok(())
    }
    fn release_after_attempt(&self) -> Result<(), ExecutionError> {
        let state = self.state.lock().map_err(|_| cause("Gate poisoned"))?;
        let mut state = self
            .changed
            .wait_while(state, |s| !s.entered || !s.attempting)
            .map_err(|_| cause("Gate poisoned"))?;
        state.released = true;
        self.changed.notify_all();
        Ok(())
    }
}
/// Release controlled external work on every test error before the execution owner joins.
struct ReleaseOnDrop(Arc<Gate>);
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        let mut state = match self.0.state.lock() {
            Ok(state) => state,
            Err(state) => state.into_inner(),
        };
        state.released = true;
        state.attempting = true;
        self.0.changed.notify_all();
    }
}
impl EventBoundary for Gate {
    fn signal(&self) -> Result<(), ExecutionError> {
        let mut state = self.state.lock().map_err(|_| cause("Gate poisoned"))?;
        if state.closed {
            return Err(cause("Signal reached checked-closed event"));
        }
        state.signals += 1;
        if !state.entered {
            state.entered = true;
            state.active = true;
            self.changed.notify_all();
            state = self
                .changed
                .wait_while(state, |s| !s.released)
                .map_err(|_| cause("Gate poisoned"))?;
            state.active = false;
        }
        Ok(())
    }
    fn reset(&self) -> Result<(), ExecutionError> {
        Ok(())
    }
    fn wait(&self, _: Option<Instant>, _: bool) -> Result<WaitOutcome, DesktopError> {
        Err(DesktopError::message("gate_wait", "This synchronized fixture does not wait natively"))
    }
    fn close(&self) -> Result<(), DesktopError> {
        let mut state =
            self.state.lock().map_err(|_| DesktopError::message("gate_close", "Gate poisoned"))?;
        if state.active {
            return Err(DesktopError::message("gate_close", "Closed during active native signal"));
        }
        state.closed = true;
        Ok(())
    }
}
#[derive(Default)]
struct Proxy {
    generation: Mutex<usize>,
    changed: Condvar,
}
impl Proxy {
    fn observed(&self) -> Result<usize, ExecutionError> {
        Ok(*self.generation.lock().map_err(|_| cause("Proxy poisoned"))?)
    }
    fn await_change(&self, observed: usize) -> Result<(), ExecutionError> {
        let generation = self.generation.lock().map_err(|_| cause("Proxy poisoned"))?;
        drop(
            self.changed
                .wait_while(generation, |g| *g == observed)
                .map_err(|_| cause("Proxy poisoned"))?,
        );
        Ok(())
    }
}
impl CompletionWake for Proxy {
    fn wake(&self) -> Result<(), ExecutionError> {
        let mut generation = self.generation.lock().map_err(|_| cause("Proxy poisoned"))?;
        *generation =
            generation.checked_add(1).ok_or_else(|| cause("Proxy generation exhausted"))?;
        self.changed.notify_all();
        Ok(())
    }
}
fn spec(id: u64) -> Result<JobSpec, IdentityError> {
    Ok(JobSpec {
        id: JobId::new(id)?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(Moment::from_nanos(1_000_000_000)),
        reservation_bytes: 4096,
        replacement: None,
    })
}
#[test]
fn actual_execution_producers_serialize_active_event_signal_with_handoff_and_checked_close()
-> Result<(), Box<dyn Error>> {
    let gate = Arc::new(Gate::default());
    let bootstrap = Bootstrap::from_event(gate.clone());
    let endpoint = bootstrap.wake();
    let mut owner = ExecutionOwner::new(Arc::new(ClockPort), endpoint.clone())?;
    let release_guard = ReleaseOnDrop(gate.clone());
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let producer = endpoint.clone();
    let mut first = client.submit_blocking(&scope, spec(1)?, move |_| producer.wake())?;
    let releaser = gate.clone();
    let mut second =
        client.submit_blocking(&scope, spec(2)?, move |_| releaser.release_after_attempt())?;
    gate.await_entered()?;
    gate.mark_attempt()?;
    let proxy = Arc::new(Proxy::default());
    bootstrap.handoff(proxy.clone())?;
    assert!(gate.state.lock().map_err(|_| "gate")?.closed);
    let observed = proxy.observed()?;
    endpoint.wake()?;
    assert_eq!(proxy.observed()?, observed + 1);
    let mut consumed = [false; 2];
    while !consumed.into_iter().all(|done| done) {
        let observed = proxy.observed()?;
        let _batch = client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        for (index, job) in [&mut first, &mut second].into_iter().enumerate() {
            if !consumed[index]
                && let Some(completion) = job.try_take_completion()
            {
                match completion.outcome {
                    JobOutcome::Success(()) => {}
                    other => return Err(format!("Unexpected execution terminal: {other:?}").into()),
                }
                consumed[index] = true;
            }
        }
        if !consumed.into_iter().all(|done| done) {
            proxy.await_change(observed)?;
        }
    }
    client.now()?;
    owner.request_shutdown(Moment::from_nanos(1_000_000_000));
    drop(first);
    drop(second);
    drop(scope);
    drop(client);
    // The sole execution owner's destructor actually cancels and joins its finite
    // workers here, off UI. Dropping job/future tokens is not used as a stop proof.
    drop(release_guard);
    drop(owner);
    bootstrap.disconnect()?;
    assert!(endpoint.wake().is_err());
    let state = gate.state.lock().map_err(|_| "gate")?;
    assert!(state.entered && state.released && state.closed && !state.active);
    Ok(())
}
