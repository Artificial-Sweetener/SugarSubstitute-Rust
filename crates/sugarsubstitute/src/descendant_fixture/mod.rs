// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove natural root exit and a live descendant retained by the original outer Job.
mod activity;
mod enrollment;
mod evidence;
mod lifetime;
use crate::process_fixture::progress::StopPlan;
use evidence::Evidence;
use std::{
    error::Error,
    num::NonZeroUsize,
    sync::Arc,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::{execution::*, lifecycle::*};
use sugarsubstitute_desktop::{HeadlessLifetime, HeadlessStep, run_headless};
use sugarsubstitute_execution::{
    ExecutionClient, ExecutionOwner, MonotonicClock, WorkerStatus,
    windows::NativeCorrelation,
    windows_fixture::{FixtureDeadline, FixtureSession, MIN_FIXTURE_SESSION_BYTES},
};
use sugarsubstitute_supervision::{
    FamilyBinding, RegistryObservation, WindowsLaunchInput, WindowsProcessRegistry,
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub(crate) fn run() -> Result<serde_json::Value> {
    let mut fixture = Fixture::new()?;
    run_headless(&mut fixture)?;
    fixture.receipt()
}
struct Fixture {
    clock: Arc<MonotonicClock>,
    original: Instant,
    deadline: Moment,
    qpc_deadline: FixtureDeadline,
    owner_key: CoordinatorKey,
    registry: WindowsProcessRegistry,
    owner: Option<ExecutionOwner>,
    client: Option<ExecutionClient>,
    creation_scope: Option<Scope>,
    event_scope: Option<Scope>,
    session: Option<FixtureSession>,
    key: Option<ProcessKey>,
    stop: Option<StopPlan>,
    next_job: u64,
    cleanup_job: Option<JobId>,
    member_job: Option<JobId>,
    cleanup_needed: bool,
    session_retired: bool,
    closing: bool,
    stopping: bool,
    finished: bool,
    deadline_observed: bool,
    evidence: Evidence,
    workers: Option<WorkerStatus>,
    descendant_exit: Option<sugarsubstitute_execution::windows::ProcessExit>,
    member_notice: Option<TerminalKind>,
}
impl Fixture {
    fn new() -> Result<Self> {
        let clock = MonotonicClock::new();
        let lifetime = Duration::from_secs(60);
        let original =
            Instant::now().checked_add(lifetime).ok_or("Descendant campaign deadline overflow")?;
        let deadline = clock.now().checked_add(lifetime)?;
        let qpc_deadline = FixtureDeadline::until(original)?;
        let owner_key = CoordinatorKey {
            namespace: NamespaceId::new([7; 32]),
            epoch: CoordinatorEpoch::new([1; 16])?,
        };
        Ok(Self {
            clock,
            original,
            deadline,
            qpc_deadline,
            owner_key,
            registry: WindowsProcessRegistry::new(owner_key)?,
            owner: None,
            client: None,
            creation_scope: None,
            event_scope: None,
            session: None,
            key: None,
            stop: None,
            next_job: 1,
            cleanup_job: None,
            member_job: None,
            cleanup_needed: false,
            session_retired: false,
            closing: false,
            stopping: false,
            finished: false,
            deadline_observed: false,
            evidence: Evidence::default(),
            workers: None,
            descendant_exit: None,
            member_notice: None,
        })
    }
    fn spec(&mut self, class: JobClass, bytes: u64) -> Result<JobSpec> {
        let id = JobId::new(self.next_job)?;
        self.next_job = self.next_job.checked_add(1).ok_or("Descendant job ID overflow")?;
        Ok(JobSpec {
            id,
            operation: Operation::NativeLifecycle,
            class,
            deadline: None,
            reservation_bytes: bytes,
            replacement: None,
        })
    }
    fn begin_stop(&mut self) -> Result<()> {
        if self.stop.is_some() {
            return Ok(());
        }
        let Some(key) = self.key else { return Ok(()) };
        let plan = StopPlan::new(self.clock.now(), Instant::now())?;
        self.registry.begin_stop(key, ShutdownId::new(1)?, plan.budget)?;
        self.stop = Some(plan);
        self.cleanup_needed = true;
        Ok(())
    }
    fn receipt(&self) -> Result<serde_json::Value> {
        if !self.finished
            || !self.registry.is_empty()
            || !self.session_retired
            || !self.evidence.complete()
        {
            return Err("Descendant receipt lacks actual complete family evidence".into());
        }
        let workers = self.workers.ok_or("Descendant workers missing")?;
        if !(workers.async_finished && workers.cpu_finished && workers.native_finished) {
            return Err("Descendant workers unresolved".into());
        }
        let stop = self.stop.as_ref().ok_or("Descendant stop missing")?;
        Ok(serde_json::json!({"schema_version":1,"fixture":"root_exit_surviving_descendant",
            "original_lifetime_seconds":60,"root_exit_code":0,"ready_observed":self.evidence.ready,
            "challenge_after_root_exit":self.evidence.challenge,"response_after_root_exit":self.evidence.response,
            "release_requested":false,"maximum_retained_members":self.evidence.retained,
            "forced":self.evidence.forced,"descendant_process_id":self.descendant_exit.map(|exit|exit.process_id),
            "descendant_creation_time_100ns":self.descendant_exit.map(|exit|exit.creation_time_100ns),
            "descendant_exit_code":self.descendant_exit.map(|exit|exit.exit_code),
            "member_watch_job_id":self.evidence.member_job.map(JobId::get),"member_watch_success":self.evidence.member_success,
            "grace_boundary_observed":self.evidence.grace,"family_settled":self.evidence.settled,"family_retired":self.evidence.retired,
            "shutdown_started_nanos":stop.budget.started.as_nanos(),"graceful_until_nanos":stop.budget.graceful_until.as_nanos(),
            "settle_until_nanos":stop.budget.settle_until.as_nanos(),"registry_empty":true,
            "workers":{"async_finished":workers.async_finished,"cpu_finished":workers.cpu_finished,"native_finished":workers.native_finished}}))
    }
}
