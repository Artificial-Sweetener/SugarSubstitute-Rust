// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fake only protocol/family facts; cancellation and transport completion use the real owner.
//! These fixtures authenticate no peer and claim no integrated helper or ACK qualification.
use super::*;
use std::{
    num::NonZeroUsize,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use sugarsubstitute_contracts::execution::{
    Clock, CompletionWake, ExecutionError, ExecutionPort, Scope, ScopeId, ScopeSpec, WaitFuture,
};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner, windows::NativeObservationError};
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Event {
    Observe,
    FinishPending,
    Finished,
    Stop,
    Force,
    ExpiredCleanup,
    PipeClosed,
    Terminal,
    Retired,
}
#[derive(Default)]
pub(super) struct TestClock(AtomicU64);
impl TestClock {
    pub(super) fn set(&self, now: Moment) {
        self.0.store(now.as_nanos(), Ordering::Release);
    }
}
impl Clock for TestClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::Acquire))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
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
    fn revision(&self) -> u64 {
        *self.revision.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    fn wait(&self, prior: u64) -> TestResult {
        let guard = self.revision.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let (_guard, timeout) = self
            .changed
            .wait_timeout_while(guard, Duration::from_secs(10), |value| *value == prior)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if timeout.timed_out() {
            return Err("Actual transport retirement did not wake".into());
        }
        Ok(())
    }
}
pub(super) struct Fixture {
    pub(super) clock: Arc<TestClock>,
    pub(super) client: ExecutionClient,
    pub(super) port: Port,
    pub(super) scope: Scope,
    wake: Arc<Wake>,
    _execution: Option<ExecutionOwner>,
}
impl Fixture {
    /// Observe the matching real job notification without consuming its original outcome.
    pub(super) fn terminal(
        &self,
        id: sugarsubstitute_contracts::execution::JobId,
    ) -> std::result::Result<
        sugarsubstitute_contracts::execution::CompletionBatch,
        Box<dyn Error + Send + Sync>,
    > {
        loop {
            let prior = self.wake.revision();
            let batch =
                self.client.drain_completions(NonZeroUsize::new(32).ok_or("Drain bound")?)?;
            if batch.notifications.iter().any(|notice| notice.job_id == id) {
                return Ok(batch);
            }
            self.wake.wait(prior)?;
        }
    }
    pub(super) fn new(tag: Option<u8>) -> std::result::Result<Self, Box<dyn Error + Send + Sync>> {
        let clock = Arc::new(TestClock::default());
        let wake = Arc::new(Wake::default());
        let execution = ExecutionOwner::new(clock.clone(), wake.clone())?;
        let client = execution.client();
        let scope =
            client.create_native_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let parent = if let Some(tag) = tag {
            use sugarsubstitute_contracts::{
                backend_custody::BackendCustodySession,
                execution::JobId,
                lifecycle::{CoordinatorEpoch, CoordinatorKey, NamespaceId},
            };
            // Each caller has a distinct endpoint; matching data does not authenticate a peer.
            let key = BackendCustodyKey::new(
                CoordinatorKey {
                    namespace: NamespaceId::new([tag; 32]),
                    epoch: CoordinatorEpoch::new([tag; 16])?,
                },
                BackendCustodySession::new(1)?,
            );
            let mut session = Some(client.reserve_backend_custody_pipe_server(
                &scope,
                JobSpec {
                    id: JobId::new(1)?,
                    operation: Operation::NativeLifecycle,
                    class: JobClass::NativeObservation,
                    reservation_bytes:
                        sugarsubstitute_execution::windows_lifecycle_pipe::MIN_LIFECYCLE_PIPE_BYTES,
                    deadline: None,
                    replacement: None,
                },
                key,
                Moment::from_nanos(30_000_000_000),
            )?);
            {
                let parent = BackendCustodyParent::new(&mut session)?;
                assert!(parent.validate_current_admission().is_err());
                // Observe actual native creation through the original Parent, not timing.
                loop {
                    let prior = wake.revision();
                    client.drain_completions(NonZeroUsize::MIN)?;
                    parent.consume(NonZeroUsize::MIN)?;
                    if parent.prepared() {
                        break;
                    }
                    wake.wait(prior)?;
                }
                Some(parent)
            }
        } else {
            None
        };
        let port = Port {
            parent,
            client: client.clone(),
            clock: clock.clone(),
            finished: false,
            finish_status: BackendCustodyFinishStatus::Unissued,
            failure: None,
            cleanup_failure: false,
            family_retired: false,
            original: None,
            events: Vec::new(),
            terminal_taken: false,
        };
        Ok(Self { clock, client, port, scope, wake, _execution: Some(execution) })
    }
    pub(super) fn settle(
        &mut self,
        owner: &mut Retirement,
        budget: ShutdownBudget,
        ready: bool,
        key: BackendCustodyKey,
    ) -> TestResult {
        loop {
            let prior = self.wake.revision();
            owner.advance(budget, ready, true, self.client.now()?, key, &mut self.port)?;
            if owner.settled {
                return Ok(());
            }
            self.wake.wait(prior)?;
        }
    }
    pub(super) fn close_fixture(&mut self) -> TestResult {
        if !self.port.terminal_taken {
            if let Some(parent) = &self.port.parent {
                parent.close()?;
            }
            loop {
                let prior = self.wake.revision();
                if self.port.take_terminal()?.is_some() {
                    break;
                }
                self.wake.wait(prior)?;
            }
        }
        drop(self.port.parent.take());
        drop(self._execution.take());
        assert_eq!(self.scope.snapshot().reserved_bytes, 0);
        Ok(())
    }
}
pub(super) struct Port {
    parent: Option<BackendCustodyParent>,
    client: ExecutionClient,
    clock: Arc<TestClock>,
    pub(super) finished: bool,
    pub(super) finish_status: BackendCustodyFinishStatus,
    pub(super) failure: Option<NativeObservationError>,
    pub(super) cleanup_failure: bool,
    pub(super) family_retired: bool,
    pub(super) original: Option<ShutdownBudget>,
    pub(super) events: Vec<Event>,
    terminal_taken: bool,
}
impl Port {
    pub(super) fn observe_terminal(&mut self, owner: &mut Retirement) -> Result<()> {
        let result = owner.observe_terminal(self.parent()?);
        self.terminal_taken |= owner.terminal.is_some();
        result
    }
    pub(super) fn parent(&self) -> Result<&BackendCustodyParent> {
        self.parent.as_ref().ok_or_else(|| "Actual original pipe absent".into())
    }
    pub(super) fn key(
        &self,
    ) -> std::result::Result<BackendCustodyKey, Box<dyn Error + Send + Sync>> {
        Ok(self.parent.as_ref().ok_or("Actual original parent absent")?.key())
    }
}
impl Boundary for Port {
    fn observe(&mut self) -> Result<()> {
        self.events.push(Event::Observe);
        if let Some(cause) = &self.failure {
            return Err(Box::new(cause.clone()));
        }
        Ok(())
    }
    fn finish(&mut self, _: Moment) -> Result<bool> {
        if let Some(cause) = &self.failure {
            return Err(Box::new(cause.clone()));
        }
        self.events.push(if self.finished { Event::Finished } else { Event::FinishPending });
        Ok(self.finished)
    }
    fn finish_status(&self) -> BackendCustodyFinishStatus {
        self.finish_status
    }
    fn retire_family(&mut self, budget: ShutdownBudget) -> Result<bool> {
        if let Some(original) = self.original {
            assert_eq!(original, budget);
        } else {
            self.original = Some(budget);
            self.events.push(Event::Stop);
        }
        if self.cleanup_failure {
            return Err(Box::new(NativeObservationError::new(
                "original_cleanup",
                std::io::Error::from_raw_os_error(6),
            )));
        }
        if self.clock.now() >= budget.settle_until {
            self.events.push(Event::ExpiredCleanup);
            return Err(Box::new(NativeObservationError::new(
                "original_cleanup_deadline",
                std::io::Error::other("Original finite effect expired"),
            )));
        }
        if !self.family_retired && self.clock.now() >= budget.graceful_until {
            self.events.push(Event::Force);
        }
        Ok(self.family_retired)
    }
    fn close_pipe(&mut self) -> Result<()> {
        self.events.push(Event::PipeClosed);
        self.parent.as_ref().ok_or("Actual original pipe absent")?.close()?;
        Ok(())
    }
    fn take_terminal(&mut self) -> Result<Option<Terminal>> {
        self.client.drain_completions(NonZeroUsize::MIN)?;
        let result =
            self.parent.as_ref().map(BackendCustodyParent::take_terminal).transpose()?.flatten();
        if result.is_some() {
            self.terminal_taken = true;
            self.events.push(Event::Terminal);
        }
        Ok(result)
    }
    fn parent_retired(&mut self) {
        self.events.push(Event::Retired);
    }
}
