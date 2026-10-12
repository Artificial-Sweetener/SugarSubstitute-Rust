// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fake only external read/close effects while real execution owns admitted lifetimes.
use super::*;
pub(super) struct Boundary {
    pub(super) scope: Mutex<Option<sugarsubstitute_contracts::execution::Scope>>,
    pub(super) data: Mutex<[VecDeque<Vec<u8>>; 2]>,
    pub(super) eof: [AtomicBool; 2],
    pub(super) signals: Mutex<Option<[Arc<Signal>; 2]>>,
    pub(super) prepared: Gate,
    pub(super) reading: Gate,
    pub(super) issued: Gate,
    pub(super) defer_read: [AtomicBool; 2],
    pub(super) completed: Mutex<[Option<Progress>; 2]>,
    pub(super) retiring: Gate,
    pub(super) allow_retire: AtomicBool,
    pub(super) prepares: AtomicUsize,
    pub(super) closes: AtomicUsize,
    pub(super) releases: AtomicUsize,
    pub(super) fail_prepare: AtomicBool,
    pub(super) fail_retire: AtomicUsize,
    pub(super) panic_read: AtomicBool,
    pub(super) order: Mutex<Vec<usize>>,
    pub(super) advance: Mutex<Option<(Arc<crate::runtime_test_support::ManualClock>, u64)>>,
    pub(super) end_clock_panic: Mutex<Option<Arc<Mutex<Option<std::thread::ThreadId>>>>>,
}
impl Boundary {
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self {
            scope: Mutex::new(None),
            data: Mutex::new([VecDeque::new(), VecDeque::new()]),
            eof: [AtomicBool::new(false), AtomicBool::new(false)],
            signals: Mutex::new(None),
            prepared: Gate::default(),
            reading: Gate::default(),
            issued: Gate::default(),
            defer_read: [AtomicBool::new(false), AtomicBool::new(false)],
            completed: Mutex::new([None, None]),
            retiring: Gate::default(),
            allow_retire: AtomicBool::new(true),
            prepares: AtomicUsize::new(0),
            closes: AtomicUsize::new(0),
            releases: AtomicUsize::new(0),
            fail_prepare: AtomicBool::new(false),
            fail_retire: AtomicUsize::new(0),
            panic_read: AtomicBool::new(false),
            order: Mutex::new(Vec::new()),
            advance: Mutex::new(None),
            end_clock_panic: Mutex::new(None),
        })
    }
    pub(super) fn wake(&self, index: usize) -> TestResult {
        lock(&self.signals).as_ref().ok_or("Output not prepared")?[index].complete();
        Ok(())
    }
    pub(super) fn push(&self, index: usize, bytes: Vec<u8>) -> TestResult {
        lock(&self.data)[index].push_back(bytes);
        self.wake(index)
    }
    pub(super) fn end(&self, index: usize) -> TestResult {
        self.eof[index].store(true, Ordering::Release);
        self.wake(index)
    }
    fn effect(
        &self,
        stream: OutputStream,
        bytes: &mut [u8],
    ) -> Result<Progress, NativeObservationError> {
        self.reading.open();
        if self.panic_read.swap(false, Ordering::AcqRel) {
            std::panic::resume_unwind(Box::new("Controlled pending-read boundary panic"));
        }
        if lock(&self.completed)[stream.index()].is_some() {
            return Ok(Progress::Pending);
        }
        if let Some(source) = lock(&self.data)[stream.index()].pop_front() {
            bytes[..source.len()].copy_from_slice(&source);
            lock(&self.order).push(stream.index());
            if let Some((clock, nanos)) = lock(&self.advance).take() {
                clock.advance(nanos);
            }
            let result = if source.is_empty() {
                Progress::Empty
            } else {
                Progress::Bytes(source.len() as u32)
            };
            if self.defer_read[stream.index()].swap(false, Ordering::AcqRel) {
                lock(&self.completed)[stream.index()] = Some(result);
                self.issued.open();
                return Ok(Progress::Pending);
            }
            return Ok(result);
        }
        Ok(if self.eof[stream.index()].load(Ordering::Acquire) {
            Progress::Eof
        } else {
            Progress::Pending
        })
    }
}
pub(super) struct Lease(pub(super) Arc<Boundary>);
impl OutputLease for Lease {
    fn prepare(
        &mut self,
        _: ResourceKey,
        signals: [Arc<Signal>; 2],
    ) -> Result<[usize; 3], NativeObservationError> {
        *lock(&self.0.signals) = Some(signals);
        self.0.prepares.fetch_add(1, Ordering::AcqRel);
        self.0.prepared.open();
        if self.0.fail_prepare.load(Ordering::Acquire) {
            return Err(invariant("partial_output_prepare"));
        }
        Ok([11, 12, 13])
    }
    fn release_child_ends(&mut self) -> Result<(), NativeObservationError> {
        self.0.releases.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
    fn read(
        &mut self,
        stream: OutputStream,
        bytes: &mut [u8],
    ) -> Result<Progress, NativeObservationError> {
        self.0.effect(stream, bytes).map(|progress| match progress {
            Progress::Pending => Progress::IssuedPending,
            other => other,
        })
    }
    fn poll(
        &mut self,
        stream: OutputStream,
        bytes: &mut [u8],
    ) -> Result<Progress, NativeObservationError> {
        self.0.effect(stream, bytes)
    }
    fn quiesce(&mut self, _: OutputStream) -> Result<(), NativeObservationError> {
        Ok(())
    }
    fn take_completed_read(&mut self, stream: OutputStream) -> Option<Progress> {
        lock(&self.0.completed)[stream.index()].take()
    }
    fn retire(&mut self, pinned: bool) -> Result<bool, NativeObservationError> {
        self.0.retiring.open();
        if self
            .0
            .fail_retire
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_sub(1))
            .is_ok()
        {
            return Err(invariant("output_checked_retirement"));
        }
        if pinned || !self.0.allow_retire.load(Ordering::Acquire) {
            return Ok(false);
        }
        self.0.closes.fetch_add(1, Ordering::AcqRel);
        if let Some(trigger) = lock(&self.0.end_clock_panic).take() {
            *lock(&trigger) = Some(std::thread::current().id());
        }
        Ok(true)
    }
}
pub(super) fn resource() -> TestResult<ResourceKey> {
    use sugarsubstitute_contracts::lifecycle::*;
    Ok(ResourceKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        id: ResourceId::new(1)?,
        generation: ResourceGeneration::new(1)?,
    })
}
pub(super) fn session(rig: &Rig, boundary: &Arc<Boundary>) -> TestResult<WindowsOutputSession> {
    session_with(rig, boundary, 2, 1, None)
}
pub(super) fn session_with(
    rig: &Rig,
    boundary: &Arc<Boundary>,
    scope_id: u64,
    job_id: u64,
    deadline: Option<sugarsubstitute_contracts::execution::Moment>,
) -> TestResult<WindowsOutputSession> {
    use sugarsubstitute_contracts::execution::*;
    let scope = rig.client.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(scope_id)?, deadline: None },
        None,
        true,
    )?;
    let mut job = spec(job_id, JobClass::NativeObservation, MIN_WINDOWS_OUTPUT_BYTES)?;
    job.deadline = deadline;
    job.operation = Operation::NativeLifecycle;
    let session =
        reserve(&rig.client.core, &scope, job, resource()?, Box::new(Lease(boundary.clone())))?;
    *lock(&boundary.scope) = Some(scope);
    boundary.prepared.wait()?;
    loop {
        let count = rig.wake.count();
        rig.client.drain_completions(NonZeroUsize::MIN)?;
        if session.control.snapshot().prepared {
            break;
        }
        rig.wake.wait_after(count)?;
    }
    Ok(session)
}
pub(super) fn completion(
    rig: &Rig,
    session: &mut WindowsOutputSession,
) -> TestResult<
    sugarsubstitute_contracts::execution::JobCompletion<
        WindowsOutputReport,
        NativeObservationError,
    >,
> {
    loop {
        let count = rig.wake.count();
        if let Err(error) = rig.client.drain_completions(NonZeroUsize::MIN) {
            assert!(matches!(
                error.operation(),
                sugarsubstitute_contracts::execution::Operation::NativeLifecycle
                    | sugarsubstitute_contracts::execution::Operation::Infrastructure
            ));
            continue;
        }
        if let Some(value) = session.completion.try_take_completion() {
            return Ok(value);
        }
        rig.wake.wait_after(count)?;
    }
}
