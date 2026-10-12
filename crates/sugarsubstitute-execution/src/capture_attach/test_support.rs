// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fake only external native operations; admission, custody, clocks and cleanup remain actual.
use super::*;
use crate::runtime_test_support::{Gate, Rig, TestResult, spec};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use sugarsubstitute_contracts::{
    crash::CaptureBudget, execution::*, incident::BuildIdentity, lifecycle::*,
};
use types::*;

pub(crate) fn expected() -> TestResult<CaptureAttachExpectation> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let run = RunKey { owner, instance: InstanceId::new(1)?, generation: RunGeneration::new(1)? };
    Ok(CaptureAttachExpectation::new(
        owner,
        run,
        &BuildIdentity::new("test-build")?,
        CaptureProcessFacts::new(3, 4)?,
        CaptureProcessFacts::new(5, 6)?,
        CaptureBudget::new(Moment::from_nanos(0))?,
    )?)
}
pub(super) fn scope(rig: &Rig) -> TestResult<Scope> {
    Ok(rig.client.create_native_scope(
        ScopeSpec { id: ScopeId::new(2)?, deadline: None },
        Some(&rig.scope),
    )?)
}
pub(super) fn attach_spec(id: u64) -> TestResult<JobSpec> {
    let mut request = spec(id, JobClass::NativeObservation, 16_384)?;
    request.operation = Operation::NativeLifecycle;
    Ok(request)
}
pub(super) type ClockProbe = Box<dyn FnOnce(&Arc<Shared>) + Send>;
#[derive(Default)]
pub(super) struct Boundary {
    pub(super) clock_probe: Mutex<Option<ClockProbe>>,
    pub(super) context_override: Mutex<Option<JobContext>>,
    pub(super) started: Gate,
    pub(super) retiring: Gate,
    pub(super) closed: Gate,
    pub(super) source_retired: Gate,
    pub(super) source_attempted: Gate,
    pub(super) source_blocked: AtomicBool,
    pub(super) ready: AtomicBool,
    pub(super) close_allowed: AtomicBool,
    pub(super) physical_closed: AtomicBool,
    pub(super) observations: AtomicUsize,
    pub(super) retirements: AtomicUsize,
    pub(super) starts: AtomicUsize,
    pub(super) signal: Mutex<Option<Arc<Signal>>>,
}
impl Boundary {
    pub(super) fn signal(&self) -> TestResult {
        lock(&self.signal).as_ref().ok_or("Actual native boundary not prepared")?.complete();
        Ok(())
    }
    pub(super) fn settle(&self) -> TestResult {
        self.close_allowed.store(true, Ordering::Release);
        self.signal()
    }
}
impl CaptureEndpointSource for Boundary {
    fn observe(
        &self,
        expected: CaptureAttachExpectation,
        _: &JobContext,
    ) -> Result<CaptureEndpointObservation, CaptureEndpointSourceError> {
        self.observations.fetch_add(1, Ordering::AcqRel);
        let endpoint: Vec<u16> = r"\\.\pipe\controlled_capture".encode_utf16().collect();
        CaptureEndpointObservation::new(
            &expected,
            CaptureEndpointProvenance::CreatorInherited,
            expected.creator(),
            expected.handler(),
            &endpoint,
        )
        .map_err(|cause| CaptureEndpointSourceError::new("controlled_endpoint", cause))
    }
    fn native_retired(
        &self,
        _: &CaptureAttachRetirementReport,
    ) -> Result<(), CaptureEndpointSourceError> {
        if !self.physical_closed.load(Ordering::Acquire) {
            return Err(CaptureEndpointSourceError::new(
                "controlled_retirement",
                std::io::Error::other("Native close has not happened"),
            ));
        }
        self.source_attempted.open();
        if self.source_blocked.load(Ordering::Acquire) {
            return Err(CaptureEndpointSourceError::new(
                "controlled_source_retirement",
                std::io::Error::other("Source retirement remains externally blocked"),
            ));
        }
        self.retirements.fetch_add(1, Ordering::AcqRel);
        self.source_retired.open();
        Ok(())
    }
}
pub(super) struct Lease(pub(super) Arc<Boundary>);
impl AttachLease for Lease {
    fn prepare(
        &mut self,
        _: CaptureEndpointObservation,
        context: &JobContext,
        signal: Arc<Signal>,
        shared: &Arc<Shared>,
    ) -> Result<(), NativeObservationError> {
        *lock(&self.0.signal) = Some(signal);
        let probe = lock(&self.0.clock_probe).take();
        if let Some(probe) = probe {
            probe(shared);
        }
        let alternate = lock(&self.0.context_override).take();
        let context = alternate.as_ref().unwrap_or(context);
        super::issue::execute(shared, context, || {
            self.0.starts.fetch_add(1, Ordering::AcqRel);
            self.0.started.open();
            Ok(())
        })?;
        Ok(())
    }
    fn progress(&mut self) -> Result<Option<[usize; 2]>, NativeObservationError> {
        Ok(self.0.ready.load(Ordering::Acquire).then_some([41, 42]))
    }
    fn prepared(&self) -> bool {
        self.0.ready.load(Ordering::Acquire)
    }
    fn retire(
        &mut self,
        borrowed: bool,
        _: &CaptureCleanupSnapshot,
    ) -> Result<bool, NativeObservationError> {
        self.0.retiring.open();
        if borrowed || !self.0.close_allowed.load(Ordering::Acquire) {
            return Ok(false);
        }
        self.0.physical_closed.store(true, Ordering::Release);
        self.0.closed.open();
        Ok(true)
    }
}
pub(super) fn submit(
    rig: &Rig,
    scope: &Scope,
    boundary: &Arc<Boundary>,
) -> TestResult<OwnedJob<CaptureAttachLaunchPin, CaptureAttachFailure>> {
    admission::reserve(
        &rig.client.core,
        scope,
        attach_spec(1)?,
        expected()?,
        boundary.clone(),
        Box::new(Lease(boundary.clone())),
    )
    .map_err(|cause| format!("{cause:?}").into())
}
pub(super) fn take_pin(
    rig: &Rig,
    job: &mut OwnedJob<CaptureAttachLaunchPin, CaptureAttachFailure>,
) -> TestResult<CaptureAttachLaunchPin> {
    let completion = rig.completion(job)?;
    let JobOutcome::Success(pin) = completion.outcome else {
        return Err("Actual preparation did not succeed".into());
    };
    drop(completion.memory);
    rig.client.drain_completions(std::num::NonZeroUsize::MIN)?;
    Ok(pin)
}

/// Release only controlled external boundaries if an assertion unwinds.
pub(super) struct ReleaseBoundary {
    boundary: Arc<Boundary>,
    core: Arc<Core>,
}
impl ReleaseBoundary {
    pub(super) fn new(core: &Arc<Core>, boundary: &Arc<Boundary>) -> Self {
        Self { boundary: boundary.clone(), core: core.clone() }
    }
}
impl Drop for ReleaseBoundary {
    fn drop(&mut self) {
        self.boundary.ready.store(true, Ordering::Release);
        self.boundary.close_allowed.store(true, Ordering::Release);
        self.boundary.source_blocked.store(false, Ordering::Release);
        if let Some(signal) = lock(&self.boundary.signal).as_ref() {
            signal.complete();
        }
        self.core.changed();
    }
}
pub(super) fn take_retirement_report(
    rig: &Rig,
    retirement: &mut CaptureAttachRetirement,
) -> TestResult<Result<CaptureAttachRetirementReport, CaptureAttachFailure>> {
    loop {
        let previous = rig.wake.count();
        if let Some(report) = retirement.try_take_report() {
            return Ok(report);
        }
        rig.client.drain_completions(std::num::NonZeroUsize::MIN)?;
        if let Some(report) = retirement.try_take_report() {
            return Ok(report);
        }
        rig.wake.wait_after(previous)?;
    }
}
