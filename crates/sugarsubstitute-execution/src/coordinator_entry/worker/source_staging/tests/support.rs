// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own actual admitted fixtures and fake only external native effects.
use super::*;
pub(super) struct Fixture {
    pub(super) core: Arc<Core>,
    pub(super) clock: Arc<ManualClock>,
    pub(super) original: Scope,
    pub(super) parent: Scope,
    pub(super) client_scope: Scope,
    pub(super) prepared: Prepared,
    pub(super) active: Active,
    pub(super) session: CoordinatorEntryClientSession,
}
impl Fixture {
    pub(super) fn new() -> TestResult<Self> {
        let clock = Arc::new(ManualClock::default());
        let core = Core::new(
            ExecutionId::new(1)?,
            Arc::new(ClockPort(clock.clone())),
            Arc::new(Wake::default()),
            crate::runtime::DEFAULT_MEMORY_BYTES,
        );
        let client = crate::ExecutionClient { core: core.clone() };
        let original =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let mut prepared = staging_support::prepare(&core, &original)?;
        let parent =
            core.create_scope_kind(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None, true)?;
        let client_scope = core.create_scope_kind(
            ScopeSpec { id: ScopeId::new(3)?, deadline: None },
            Some(&parent),
            true,
        )?;
        let facts = EntryNativeIdentityFacts::new(
            NamespaceId::new([1; 32]),
            &[1, 0, 0, 0, 0, 0, 0, 5],
            1,
            [4; 24],
        )?;
        let input = CoordinatorEntryClientInput::from_owned(
            facts,
            None,
            prepared.input.take().ok_or("Missing input")?,
            CounterDeadline::new(30_000_001_000, 1_000_000_000)?,
        )?;
        let mut specification = spec(2, JobClass::NativeObservation, MIN_ENTRY_CLIENT_BYTES)?;
        specification.operation = Operation::NativeLifecycle;
        let session = reserve_client(&core, &client_scope, specification, input, Box::new(Lease))?;
        let now = core.clock.metadata()?;
        let (queued, wake) = core.take_native_at(now).ok_or("Actual client not dequeued")?;
        core.deliver_capacity_wake(wake);
        let Work::Native(crate::native::NativeWork::CoordinatorEntry(work)) = queued.work else {
            return Err("Incorrect admitted native work".into());
        };
        let active = Active::new(work, queued.context, &core, now);
        Ok(Self { core, clock, original, parent, client_scope, prepared, active, session })
    }
    pub(super) fn retain_source(&mut self) {
        self.active.source_owner = Some(self.prepared.owner.clone());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.core.cancel_job(self.active.context.job_id());
        self.active.outcome = Some(JobOutcome::Cancelled);
        self.active.retirement(&self.core, Moment::from_nanos(0), true);
        drop(self.session.completion.try_take_completion());
        self.core.cancel_job(self.prepared.queued.context.job_id());
        self.prepared.queued.job.stop(
            &self.prepared.queued.context,
            Some(Moment::from_nanos(0)),
            Stop::Cancelled,
        );
        drop(self.prepared.job.try_take_completion());
    }
}
pub(super) struct Lease;
impl EntryLease for Lease {
    fn sample_counter(&mut self) -> Result<(u64, u64), NativeObservationError> {
        Ok((1_000, 1_000_000_000))
    }
    fn prepare(
        &mut self,
        _: &[[Arc<Signal>; 3]],
        _: &JobContext,
        _: Moment,
    ) -> Result<(), EntryTransportError> {
        Err(EntryTransportError::InvalidPhase)
    }
    fn connect(
        &mut self,
        _: usize,
        _: NativeIssue<'_>,
    ) -> Result<IssueDispatch<Progress>, EntryTransportError> {
        Err(EntryTransportError::InvalidPhase)
    }
    fn read(
        &mut self,
        _: usize,
        _: &mut [u8],
        _: NativeIssue<'_>,
    ) -> Result<IssueDispatch<Progress>, EntryTransportError> {
        Err(EntryTransportError::InvalidPhase)
    }
    fn write(
        &mut self,
        _: usize,
        _: &[u8],
        _: NativeIssue<'_>,
    ) -> Result<IssueDispatch<Progress>, EntryTransportError> {
        Err(EntryTransportError::InvalidPhase)
    }
    fn poll(&mut self, _: usize, _: Role) -> Result<Progress, EntryTransportError> {
        Err(EntryTransportError::InvalidPhase)
    }
    fn quiesce(&mut self, _: usize, _: Role) -> Result<(), EntryTransportError> {
        Ok(())
    }
    fn verify(&mut self, _: usize, _: bool) -> Result<KernelPeer, EntryTransportError> {
        Err(EntryTransportError::InvalidPhase)
    }
    fn nonce(&mut self) -> Result<[u8; 32], EntryTransportError> {
        Err(EntryTransportError::InvalidPhase)
    }
    fn retire_connection(&mut self, _: usize, _: bool) -> Result<bool, EntryTransportError> {
        Ok(true)
    }
    fn retire(&mut self) -> Result<bool, EntryTransportError> {
        Ok(true)
    }
}

pub(super) struct ForeignPorts(pub(super) Arc<AtomicUsize>);
impl CancelControl for ForeignPorts {
    fn is_requested(&self) -> bool {
        self.0.fetch_add(1, Ordering::AcqRel);
        false
    }
    fn cancelled(&self) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
impl Clock for ForeignPorts {
    fn now(&self) -> Moment {
        self.0.fetch_add(1, Ordering::AcqRel);
        Moment::from_nanos(u64::MAX)
    }
    fn sleep_until(&self, _: Moment) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(std::future::pending())
    }
}
