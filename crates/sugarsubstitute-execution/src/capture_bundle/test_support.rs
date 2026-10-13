// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Hold real admitted finite factories outside the actual owner for capture proof tests.

use crate::{
    runtime_test_support::{Gate, Rig, TestResult, spec},
    scope::lock,
};
use std::{
    io,
    sync::{Arc, Mutex},
};
use sugarsubstitute_contracts::execution::*;

pub(super) struct HeldCapture {
    pub(super) context: JobContext,
    pub(super) job: OwnedJob<(), io::Error>,
    release: Arc<Gate>,
}
impl HeldCapture {
    pub(super) fn new(rig: &Rig, id: u64, bytes: u64, deadline: u64) -> TestResult<Self> {
        Self::in_scope(rig, &rig.scope, id, bytes, deadline)
    }
    /// Submit to the real supplied scope, retaining the identical original callback gates.
    pub(super) fn in_scope(
        rig: &Rig,
        scope: &Scope,
        id: u64,
        bytes: u64,
        deadline: u64,
    ) -> TestResult<Self> {
        let entered = Arc::new(Gate::default());
        let release = Arc::new(Gate::default());
        let retained = Arc::new(Mutex::new(None));
        let worker_entered = entered.clone();
        let worker_release = release.clone();
        let worker_retained = retained.clone();
        let mut request = spec(id, JobClass::InfrastructureLifecycle, bytes)?;
        request.operation = Operation::NativeLifecycle;
        request.deadline = Some(Moment::from_nanos(deadline));
        let job = rig.client.submit_blocking(scope, request, move |context| {
            *lock(&worker_retained) = Some(context);
            worker_entered.open();
            worker_release.wait().map_err(|cause| io::Error::other(cause.to_string()))?;
            Ok::<_, io::Error>(())
        })?;
        if let Err(cause) = entered.wait() {
            release.open();
            return Err(cause);
        }
        let context = match lock(&retained).take() {
            Some(context) => context,
            None => {
                release.open();
                return Err("Actual admitted capture context absent".into());
            }
        };
        Ok(Self { context, job, release })
    }
    pub(super) fn finish(&mut self, rig: &Rig) -> TestResult<JobCompletion<(), io::Error>> {
        self.release.open();
        rig.completion(&mut self.job)
    }
}
impl Drop for HeldCapture {
    fn drop(&mut self) {
        // Release precedes the fixture's ExecutionOwner teardown on every error path.
        self.release.open();
    }
}

pub(super) struct ForeignRelease;
impl MemoryRelease for ForeignRelease {
    fn release(&self, _: u64) {}
}
pub(super) struct SubstitutedPorts {
    pub(super) reads: std::sync::atomic::AtomicUsize,
}
impl CancelControl for SubstitutedPorts {
    fn is_requested(&self) -> bool {
        self.reads.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        true
    }
    fn cancelled(&self) -> WaitFuture {
        self.reads.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        Box::pin(std::future::pending())
    }
}
impl Clock for SubstitutedPorts {
    fn now(&self) -> Moment {
        self.reads.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        Moment::from_nanos(u64::MAX)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        self.reads.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        Box::pin(std::future::pending())
    }
}
pub(super) fn substituted(
    actual: &JobContext,
    memory: MemoryLease,
    ports: Arc<SubstitutedPorts>,
) -> JobContext {
    JobContext::new(
        actual.owner_id(),
        actual.scope_id(),
        actual.spec(),
        actual.deadline(),
        ports.clone(),
        ports,
        AdmittedJobResources::new(memory, None),
    )
}

/// Release held CPU boundary callbacks before the fixture's execution owner can join.
pub(super) struct DropRelease(pub(super) Arc<Gate>);
impl Drop for DropRelease {
    fn drop(&mut self) {
        self.0.open();
    }
}
pub(super) struct OriginDropTask {
    pub(super) origin: Option<super::CaptureBundleOrigin>,
    pub(super) arrivals: Arc<std::sync::atomic::AtomicUsize>,
    pub(super) entered: Arc<Gate>,
    pub(super) release: Arc<Gate>,
}
impl CpuTask<(), io::Error> for OriginDropTask {
    fn step(&mut self, _: &JobContext) -> Result<CpuStep<()>, io::Error> {
        if self.arrivals.fetch_add(1, std::sync::atomic::Ordering::AcqRel) + 1 == 2 {
            self.entered.open();
        }
        if let Err(cause) = self.release.wait() {
            return Err(io::Error::other(cause.to_string()));
        }
        drop(self.origin.take());
        Ok(CpuStep::Complete(()))
    }
}
