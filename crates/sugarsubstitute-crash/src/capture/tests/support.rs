// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fake native mechanics; admission, cancellation, charge and delivery are real.
use super::*;
pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
pub(super) fn coordinator() -> TestResult<CoordinatorKey> {
    use sugarsubstitute_contracts::lifecycle::{CoordinatorEpoch, NamespaceId};
    Ok(CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    })
}
pub(super) fn input() -> Result<CaptureStartInput, CrashError> {
    CaptureStartInput::new(
        r"E:\bundle\client.dll".into(),
        r"E:\bundle\handler.exe".into(),
        r"D:\capture\root".into(),
        [0; 32],
        [0; 32],
        "test-build",
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
#[derive(Default)]
pub(super) struct ManualClock(pub(super) AtomicU64);
impl Clock for ManualClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::Acquire))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
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
    pub(super) fn wait(&self) -> Result<(), std::io::Error> {
        let (state, timeout) = self
            .changed
            .wait_timeout_while(lock(&self.state), Duration::from_secs(10), |state| !*state)
            .map_err(|cause| std::io::Error::other(cause.to_string()))?;
        if timeout.timed_out() && !*state {
            return Err(std::io::Error::other("Controlled boundary did not release"));
        }
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
    pub(super) fn wait(&self, target: usize) -> TestResult {
        let (count, timeout) = self
            .changed
            .wait_timeout_while(lock(&self.count), Duration::from_secs(10), |count| *count < target)
            .map_err(|cause| std::io::Error::other(cause.to_string()))?;
        if timeout.timed_out() && *count < target {
            return Err("Controlled workers did not enter".into());
        }
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
    pub(super) scope: Scope,
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
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
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
    fn wait(&self, old: u64) -> TestResult {
        let (count, timeout) = self
            .wake
            .changed
            .wait_timeout_while(lock(&self.wake.count), Duration::from_secs(10), |count| {
                *count == old
            })
            .map_err(|cause| std::io::Error::other(cause.to_string()))?;
        if timeout.timed_out() && *count == old {
            return Err("Real owner did not notify completion".into());
        }
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
    pub(super) endpoint_reads: Arc<AtomicUsize>,
    pub(super) endpoint_unavailable: Arc<AtomicUsize>,
    pub(super) created: AtomicUsize,
    pub(super) dropped: Arc<AtomicUsize>,
    pub(super) stopped: Arc<AtomicUsize>,
    pub(super) fail_stop: Arc<AtomicUsize>,
    pub(super) entered: Arc<Gate>,
    pub(super) release: Arc<Gate>,
    pub(super) partial_process: Arc<AtomicUsize>,
    pub(super) watch_binds: Arc<AtomicUsize>,
    pub(super) healthy: Arc<AtomicUsize>,
    pub(super) watch_job: Arc<Mutex<Option<WatchJob>>>,
    pub(super) cancelled_permits: Arc<Mutex<Vec<WatchJob>>>,
}
struct FakeLease {
    endpoint_reads: Arc<AtomicUsize>,
    endpoint_unavailable: Arc<AtomicUsize>,
    _memory: MemoryClaim,
    dropped: Arc<AtomicUsize>,
    stopped: Arc<AtomicUsize>,
    fail_stop: Arc<AtomicUsize>,
    entered: Arc<Gate>,
    release: Arc<Gate>,
    partial_process: Arc<AtomicUsize>,
    watch_binds: Arc<AtomicUsize>,
    healthy: Arc<AtomicUsize>,
    watch_job: Arc<Mutex<Option<WatchJob>>>,
    cancelled_permits: Arc<Mutex<Vec<WatchJob>>>,
}
impl NativePort for FakePort {
    fn create(&self, memory: &MemoryLease) -> Result<Box<dyn CaptureLease>, CrashError> {
        self.created.fetch_add(1, Ordering::AcqRel);
        Ok(Box::new(FakeLease {
            endpoint_reads: self.endpoint_reads.clone(),
            endpoint_unavailable: self.endpoint_unavailable.clone(),
            _memory: memory
                .claim((crate::input::STORAGE_BYTES - crate::input::SOURCE_STORAGE_BYTES) as u64)
                .map_err(|cause| {
                    CrashError::new(
                        sugarsubstitute_contracts::crash::CaptureOperation::Runtime,
                        cause,
                    )
                })?,
            dropped: self.dropped.clone(),
            stopped: self.stopped.clone(),
            fail_stop: self.fail_stop.clone(),
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
    fn start(
        &mut self,
        _: &CaptureStartInput,
        _: CoordinatorKey,
        _: &JobContext,
    ) -> Result<(), CrashError> {
        self.entered.open();
        self.release.wait().map_err(|cause| {
            CrashError::new(sugarsubstitute_contracts::crash::CaptureOperation::Registration, cause)
        })?;
        if self.healthy.load(Ordering::Acquire) != 0 {
            return Ok(());
        }
        Err(CrashError::new(
            sugarsubstitute_contracts::crash::CaptureOperation::Registration,
            std::io::Error::other("controlled-registration-failure"),
        ))
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
        if self.fail_stop.load(Ordering::Acquire) != 0 {
            Err(CrashError::invalid(sugarsubstitute_contracts::crash::CaptureOperation::Stop))
        } else {
            Ok(())
        }
    }
    fn retire(&mut self, _: &JobContext) -> Result<bool, CrashError> {
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
