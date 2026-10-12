// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Supply causal clocks, completion notification and the controlled native boundary.
use super::*;
use std::sync::{
    Condvar,
    atomic::{AtomicU64, AtomicUsize},
};
#[derive(Default)]
pub(in crate::claims::tests) struct ClockPort(AtomicU64);
impl ClockPort {
    pub(in crate::claims::tests) fn advance(
        &self,
        elapsed: Duration,
    ) -> Result<(), Box<dyn Error>> {
        let now = Moment::from_nanos(self.0.load(Ordering::Acquire));
        self.0.store(now.checked_add(elapsed)?.as_nanos(), Ordering::Release);
        Ok(())
    }
}
impl Clock for ClockPort {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::Acquire))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
pub(in crate::claims::tests) struct Wake {
    pub(super) revision: Mutex<u64>,
    pub(super) changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *lock(&self.revision) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
#[derive(Default)]
pub(in crate::claims::tests) struct Fake {
    pub(in crate::claims::tests) close_deadlines: Mutex<Vec<Moment>>,
    pub(in crate::claims::tests) resolutions: AtomicUsize,
    pub(in crate::claims::tests) initializations: AtomicUsize,
    pub(in crate::claims::tests) gate: Mutex<Option<(Stage, Arc<Barrier>)>>,
    pub(in crate::claims::tests) failure: Mutex<Option<Stage>>,
    pub(in crate::claims::tests) panic_at: Mutex<Option<Stage>>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::claims::tests) enum Stage {
    Initialize,
    Resolve,
    Acquire,
    Release,
    Close,
}
#[derive(Default)]
pub(in crate::claims::tests) struct Barrier {
    state: Mutex<(usize, bool)>,
    pub(super) changed: Condvar,
}
impl Barrier {
    pub(in crate::claims::tests) fn enter(&self) -> Result<(), ClaimError> {
        let mut state = lock(&self.state);
        state.0 += 1;
        self.changed.notify_all();
        let (_state, timeout) = self
            .changed
            .wait_timeout_while(state, Duration::from_secs(10), |state| !state.1)
            .map_err(|_| ClaimError::invalid("fixture_barrier_poison"))?;
        if timeout.timed_out() {
            return Err(ClaimError::invalid("fixture_barrier_deadline"));
        }
        Ok(())
    }
    pub(in crate::claims::tests) fn wait(&self, count: usize) -> Result<(), Box<dyn Error>> {
        let state = lock(&self.state);
        let (_state, timeout) = self
            .changed
            .wait_timeout_while(state, Duration::from_secs(10), |state| state.0 < count)
            .map_err(|_| "Fixture barrier poisoned")?;
        if timeout.timed_out() {
            return Err("Fixture barrier deadline expired".into());
        }
        Ok(())
    }
    pub(in crate::claims::tests) fn release(&self) {
        lock(&self.state).1 = true;
        self.changed.notify_all();
    }
}
impl Fake {
    fn pause(&self, stage: Stage) -> Result<(), ClaimError> {
        let gate = lock(&self.gate)
            .as_ref()
            .filter(|(value, _)| *value == stage)
            .map(|(_, gate)| gate.clone());
        if let Some(gate) = gate {
            gate.enter()?;
        }
        let should_panic = {
            let mut planned = lock(&self.panic_at);
            if planned.as_ref() == Some(&stage) {
                *planned = None;
                true
            } else {
                false
            }
        };
        if should_panic {
            std::panic::resume_unwind(Box::new("native-boundary-private-cause"));
        }
        let mut failure = lock(&self.failure);
        if failure.as_ref() == Some(&stage) {
            *failure = None;
            return Err(ClaimError::new(
                "fixture_native_failure",
                std::io::Error::other("native-boundary-private-cause"),
            ));
        }
        Ok(())
    }
}
impl NativePort for Fake {
    fn initialize(
        &self,
        root: &mut Root,
        _: &std::path::Path,
        _: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<(), ClaimError> {
        root.key = Some(owner()?);
        root.sid = vec![1];
        root.claimed = true;
        self.initializations.fetch_add(1, Ordering::AcqRel);
        self.pause(Stage::Initialize)
    }
    fn resolve(&self, resolution: &mut Resolution, sid: &[u8]) -> Result<(), ClaimError> {
        self.resolutions.fetch_add(1, Ordering::AcqRel);
        resolution.ready = true;
        resolution.account_sid = sid.to_vec();
        resolution.digest = [2; 32];
        self.pause(Stage::Resolve)
    }
    fn acquire(
        &self,
        backend: &mut Backend,
        _: &std::path::Path,
        _: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<(), ClaimError> {
        backend.claimed = true;
        self.pause(Stage::Acquire)
    }
    fn publish(
        &self,
        _: &mut Backend,
        _: native_claims::RecordState,
        _: u64,
        _: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<(), ClaimError> {
        Ok(())
    }
    fn release(
        &self,
        backend: &mut Backend,
        _: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<(), ClaimError> {
        backend.proof.close()?;
        backend.released = true;
        self.pause(Stage::Release)
    }
    fn close(
        &self,
        root: &mut Root,
        context: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<(), ClaimError> {
        lock(&self.close_deadlines)
            .push(context.deadline().ok_or_else(|| ClaimError::invalid("fixture_close_deadline"))?);
        self.pause(Stage::Close)?;
        root.pool = None;
        Ok(())
    }
}
