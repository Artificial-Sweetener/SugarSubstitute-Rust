// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fake native mechanics; admission, cancellation, charge and delivery are real.
use super::*;
pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
/// One exclusive fake native callback; the production owner supplies the lock boundary.
type EndpointProbe = Box<dyn FnOnce() -> Result<(), CrashError> + Send>;
pub(super) fn coordinator() -> TestResult<CoordinatorKey> {
    use sugarsubstitute_contracts::lifecycle::{CoordinatorEpoch, NamespaceId};
    Ok(CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    })
}
pub(super) fn input() -> Result<CaptureStartInput, CrashError> {
    CaptureStartInput::new(
        r"D:\capture\root".into(),
        sugarsubstitute_supervision::InstalledReleaseOwner::new().capture_selection(),
    )
}
pub(super) fn work_spec(id: u64, deadline: Moment) -> TestResult<JobSpec> {
    Ok(JobSpec {
        id: JobId::new(id)?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(deadline),
        reservation_bytes: crate::input::STORAGE_BYTES as u64,
        replacement: None,
    })
}
pub(super) fn close_spec(id: u64, deadline: Moment) -> TestResult<JobSpec> {
    let mut spec = work_spec(id, deadline)?;
    spec.reservation_bytes = 4096;
    Ok(spec)
}
pub(super) use super::clock::{ClockCallback, ManualClock};
pub(super) type LeaseProbe = Box<dyn FnOnce(&JobContext) + Send>;
#[derive(Default)]
pub(super) struct Gate {
    state: Mutex<bool>,
    changed: Condvar,
}
impl Gate {
    pub(super) fn open(&self) {
        *lock(&self.state) = true;
        self.changed.notify_all();
    }
    /// Block until the gate opens; a gate that is never opened leaves the test running.
    pub(super) fn wait(&self) -> Result<(), std::io::Error> {
        drop(
            self.changed
                .wait_while(lock(&self.state), |state| !*state)
                .map_err(|cause| std::io::Error::other(cause.to_string()))?,
        );
        Ok(())
    }
}
#[derive(Default)]
pub(super) struct Barrier {
    count: Mutex<usize>,
    changed: Condvar,
}
impl Barrier {
    pub(super) fn arrive(&self) {
        *lock(&self.count) += 1;
        self.changed.notify_all();
    }
    /// Block until `target` workers have arrived.
    pub(super) fn wait(&self, target: usize) -> TestResult {
        drop(
            self.changed
                .wait_while(lock(&self.count), |count| *count < target)
                .map_err(|cause| std::io::Error::other(cause.to_string()))?,
        );
        Ok(())
    }
}
#[derive(Default)]
struct Wake {
    count: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *lock(&self.count) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
pub(super) struct Rig {
    _owner: ExecutionOwner,
    pub(super) client: ExecutionClient,
    pub(super) clock: Arc<ManualClock>,
    wake: Arc<Wake>,
    pub(super) scope: Arc<Scope>,
    pub(super) native_scope: Scope,
}
impl Rig {
    /// Join the actual execution owner before returning its original scope for observation.
    pub(super) fn join(self) -> Scope {
        drop(self._owner);
        self.native_scope
    }
    pub(super) fn new() -> TestResult<Self> {
        let clock = Arc::new(ManualClock::default());
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
        let client = owner.client();
        let scope = Arc::new(
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?,
        );
        let native_scope =
            client.create_native_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
        Ok(Self { _owner: owner, client, clock, wake, scope, native_scope })
    }
    pub(super) fn watch(&self, id: u64) -> TestResult<ProcessWatchPermit> {
        let mut spec = close_spec(id, Moment::from_nanos(60_000_000_000))?;
        spec.class = JobClass::NativeObservation;
        spec.deadline = None;
        Ok(self.client.reserve_process_watch(
            &self.native_scope,
            spec,
            NativeCorrelation::new(id)?,
        )?)
    }
    fn count(&self) -> u64 {
        *lock(&self.wake.count)
    }
    /// Block until the real owner publishes a completion wake newer than `old`.
    fn wait(&self, old: u64) -> TestResult {
        drop(
            self.wake
                .changed
                .wait_while(lock(&self.wake.count), |count| *count == old)
                .map_err(|cause| std::io::Error::other(cause.to_string()))?,
        );
        Ok(())
    }
    pub(super) fn completion<T, E>(
        &self,
        job: &mut OwnedJob<T, E>,
    ) -> TestResult<JobCompletion<T, E>> {
        loop {
            let old = self.count();
            if let Some(value) = job.try_take_completion() {
                return Ok(value);
            }
            self.client.drain_completions(NonZeroUsize::MIN)?;
            if let Some(value) = job.try_take_completion() {
                return Ok(value);
            }
            self.wait(old)?;
        }
    }
    /// Observe actual terminal custody after an explicitly expected fatal cause.
    /// Every fatal mailbox cause is checked; this does not mint new admission.
    pub(super) fn fatal_completion<T, E>(
        &self,
        job: &mut OwnedJob<T, E>,
        original: &str,
    ) -> TestResult<JobCompletion<T, E>> {
        loop {
            let old = self.count();
            if let Some(value) = job.try_take_completion() {
                return Ok(value);
            }
            if let Err(cause) = self.client.drain_completions(NonZeroUsize::MIN) {
                assert!(cause.to_string().contains(original));
            }
            if let Some(value) = job.try_take_completion() {
                return Ok(value);
            }
            self.wait(old)?;
        }
    }
    pub(super) fn observe_until(
        &self,
        capture: &mut WindowsCrashCapture,
        done: impl Fn(&WindowsCrashCapture) -> bool,
    ) -> TestResult {
        loop {
            let old = self.count();
            let activity = self.client.drain_completions(NonZeroUsize::MIN)?;
            capture.observe(&activity);
            if done(capture) {
                return Ok(());
            }
            self.wait(old)?;
        }
    }
}
#[derive(Default)]
pub(super) struct FakePort {
    pub(super) start_address: Arc<AtomicUsize>,
    pub(super) retire_address: Arc<AtomicUsize>,
    pub(super) start_probe: Arc<Mutex<Option<LeaseProbe>>>,
    pub(super) build_probe: Arc<Mutex<Option<ClockCallback>>>,
    pub(super) start_failure: Arc<Mutex<Option<CrashError>>>,
    pub(super) retire_probe: Arc<Mutex<Option<LeaseProbe>>>,
    pub(super) endpoint_probe: Arc<Mutex<Option<EndpointProbe>>>,
    pub(super) endpoint_reads: Arc<AtomicUsize>,
    pub(super) endpoint_unavailable: Arc<AtomicUsize>,
    pub(super) created: AtomicUsize,
    pub(super) dropped: Arc<AtomicUsize>,
    pub(super) stopped: Arc<AtomicUsize>,
    pub(super) fail_stop: Arc<AtomicUsize>,
    pub(super) stop_failure: Arc<Mutex<Option<CrashError>>>,
    pub(super) entered: Arc<Gate>,
    pub(super) release: Arc<Gate>,
    pub(super) partial_process: Arc<AtomicUsize>,
    pub(super) watch_binds: Arc<AtomicUsize>,
    pub(super) healthy: Arc<AtomicUsize>,
    pub(super) watch_job: Arc<Mutex<Option<WatchJob>>>,
    pub(super) cancelled_permits: Arc<Mutex<Vec<WatchJob>>>,
}
struct FakeLease {
    start_address: Arc<AtomicUsize>,
    retire_address: Arc<AtomicUsize>,
    start_probe: Arc<Mutex<Option<LeaseProbe>>>,
    build_probe: Arc<Mutex<Option<ClockCallback>>>,
    start_failure: Arc<Mutex<Option<CrashError>>>,
    retire_probe: Arc<Mutex<Option<LeaseProbe>>>,
    endpoint_probe: Arc<Mutex<Option<EndpointProbe>>>,
    endpoint_reads: Arc<AtomicUsize>,
    endpoint_unavailable: Arc<AtomicUsize>,
    _memory: MemoryClaim,
    _admission: sugarsubstitute_execution::CaptureBundleAdmission,
    dropped: Arc<AtomicUsize>,
    stopped: Arc<AtomicUsize>,
    fail_stop: Arc<AtomicUsize>,
    stop_failure: Arc<Mutex<Option<CrashError>>>,
    entered: Arc<Gate>,
    release: Arc<Gate>,
    partial_process: Arc<AtomicUsize>,
    watch_binds: Arc<AtomicUsize>,
    healthy: Arc<AtomicUsize>,
    watch_job: Arc<Mutex<Option<WatchJob>>>,
    cancelled_permits: Arc<Mutex<Vec<WatchJob>>>,
}
impl NativePort for FakePort {
    fn create(
        &self,
        admission: sugarsubstitute_execution::CaptureBundleAdmission,
        _execution: ExecutionClient,
    ) -> Result<Box<dyn CaptureLease>, CrashError> {
        let context = admission
            .original_context()
            .map_err(|cause| {
                CrashError::new(sugarsubstitute_contracts::crash::CaptureOperation::Runtime, cause)
            })?
            .clone();
        let memory = context.memory();
        self.created.fetch_add(1, Ordering::AcqRel);
        Ok(Box::new(FakeLease {
            start_address: self.start_address.clone(),
            retire_address: self.retire_address.clone(),
            start_probe: self.start_probe.clone(),
            build_probe: self.build_probe.clone(),
            start_failure: self.start_failure.clone(),
            retire_probe: self.retire_probe.clone(),
            endpoint_probe: self.endpoint_probe.clone(),
            endpoint_reads: self.endpoint_reads.clone(),
            endpoint_unavailable: self.endpoint_unavailable.clone(),
            _memory: memory
                .claim(
                    (crate::input::BASE_STORAGE_BYTES - crate::input::SOURCE_STORAGE_BYTES) as u64,
                )
                .map_err(|cause| {
                    CrashError::new(
                        sugarsubstitute_contracts::crash::CaptureOperation::Runtime,
                        cause,
                    )
                })?,
            _admission: admission,
            dropped: self.dropped.clone(),
            stopped: self.stopped.clone(),
            fail_stop: self.fail_stop.clone(),
            stop_failure: self.stop_failure.clone(),
            entered: self.entered.clone(),
            release: self.release.clone(),
            partial_process: self.partial_process.clone(),
            watch_binds: self.watch_binds.clone(),
            healthy: self.healthy.clone(),
            watch_job: self.watch_job.clone(),
            cancelled_permits: self.cancelled_permits.clone(),
        }))
    }
}
impl CaptureLease for FakeLease {
    fn take_root_storage(&mut self) -> Result<crate::native::RootStorageParts, CrashError> {
        Err(CrashError::invalid(sugarsubstitute_contracts::crash::CaptureOperation::Observation))
    }
    fn prepare_finished(
        &mut self,
        _: &mut crate::incident::CapturedReport,
        _: &mut sugarsubstitute_supervision::diagnostic_storage::DiagnosticStorage,
        _: &sugarsubstitute_supervision::diagnostic_storage::DiagnosticStorageOperation,
        _: &JobContext,
    ) -> Result<(), crate::incident::IncidentStoreError> {
        Err(crate::incident::IncidentStoreError::MissingPayload)
    }
    fn start(
        &mut self,
        _: CaptureStartInput,
        _: CoordinatorKey,
        context: &JobContext,
    ) -> Result<(), CrashError> {
        self.start_address.store(std::ptr::from_ref(self) as usize, Ordering::Release);
        self.entered.open();
        self.release.wait().map_err(|cause| {
            CrashError::new(sugarsubstitute_contracts::crash::CaptureOperation::Registration, cause)
        })?;
        let probe = lock(&self.start_probe).take();
        if let Some(probe) = probe {
            probe(context);
        }
        if self.healthy.load(Ordering::Acquire) != 0 {
            return Ok(());
        }
        if let Some(cause) = lock(&self.start_failure).clone() {
            return Err(cause);
        }
        Err(CrashError::new(
            sugarsubstitute_contracts::crash::CaptureOperation::Registration,
            std::io::Error::other("controlled-registration-failure"),
        ))
    }
    fn creator(&self) -> Option<crate::capture::CaptureCreator> {
        crate::capture::CaptureCreator::new(1, 11).ok()
    }
    fn build_identity(&self) -> Option<&str> {
        let probe = lock(&self.build_probe).take();
        if let Some(probe) = probe {
            probe();
        }
        Some("test-build")
    }
    fn handler(&self) -> Option<CaptureHandlerIdentity> {
        (self.healthy.load(Ordering::Acquire) != 0)
            .then_some(CaptureHandlerIdentity { process_id: 17, creation_time_100ns: 23 })
    }
    fn has_process(&self) -> bool {
        self.partial_process.load(Ordering::Acquire) != 0
            || self.healthy.load(Ordering::Acquire) != 0
    }
    fn registered(&self) -> bool {
        self.healthy.load(Ordering::Acquire) != 0
    }
    fn endpoint(
        &self,
        expected: sugarsubstitute_execution::windows_capture_attach::CaptureAttachExpectation,
    ) -> Result<
        sugarsubstitute_execution::windows_capture_attach::CaptureEndpointObservation,
        CrashError,
    > {
        let probe = lock(&self.endpoint_probe).take();
        if let Some(probe) = probe {
            probe()?;
        }
        self.endpoint_reads.fetch_add(1, Ordering::AcqRel);
        if self.endpoint_unavailable.load(Ordering::Acquire) != 0 {
            return Err(CrashError::new(
                sugarsubstitute_contracts::crash::CaptureOperation::Observation,
                std::io::Error::other("controlled-endpoint-unavailable"),
            ));
        }
        sugarsubstitute_execution::windows_capture_attach::CaptureEndpointObservation::new(
            &expected,
            sugarsubstitute_execution::windows_capture_attach::CaptureEndpointProvenance::CreatorInherited,
            expected.creator(), expected.handler(),
            &r"\\.\pipe\crashpad_fixture".encode_utf16().collect::<Vec<_>>(),
        ).map_err(|cause| CrashError::new(
            sugarsubstitute_contracts::crash::CaptureOperation::Observation, cause))
    }
    fn callback_pin_bytes(&self) -> u64 {
        0
    }
    fn callback_pin_reservation_bytes(&self) -> u64 {
        0
    }
    fn bind_watch(
        &mut self,
        permit: ProcessWatchPermit,
    ) -> Result<WatchJob, crate::native::BindFailed> {
        self.watch_binds.fetch_add(1, Ordering::AcqRel);
        if let Some(job) = lock(&self.watch_job).take() {
            lock(&self.cancelled_permits).push(permit.cancel_unbound());
            return Ok(job);
        }
        Ok(permit.cancel_unbound())
    }
    fn request_stop(&mut self) -> Result<(), CrashError> {
        self.stopped.fetch_add(1, Ordering::AcqRel);
        if let Some(cause) = lock(&self.stop_failure).clone() {
            return Err(cause);
        }
        if self.fail_stop.load(Ordering::Acquire) != 0 {
            Err(CrashError::invalid(sugarsubstitute_contracts::crash::CaptureOperation::Stop))
        } else {
            Ok(())
        }
    }
    fn retire(&mut self, context: &JobContext) -> Result<bool, CrashError> {
        self.retire_address.store(std::ptr::from_ref(self) as usize, Ordering::Release);
        let probe = lock(&self.retire_probe).take();
        if let Some(probe) = probe {
            probe(context);
        }
        Ok(self.stopped.load(Ordering::Acquire) >= 2 || self.fail_stop.load(Ordering::Acquire) == 0)
    }
    fn take_error(&mut self) -> Option<CrashError> {
        None
    }
    fn take_omitted_errors(&mut self) -> u64 {
        0
    }
}
impl Drop for FakeLease {
    fn drop(&mut self) {
        self.dropped.fetch_add(1, Ordering::AcqRel);
    }
}

/// Keep the actual execution owner outside module-output claims through test teardown.
pub(crate) struct ModuleClaimFixture(Rig);
impl ModuleClaimFixture {
    /// Join the actual owner before asserting the original scope's release.
    pub(crate) fn finish(self) -> u64 {
        let scope = self.0.scope.clone();
        drop(self.0);
        scope.snapshot().reserved_bytes
    }
}
/// Obtain a genuine finite-job context; the fixture never mints admission or trust facts.
pub(crate) fn module_claim_context() -> TestResult<(ModuleClaimFixture, JobContext)> {
    let rig = Rig::new()?;
    let mut job = rig.client.submit_blocking(
        &rig.scope,
        work_spec(1, Moment::from_nanos(60_000_000_000))?,
        |context| Ok::<_, std::io::Error>(context.clone()),
    )?;
    let completion = rig.completion(&mut job)?;
    let JobOutcome::Success(context) = completion.outcome else {
        return Err("Real module-claim context did not complete".into());
    };
    drop(completion.memory);
    drop(job);
    Ok((ModuleClaimFixture(rig), context))
}
