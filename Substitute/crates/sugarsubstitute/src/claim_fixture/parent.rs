// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive real cross-process claim evidence while retaining each native owner.
mod activity;
mod cleanup;
mod delivery;
mod enrollment;
mod lifetime;
mod phases;
mod receipt;
mod storage;
use super::{
    Result, failure,
    options::{CampaignOptions, Role, hex_text},
    receipt::ChildReceipt,
};
use crate::campaign_watch::{self, CampaignWatch};
use crate::process_fixture::progress::{CaseProgress, StopPlan};
use std::{num::NonZeroUsize, sync::Arc, time::Instant};
use sugarsubstitute_contracts::{
    configuration::LaunchConfiguration,
    execution::{
        Clock, CompletionWake, ExecutionPort, JobClass, JobId, JobSpec, Moment, OwnedJob, Scope,
        ScopeId, ScopeSpec, TerminalKind,
    },
    lifecycle::{
        CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, ResourceGeneration, ResourceId,
        ResourceKey, RunGeneration, RunKey, ShutdownId,
    },
};
use sugarsubstitute_desktop::{HeadlessLifetime, HeadlessStep, run_headless};
use sugarsubstitute_execution::{
    ExecutionClient, ExecutionOwner, MonotonicClock, WorkerStatus,
    windows::NativeCorrelation,
    windows_fixture::{
        FixtureDeadline, FixtureObservation, FixtureSession, FixtureSessionReport,
        MIN_FIXTURE_SESSION_BYTES,
    },
};
use sugarsubstitute_supervision::{
    CLAIM_OPERATION_BYTES, CLAIM_POOL_BYTES, ClaimAdmissionError, ClaimLaunchJobs,
    ClaimObservation, FamilyBinding, LaunchEnvironment, LaunchTicket, RegistryObservation,
    ResolutionTicket, WindowsLaunchCommand, WindowsLaunchInput, WindowsProcessRegistry,
    WindowsResourceClaims,
};

/// Produce a receipt only after contained processes, claims and native pumping retire.
pub(crate) fn run(options: CampaignOptions) -> Result<serde_json::Value> {
    let mut fixture = Fixture::new(options)?;
    run_headless(&mut fixture)?;
    fixture.receipt()
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Registry,
    Setup,
    Initialize,
    Resolve,
    Acquire,
    CoordinatorContender,
    BackendBefore,
    LaunchManaged,
    ManagedExit,
    BackendAfterExit,
    ManagedSettlement,
    ManagedRelease,
    BackendSuccess,
    ActiveHolder,
    ActiveKill,
    Predecessor,
    PredecessorAgain,
    Close,
    Done,
}
struct Family {
    ticket: LaunchTicket,
    creation: JobId,
    root_watch: JobId,
    settlement_transferred: bool,
    progress: CaseProgress,
    stop: Option<StopPlan>,
    cleanup_requested: bool,
    cleanup_job: Option<JobId>,
    member_job: Option<JobId>,
}
impl Family {
    fn new(ticket: LaunchTicket, code: u32, creation: JobId, root_watch: JobId) -> Result<Self> {
        let mut progress = CaseProgress::new(code);
        progress.bind(ticket.process())?;
        Ok(Self {
            ticket,
            creation,
            root_watch,
            settlement_transferred: false,
            progress,
            stop: None,
            cleanup_requested: false,
            cleanup_job: None,
            member_job: None,
        })
    }
}
struct ChildCase {
    role: Role,
    index: usize,
    session: Option<FixtureSession>,
    family: Option<Family>,
    prepared: bool,
    ready: bool,
    challenge_sent: bool,
    responded: bool,
    release_sent: bool,
    abortive: bool,
    event_cancel_requested: bool,
    event_notice: Option<TerminalKind>,
    event_retired: bool,
    event_report: Option<FixtureSessionReport>,
    receipt: Option<ChildReceipt>,
    read: Option<OwnedJob<ChildReceipt, std::io::Error>>,
    read_notice: bool,
}
struct Fixture {
    options: CampaignOptions,
    clock: Arc<MonotonicClock>,
    watch: CampaignWatch,
    execution_deadline: Moment,
    child_deadline: FixtureDeadline,
    owner: Option<ExecutionOwner>,
    client: Option<ExecutionClient>,
    scope: Option<Scope>,
    cleanup_scope: Option<Scope>,
    native_scope: Option<Scope>,
    wake: Option<Arc<dyn CompletionWake>>,
    claims: WindowsResourceClaims,
    registry: Option<WindowsProcessRegistry>,
    registry_creation: Option<crate::process_fixture::registry::PendingRegistry>,
    registry_binding_pending: bool,
    key: Option<CoordinatorKey>,
    resolution: Option<ResolutionTicket>,
    resource: Option<ResourceKey>,
    claims_pending: bool,
    claims_closed: bool,
    setup: Option<OwnedJob<(), std::io::Error>>,
    setup_notice: bool,
    setup_finished: bool,
    managed: Option<Family>,
    child: Option<ChildCase>,
    results: Vec<serde_json::Value>,
    next_job: u64,
    next_scope: u64,
    phase: Phase,
    closing: bool,
    initialization_admitted: bool,
    execution_stopping: bool,
    custody_expired: bool,
    workers: Option<WorkerStatus>,
    finished: bool,
    premature_release_rejected: bool,
    managed_released: bool,
    root_exit_before_release: bool,
}
impl Fixture {
    /// Retain one actual finite table constructor on the existing cleanup-capable scope.
    fn begin_registry(&mut self, key: CoordinatorKey) -> Result<()> {
        let specification = self.spec(
            JobClass::InfrastructureLifecycle,
            sugarsubstitute_supervision::WindowsProcessRegistry::required_reservation_bytes(),
            !self.closing,
        )?;
        self.registry_creation = Some(crate::process_fixture::registry::PendingRegistry::submit(
            self.client.as_ref().ok_or("Registry client missing")?,
            self.cleanup_scope.as_ref().ok_or("Registry cleanup scope missing")?,
            specification,
            key,
        )?);
        Ok(())
    }

    fn new(options: CampaignOptions) -> Result<Self> {
        let clock = campaign_watch::execution_clock();
        let (custody, execution_deadline) = campaign_watch::custody(clock.as_ref())?;
        let child_deadline = FixtureDeadline::until(custody)?;
        let mut results = Vec::new();
        results.try_reserve_exact(7)?;
        Ok(Self {
            options,
            clock,
            watch: CampaignWatch::new("Claim campaign"),
            execution_deadline,
            child_deadline,
            owner: None,
            client: None,
            scope: None,
            cleanup_scope: None,
            native_scope: None,
            wake: None,
            claims: WindowsResourceClaims::new()?,
            registry: None,
            registry_creation: None,
            registry_binding_pending: false,
            key: None,
            resolution: None,
            resource: None,
            claims_pending: false,
            claims_closed: false,
            setup: None,
            setup_notice: false,
            setup_finished: false,
            managed: None,
            child: None,
            results,
            next_job: 1,
            next_scope: 4,
            phase: Phase::Setup,
            closing: false,
            initialization_admitted: false,
            execution_stopping: false,
            custody_expired: false,
            workers: None,
            finished: false,
            premature_release_rejected: false,
            managed_released: false,
            root_exit_before_release: false,
        })
    }
    fn spec(&mut self, class: JobClass, bytes: u64, deadline: bool) -> Result<JobSpec> {
        super::specification::next(
            &mut self.next_job,
            class,
            bytes,
            deadline.then_some(self.execution_deadline),
        )
    }
    fn family_scope(&mut self) -> Result<ScopeSpec> {
        let id = ScopeId::new(self.next_scope)?;
        self.next_scope =
            self.next_scope.checked_add(1).ok_or("Claim campaign scope identity exhausted")?;
        Ok(ScopeSpec { id, deadline: None })
    }
    fn notify(&self) -> Result<()> {
        if let Some(wake) = &self.wake {
            wake.wake()?;
        }
        Ok(())
    }
    fn configuration(&self) -> Result<LaunchConfiguration> {
        Ok(LaunchConfiguration::new(
            self.options.workspace.join("backend"),
            std::env::current_exe()?,
            vec!["--internal-native-exit-fixture".into(), "37".into()],
            8188,
        )?)
    }
    fn fallback_registry(&mut self) -> Result<()> {
        if self.registry.is_none() && self.registry_creation.is_none() {
            // No acquired coordinator means no family could have been launched.
            // This empty cleanup object is matching data, never authentication.
            self.begin_registry(CoordinatorKey {
                namespace: NamespaceId::new([0; 32]),
                epoch: CoordinatorEpoch::new([1; 16])?,
            })?;
        }
        Ok(())
    }
}
