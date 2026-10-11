// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify retained watches independently of live children and eventual family exits.
//! This isolated same-executable campaign supplies matching fixture identities only.
//! It grants no coordinator authentication, backend trust or production readiness.
mod activity;
mod cleanup;
mod enrollment;
mod evidence;
mod lifetime;
mod receipt;
mod scan;
mod session;

use crate::process_fixture::progress::{CaseProgress, StopPlan};
use evidence::Evidence;
use scan::{ExternalWake, Scan};
use std::{
    error::Error,
    num::NonZeroUsize,
    sync::Arc,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::{
    execution::{
        Clock, CompletionWake, ExecutionPort, JobClass, JobId, JobSpec, Moment, Operation, Scope,
        ScopeId, ScopeSpec, TerminalKind,
    },
    lifecycle::{CoordinatorEpoch, CoordinatorKey, NamespaceId, ShutdownId},
};
use sugarsubstitute_desktop::{HeadlessLifetime, HeadlessStep, run_headless};
use sugarsubstitute_execution::{
    ExecutionClient, ExecutionOwner, MonotonicClock, WorkerStatus,
    windows_fixture::{FixtureDeadline, FixtureSession},
};
use sugarsubstitute_supervision::WindowsProcessRegistry;

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const HELD: usize = 64;
const SENTINEL: usize = HELD;

/// Run the original sixty-second campaign on the native main thread.
/// Return a receipt only after every actual family, event session and worker retires.
pub(crate) fn run() -> Result<serde_json::Value> {
    let mut fixture = Fixture::new()?;
    if let Err(error) = run_headless(&mut fixture) {
        tracing::error!(operation = "held_campaign_lifetime", cause = %error,
            original_cause = ?error.source(), "Held campaign retains its first failure after actual shutdown");
        return Err(Box::new(error));
    }
    fixture.receipt()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Enrolling,
    Sentinel,
    Cancelling,
    Releasing,
    Collecting,
}

/// Keep protocol proof independent of retained session and family cleanup ownership.
struct HeldCase {
    evidence: Evidence,
    started: bool,
    settlement_observed: bool,
    creation_job: Option<JobId>,
    session: Option<FixtureSession>,
    notice: Option<TerminalKind>,
    closing_action_done: bool,
}
/// Retain one original stop budget and the exact secondary observation identity.
struct Cleanup {
    stop: Option<StopPlan>,
    requested: bool,
    member_watch: Option<JobId>,
}
/// Compose independent event sessions, contained families and a bounded native pump.
struct Fixture {
    clock: Arc<MonotonicClock>,
    deadline: Instant,
    execution_deadline: Moment,
    child_deadline: FixtureDeadline,
    deadline_observed: bool,
    owner_key: CoordinatorKey,
    registry: WindowsProcessRegistry,
    owner: Option<ExecutionOwner>,
    client: Option<ExecutionClient>,
    creation_scope: Option<Scope>,
    event_scope: Option<Scope>,
    wake: Option<Arc<dyn CompletionWake>>,
    external_wake: Option<Arc<ExternalWake>>,
    scan: Scan,
    next_job: u64,
    next_scope: u64,
    cases: [HeldCase; HELD],
    sentinel: CaseProgress,
    sentinel_watch: Option<JobId>,
    sentinel_creation: Option<JobId>,
    sentinel_settlement_observed: bool,
    cleanup: [Cleanup; HELD + 1],
    cleanup_busy: Option<(usize, JobId)>,
    cleanup_cursor: usize,
    phase: Phase,
    enrolling: usize,
    cursor: usize,
    closing: bool,
    execution_stopping: bool,
    workers: Option<WorkerStatus>,
    finished: bool,
}
impl Fixture {
    /// Fix both clock deadlines before creating any admitted native work.
    fn new() -> Result<Self> {
        let started = Instant::now();
        let clock = MonotonicClock::new();
        let deadline =
            started.checked_add(Duration::from_secs(60)).ok_or("Campaign deadline overflow")?;
        let child_deadline = FixtureDeadline::until(deadline)?;
        let local = clock.now();
        let remaining =
            deadline.checked_duration_since(Instant::now()).ok_or("Campaign expired")?;
        let execution_deadline = local.checked_add(remaining)?;
        let owner_key = CoordinatorKey {
            namespace: NamespaceId::new([0x61; 32]),
            epoch: CoordinatorEpoch::new([0x62; 16])?,
        };
        let mut cases = Vec::new();
        cases.try_reserve_exact(HELD)?;
        for index in 0..HELD {
            cases.push(HeldCase {
                evidence: Evidence::new(u64::try_from(index)?, [0, 37, 259][index % 3])?,
                started: false,
                settlement_observed: false,
                creation_job: None,
                session: None,
                notice: None,
                closing_action_done: false,
            });
        }
        let cases = cases.try_into().map_err(|_| "Campaign case inventory differs from 64")?;
        Ok(Self {
            clock,
            deadline,
            execution_deadline,
            child_deadline,
            deadline_observed: false,
            owner_key,
            registry: WindowsProcessRegistry::new(owner_key),
            owner: None,
            client: None,
            creation_scope: None,
            event_scope: None,
            wake: None,
            external_wake: None,
            scan: Scan::default(),
            next_job: 1,
            next_scope: 3,
            cases,
            sentinel: CaseProgress::new(259),
            sentinel_watch: None,
            sentinel_creation: None,
            sentinel_settlement_observed: false,
            cleanup: std::array::from_fn(|_| Cleanup {
                stop: None,
                requested: false,
                member_watch: None,
            }),
            cleanup_busy: None,
            cleanup_cursor: 0,
            phase: Phase::Enrolling,
            enrolling: 0,
            cursor: 0,
            closing: false,
            execution_stopping: false,
            workers: None,
            finished: false,
        })
    }
    /// Allocate an exact job identity; each native owner retains its own deadline.
    fn spec(&mut self, class: JobClass, bytes: u64) -> Result<JobSpec> {
        let id = JobId::new(self.next_job)?;
        self.next_job = self.next_job.checked_add(1).ok_or("Campaign job identity exhausted")?;
        Ok(JobSpec {
            id,
            operation: Operation::NativeLifecycle,
            class,
            deadline: None,
            reservation_bytes: bytes,
            replacement: None,
        })
    }
    /// Give each retained family an independent native lifetime.
    fn native_scope_spec(&mut self) -> Result<ScopeSpec> {
        let id = ScopeId::new(self.next_scope)?;
        self.next_scope =
            self.next_scope.checked_add(1).ok_or("Campaign scope identity exhausted")?;
        Ok(ScopeSpec { id, deadline: None })
    }
    /// Continue a bounded pass without marking native evidence as changed.
    fn notify(&self) -> Result<()> {
        if let Some(wake) = &self.wake {
            wake.wake()?;
        }
        Ok(())
    }
}
