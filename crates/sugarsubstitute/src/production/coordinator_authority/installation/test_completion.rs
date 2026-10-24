// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe actual execution terminals while faking only the storage operation result.
use super::*;
use std::io;
use std::{
    sync::{Condvar, Mutex},
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::{
    AdmissionError, Clock, CompletionWake, ExecutionError, ScopeSpec, WaitFuture,
};
use sugarsubstitute_execution::ExecutionOwner;

#[derive(Default)]
struct FixedClock(std::sync::atomic::AtomicU64);
impl Clock for FixedClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(std::sync::atomic::Ordering::SeqCst))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
struct Wake(Mutex<u64>, Condvar);
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let mut revision = self.0.lock().map_err(|_| {
            ExecutionError::new(
                Operation::NativeLifecycle,
                Box::new(io::Error::other("Health observer poisoned")),
            )
        })?;
        *revision += 1;
        self.1.notify_all();
        Ok(())
    }
}

/// Observe capacity through actual admission and its release notification, not worker timing.
pub(super) struct Capacity<'a> {
    scope: &'a Scope,
    client: &'a ExecutionClient,
    wake: &'a Wake,
    bytes: u64,
}
impl Capacity<'_> {
    /// Borrow the same admitted execution owner for follow-on cleanup qualification.
    pub(super) fn client(&self) -> &ExecutionClient {
        self.client
    }

    /// Borrow the original live scope; tests never create an independent memory ledger.
    pub(super) fn scope(&self) -> &Scope {
        self.scope
    }

    /// Await actual adapter notifications until the supplied owner observes completion.
    pub(super) fn observe(
        &self,
        mut ready: impl FnMut() -> Result<bool, Box<dyn Error>>,
    ) -> Result<(), Box<dyn Error>> {
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            let revision = *self.wake.0.lock().map_err(|_| "Release observer poisoned")?;
            self.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
            if ready()? {
                return Ok(());
            }
            let remaining =
                until.checked_duration_since(Instant::now()).ok_or("Release completion absent")?;
            let (_guard, timed) = self
                .wake
                .1
                .wait_timeout_while(
                    self.wake.0.lock().map_err(|_| "Release observer poisoned")?,
                    remaining,
                    |value| *value == revision,
                )
                .map_err(|_| "Release observer poisoned")?;
            if timed.timed_out() {
                return Err("Release completion absent".into());
            }
        }
    }

    pub(super) fn reserved_bytes(&self) -> u64 {
        self.scope.snapshot().reserved_bytes
    }

    /// A whole-capacity job can start only after the publication's original reservation retires.
    pub(super) fn require_released(&self, id: JobId) -> Result<(), Box<dyn Error>> {
        let until = Instant::now() + Duration::from_secs(10);
        let spec = JobSpec {
            id,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: None,
            reservation_bytes: self.bytes,
            replacement: None,
        };
        let mut probe = None;
        loop {
            let revision = *self.wake.0.lock().map_err(|_| "Capacity observer poisoned")?;
            if probe.is_none() {
                match self.client.submit_blocking(self.scope, spec, |_| Ok::<_, io::Error>(())) {
                    Ok(job) => probe = Some(job),
                    Err(AdmissionError::MemorySaturated { .. }) => {}
                    Err(cause) => return Err(cause.into()),
                }
            }
            self.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
            if let Some(job) = &mut probe
                && let Some(completion) = job.try_take_completion()
            {
                return if matches!(completion.outcome, JobOutcome::Success(())) {
                    Ok(())
                } else {
                    Err("Capacity probe did not complete successfully".into())
                };
            }
            let remaining = until
                .checked_duration_since(Instant::now())
                .ok_or("Publication still retains execution capacity")?;
            let (_guard, timed) = self
                .wake
                .1
                .wait_timeout_while(
                    self.wake.0.lock().map_err(|_| "Capacity observer poisoned")?,
                    remaining,
                    |value| *value == revision,
                )
                .map_err(|_| "Capacity observer poisoned")?;
            if timed.timed_out() {
                return Err("Publication still retains execution capacity".into());
            }
        }
    }
}

/// Use the actual execution adapter and bounded condition notification, never timing sleeps.
pub(super) fn completed<T: Send + 'static>(
    result: io::Result<T>,
    expire: bool,
    check: impl FnOnce(
        OwnedJob<T, io::Error>,
        CompletionBatch,
        &Capacity<'_>,
    ) -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    let wake = Arc::new(Wake::default());
    let clock = Arc::new(FixedClock::default());
    let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let spec = JobSpec {
        id: JobId::new(1)?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(Moment::from_nanos(1_000_000_000)),
        reservation_bytes: 4096,
        replacement: None,
    };
    let job = client.submit_blocking(&scope, spec, move |_| {
        if expire {
            clock.0.store(2_000_000_000, std::sync::atomic::Ordering::SeqCst);
        }
        result
    })?;
    let until = Instant::now() + Duration::from_secs(10);
    let batch = loop {
        let revision = *wake.0.lock().map_err(|_| "Observer poisoned")?;
        let batch = client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        if batch.notifications.iter().any(|notice| notice.job_id == spec.id) {
            break batch;
        }
        let remaining =
            until.checked_duration_since(Instant::now()).ok_or("Health completion absent")?;
        let (_guard, timed) = wake
            .1
            .wait_timeout_while(
                wake.0.lock().map_err(|_| "Observer poisoned")?,
                remaining,
                |value| *value == revision,
            )
            .map_err(|_| "Observer poisoned")?;
        if timed.timed_out() {
            return Err("Health completion absent".into());
        }
    };
    check(
        job,
        batch,
        &Capacity {
            scope: &scope,
            client: &client,
            wake: &wake,
            bytes: owner.limits().memory_bytes,
        },
    )
}
