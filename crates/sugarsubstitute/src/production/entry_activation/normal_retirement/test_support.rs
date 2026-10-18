// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Run independent production reservations and native output workers over an external pipe fake.
use super::*;
use std::{
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_contracts::transcript::OutputStream;
use sugarsubstitute_execution::windows_output::{
    MIN_WINDOWS_OUTPUT_BYTES, boundary_fixture::OutputFixture,
};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};
pub(super) const OUTPUT_BYTES: u64 = MIN_WINDOWS_OUTPUT_BYTES;
struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(1)
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
        let mut revision = self.revision.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        *revision += 1;
        self.changed.notify_all();
        Ok(())
    }
}
pub(super) struct Rig {
    owner: Option<ExecutionOwner>,
    client: ExecutionClient,
    wake: Arc<Wake>,
    pub(super) finite: Scope,
    pub(super) native: Scope,
    pub(super) external: OutputFixture,
}
impl Rig {
    pub(super) fn new() -> Result<Self> {
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::new(Arc::new(FixedClock), wake.clone())?;
        let client = owner.client();
        let finite =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let native =
            client.create_native_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
        Ok(Self {
            owner: Some(owner),
            client,
            wake,
            finite,
            native,
            external: OutputFixture::new(),
        })
    }
    fn spec(id: u64, bytes: u64) -> Result<JobSpec> {
        Ok(JobSpec {
            id: JobId::new(id)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(30_000_000_000)),
            reservation_bytes: bytes,
            replacement: None,
        })
    }
    pub(super) fn output(
        &self,
        run: RunKey,
    ) -> Result<super::super::desktop_output::DesktopOutput> {
        self.external.push(OutputStream::Stdout, b"complete native tail\r\n")?;
        self.external.end(OutputStream::Stdout);
        self.external.end(OutputStream::Stderr);
        let spec = JobSpec {
            class: JobClass::NativeObservation,
            deadline: None,
            ..Self::spec(1, OUTPUT_BYTES)?
        };
        super::super::desktop_output::DesktopOutput::boundary_fixture(
            run,
            &self.client,
            &self.native,
            spec,
            &self.external,
        )
    }
    pub(super) fn seed(&self, run: RunKey) -> Result<incident_seed::SeedPreparation> {
        incident_seed::SeedPreparation::boundary_fixture(
            &self.client,
            &self.finite,
            Self::spec(2, 32768)?,
            run,
        )
    }
    pub(super) fn attachment(&self) -> Result<OwnedJob<MemoryLease, std::io::Error>> {
        Ok(self.client.submit_blocking(&self.finite, Self::spec(3, 16384)?, |context| {
            Ok(context.memory().clone())
        })?)
    }
    pub(super) fn batch(&self) -> Result<CompletionBatch> {
        Ok(self.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?)
    }
    pub(super) fn until(&self, mut ready: impl FnMut() -> Result<bool>) -> Result<()> {
        let until = Instant::now().checked_add(Duration::from_secs(10)).ok_or("Guard overflow")?;
        loop {
            let previous =
                *self.wake.revision.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if ready()? {
                return Ok(());
            }
            let remaining = until
                .checked_duration_since(Instant::now())
                .ok_or("Actual output retirement timed out")?;
            let revision =
                self.wake.revision.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let (revision, timeout) = self
                .wake
                .changed
                .wait_timeout_while(revision, remaining, |value| *value == previous)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            drop(revision);
            if timeout.timed_out() {
                return Err("Actual owner completion wake absent".into());
            }
        }
    }
    pub(super) fn join(&mut self) {
        drop(self.owner.take());
    }
}
