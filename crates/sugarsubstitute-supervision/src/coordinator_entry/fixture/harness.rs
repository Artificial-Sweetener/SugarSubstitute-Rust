// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose actual finite entry jobs and notifications over the shared external fixture.
use super::*;
pub(crate) struct Harness {
    pub(crate) owner: CoordinatorEntryOwner,
    pub(crate) fake: Arc<Fake>,
    pub(crate) clock: Arc<ClockPort>,
    pub(crate) client: ExecutionClient,
    pub(crate) scope: Scope,
    pub(crate) budget: EntrySetupBudget,
    pub(crate) wake: Arc<Wake>,
    executor: Option<ExecutionOwner>,
    next: u64,
}
impl Harness {
    pub(crate) fn new() -> Result<Self, Box<dyn Error>> {
        let clock = Arc::new(ClockPort::default());
        let wake = Arc::new(Wake::default());
        let executor = ExecutionOwner::new(clock.clone(), wake.clone())?;
        let client = executor.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let fake = Arc::new(Fake {
            clock: clock.clone(),
            gate: Mutex::new(None),
            failure: Mutex::new(None),
            expire_after: Mutex::new(None),
            locked: AtomicBool::new(false),
            calls: std::array::from_fn(|_| AtomicUsize::new(0)),
            close_deadlines: Mutex::new(Vec::new()),
            coordinator_inode: Mutex::new(None),
            origin: Mutex::new(ElectionRecordOrigin::Fresh(
                crate::coordinator_entry::election::FreshRecordOrigin::Missing,
            )),
        });
        let path = PathBuf::from(r"E:\entry-inert-input");
        let configuration = StartupConfiguration::new(
            ConnectionConfiguration::Attach(LoopbackOrigin::parse("http://localhost:8188", 8188)?),
            ProfileId::new("Profile_1".to_owned())?,
            Presentation::new("en", Theme::Light, false)?,
        );
        let request = EntryLaunchRequest::new(
            configuration,
            path.clone(),
            path.clone(),
            path.clone(),
            path.clone(),
            path.clone(),
            vec![0, 0],
        )?;
        let input = CoordinatorEntryInput::new(
            path.clone(),
            path,
            crate::coordinator_entry::DiagnosticStorageLocation::Account,
            request,
        )?;
        let budget = EntrySetupBudget::new(Moment::from_nanos(0))?;
        let owner = CoordinatorEntryOwner::with_port(input, budget, fake.clone());
        Ok(Self {
            owner,
            fake,
            clock,
            client,
            scope,
            budget,
            wake,
            executor: Some(executor),
            next: 1,
        })
    }
    pub(crate) fn spec(&mut self, until: Moment) -> Result<JobSpec, Box<dyn Error>> {
        let id = JobId::new(self.next)?;
        self.next += 1;
        Ok(JobSpec {
            id,
            class: JobClass::InfrastructureLifecycle,
            operation: Operation::NativeLifecycle,
            deadline: Some(until),
            reservation_bytes: ENTRY_PREPARATION_BYTES,
            replacement: None,
        })
    }
    pub(crate) fn setup_spec(&mut self) -> Result<JobSpec, Box<dyn Error>> {
        self.spec(self.budget.until())
    }
    pub(crate) fn discover(&mut self) -> Result<EntryObservation, Box<dyn Error>> {
        let spec = self.setup_spec()?;
        self.owner.submit_discovery(&self.client, &self.scope, spec)?;
        self.drain()?.ok_or_else(|| "discovery produced no observation".into())
    }
    pub(crate) fn elect(&mut self) -> Result<EntryObservation, Box<dyn Error>> {
        let spec = self.setup_spec()?;
        self.owner.submit_election(&self.client, &self.scope, spec)?;
        self.drain()?.ok_or_else(|| "election produced no observation".into())
    }
    pub(crate) fn close(&mut self, until: Moment) -> Result<EntryObservation, Box<dyn Error>> {
        let spec = self.spec(until)?;
        self.owner.submit_close(&self.client, &self.scope, spec)?;
        self.drain()?.ok_or_else(|| "close produced no observation".into())
    }
    /// Join the actual test execution owner after entry native retirement is consumed.
    /// Worker contexts can retain their own job reservation after typed completion delivery.
    pub(crate) fn join_execution_after_retirement(&mut self) -> Result<(), Box<dyn Error>> {
        let snapshot = self.owner.snapshot();
        if snapshot.phase != super::super::types::EntryPhase::Settled || snapshot.pending.is_some()
        {
            return Err("entry retirement remains unconsumed".into());
        }
        let executor = self.executor.take().ok_or("execution owner already joined")?;
        drop(executor);
        Ok(())
    }
    pub(crate) fn drain(&mut self) -> Result<Option<EntryObservation>, Box<dyn Error>> {
        loop {
            let revision = *lock(&self.wake.revision);
            let batch = self
                .client
                .drain_completions(NonZeroUsize::new(32).ok_or("entry test batch bound")?)?;
            let event = self.owner.consume(&batch)?;
            if self.owner.snapshot().pending.is_none() {
                return Ok(event);
            }
            drop(
                self.wake
                    .changed
                    .wait_while(lock(&self.wake.revision), |current| *current == revision)
                    .map_err(|_| "entry wake poisoned")?,
            );
        }
    }
    pub(crate) fn consume_job<T, E>(
        &mut self,
        job: &mut OwnedJob<T, E>,
    ) -> Result<sugarsubstitute_contracts::execution::JobCompletion<T, E>, Box<dyn Error>> {
        loop {
            let revision = *lock(&self.wake.revision);
            if let Some(done) = job.try_take_completion() {
                return Ok(done);
            }
            self.client
                .drain_completions(NonZeroUsize::new(32).ok_or("entry test batch bound")?)?;
            if let Some(done) = job.try_take_completion() {
                return Ok(done);
            }
            drop(
                self.wake
                    .changed
                    .wait_while(lock(&self.wake.revision), |current| *current == revision)
                    .map_err(|_| "entry proof wake poisoned")?,
            );
        }
    }
    /// Consume one exact claims-owned job through its actual public completion owner.
    pub(crate) fn consume_claims(
        &mut self,
        claims: &mut crate::WindowsResourceClaims,
        expected: JobId,
    ) -> Result<(), Box<dyn Error>> {
        loop {
            let revision = *lock(&self.wake.revision);
            let batch = self
                .client
                .drain_completions(NonZeroUsize::new(32).ok_or("entry test batch bound")?)?;
            let matched = batch
                .notifications
                .iter()
                .any(|notice| notice.scope_id == self.scope.id() && notice.job_id == expected);
            let consumed = claims.consume(&batch)?;
            if matched && consumed > 0 {
                return Ok(());
            }
            drop(
                self.wake
                    .changed
                    .wait_while(lock(&self.wake.revision), |current| *current == revision)
                    .map_err(|_| "entry claims wake poisoned")?,
            );
        }
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        if let Some((_, gate)) = lock(&self.fake.gate).as_ref() {
            gate.release();
        }
        self.owner.close_admission();
    }
}
