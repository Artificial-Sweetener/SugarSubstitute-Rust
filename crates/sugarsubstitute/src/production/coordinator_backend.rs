// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose actual backend claims, process custody, installed sources and readiness.
#[path = "coordinator_backend/activity.rs"]
mod activity;
#[path = "coordinator_backend/attempt.rs"]
mod attempt;
#[path = "coordinator_backend/cleanup.rs"]
mod cleanup;
#[cfg(test)]
#[path = "coordinator_backend/cleanup_lifetime_tests.rs"]
mod cleanup_lifetime_tests;
#[path = "coordinator_backend/cleanup_state.rs"]
mod cleanup_state;
#[cfg(test)]
#[path = "coordinator_backend/custody_capture_tests.rs"]
mod custody_capture_tests;
#[cfg(test)]
#[path = "coordinator_backend/cutoff_tests.rs"]
mod cutoff_tests;
#[path = "coordinator_backend/failure.rs"]
mod failure;
use failure::{FailureKind, FailureObservation, OutputTerminalFailure, RootExit, Stopped};
#[path = "coordinator_backend/output.rs"]
mod output;
#[path = "coordinator_backend/preparation.rs"]
mod preparation;
#[cfg(test)]
#[path = "coordinator_backend/tests.rs"]
pub(crate) mod tests;

use super::{coordinator_authority::CoordinatorAuthority, entry_runner::ScopeSequence};
use crate::job_sequence::JobSequence;
use std::error::Error;
use sugarsubstitute_application::{
    startup::{ConnectionBudget, ConnectionBudgetKind},
    supervision::ResourceCoordinator,
};
use sugarsubstitute_comfy::{
    readiness_campaign::{CampaignStep, ReadinessCampaign},
    readiness_transport::{ReadinessProbeInput, VerifiedReadiness},
};
use sugarsubstitute_contracts::{
    configuration::LoopbackOrigin,
    execution::{
        CompletionBatch, ExecutionPort, JobClass, JobCompletion, JobOutcome, JobSpec, MemoryClaim,
        Moment, Operation, OwnedJob, Scope, ScopeSpec, TerminalKind,
    },
    lifecycle::*,
};
use sugarsubstitute_execution::{
    ExecutionClient,
    windows_output::{
        AdmittedOutputBatch, MIN_WINDOWS_OUTPUT_BYTES, WindowsOutputReport, WindowsOutputSession,
    },
};
use sugarsubstitute_supervision::{
    CLAIM_OPERATION_BYTES, ClaimLaunchJobs, ClaimObservation, ClaimTarget, LaunchEnvironment,
    LaunchTicket, RegistryObservation, ResolutionTicket,
    installed_frontend::{
        FRONTEND_SOURCE_BYTES, FrontendInstallationInput, FrontendSourceError,
        InstalledFrontendLease, InstalledFrontendOwner,
    },
};

pub(super) type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// Borrow the sole execution/identity sequences and the actual admitted authority.
/// Native and cleanup scopes outlive finite preparation and every retained output.
pub(crate) struct BackendResources<'a> {
    pub(crate) client: &'a ExecutionClient,
    pub(crate) native: &'a Scope,
    pub(crate) cleanup: &'a Scope,
    pub(crate) jobs: &'a JobSequence,
    pub(crate) scopes: &'a ScopeSequence,
    pub(crate) authority: &'a mut CoordinatorAuthority,
    pub(crate) capture: Option<&'a super::entry_activation::CaptureRegistration>,
}

/// Retain trusted explicit installed paths and the original captured environment.
/// Managed environment comes from the real entry producer, never ambient discovery.
pub(crate) struct BackendPreparation {
    pub(crate) source: FrontendInstallationInput,
    pub(crate) environment: Option<LaunchEnvironment>,
}

struct Backend {
    key: ResourceKey,
    target: BackendTarget,
    budget: ConnectionBudget,
    resolved: Option<sugarsubstitute_contracts::configuration::LaunchConfiguration>,
    ready_origin: Option<LoopbackOrigin>,
    scope: Scope,
    source: InstalledFrontendOwner,
    source_job: bool,
    source_lease: Option<InstalledFrontendLease>,
    source_close: Option<OwnedJob<(), FrontendSourceError>>,
    source_terminal: Option<JobCompletion<(), FrontendSourceError>>,
    source_closed: bool,
    environment: Option<LaunchEnvironment>,
    resolution: Option<ResolutionTicket>,
    resolution_pending: bool,
    resolution_retiring: bool,
    claim_pending: bool,
    backend_admitted: bool,
    claimed: bool,
    release_pending: bool,
    released: bool,
    launch: Option<LaunchTicket>,
    started: bool,
    family_settled: bool,
    output: Option<WindowsOutputSession>,
    output_terminal: Option<
        JobCompletion<
            WindowsOutputReport,
            sugarsubstitute_execution::windows::NativeObservationError,
        >,
    >,
    output_observed: bool,
    campaign: ReadinessCampaign,
    readiness: Option<VerifiedReadiness>,
    shutdown: Option<(ShutdownId, ShutdownBudget)>,
    failure_published: bool,
    settled: bool,
    cleanup_failure: cleanup_state::CleanupState,
}

/// Own at most64 actual backend generations; attached processes never enter registry.
/// The composing headless thread routes each immutable completion batch after the
/// authority consumes its actual claims and registry jobs. This owner spawns nothing.
pub(crate) struct CoordinatorBackend {
    custody_options: Option<(std::path::PathBuf, std::path::PathBuf)>,
    custody: Option<Box<super::coordinator_custody::CoordinatorCustody>>,
    owner: CoordinatorKey,
    policy: ResourceCoordinator,
    records: Box<[Option<Backend>]>,
    next_shutdown: u64,
    undelivered: Option<ResourceTransition>,
}
/// Destroy the complete table and metadata allocations before returning their
/// single original projection charge. No bare funded backend escapes construction.
pub(crate) struct RetainedBackend {
    pub(crate) backend: Box<CoordinatorBackend>,
    _storage: MemoryClaim,
}
impl RetainedBackend {
    /// Cover the retained table and one MiB of bounded witness invocation metadata
    /// inside the unchanged original sixteen-MiB projection reservation.
    pub(crate) const fn required_storage_bytes() -> u64 {
        (size_of::<CoordinatorBackend>() + 64 * size_of::<Option<Backend>>() + 1_048_576) as u64
    }
    /// Construct only from an actually serving claims/registry association.
    /// The caller preclaims the table and heap header from its unchanged original
    /// projection reservation. Both allocations remain inseparable from that claim.
    pub(crate) fn new(authority: &CoordinatorAuthority, storage: MemoryClaim) -> Result<Self> {
        if !authority.serving() || authority.registry().is_none() {
            return Err(Box::new(LifecycleRejection::WrongPhase));
        }
        let owner = authority.key().ok_or(LifecycleRejection::ForeignOwner)?;
        if storage.bytes() < Self::required_storage_bytes() {
            return Err(Box::new(LifecycleRejection::Overloaded));
        }
        let mut records = Vec::new();
        records.try_reserve_exact(64)?;
        records.resize_with(64, || None);
        let backend = CoordinatorBackend {
            custody_options: None,
            custody: None,
            owner,
            policy: ResourceCoordinator::new(owner),
            records: records.into_boxed_slice(),
            next_shutdown: 0,
            undelivered: None,
        };
        Ok(Self { backend: Box::new(backend), _storage: storage })
    }
}
impl CoordinatorBackend {
    /// Borrow the same authoritative run/resource policy for incident enrollment and admission.
    pub(in crate::production) fn incident_policy(&mut self) -> &mut ResourceCoordinator {
        &mut self.policy
    }
    /// Observe the actual established original helper without invoking another launch.
    pub(crate) fn profile_custody_parent(
        &self,
    ) -> Option<sugarsubstitute_supervision::BackendCustodyParent> {
        self.custody.as_ref().and_then(|custody| custody.established_parent())
    }
    /// Establish the one original helper before any desktop profile Held publication.
    pub(crate) fn ensure_profile_custody(
        &mut self,
        environment: &LaunchEnvironment,
        until: Moment,
        resources: &mut BackendResources<'_>,
    ) -> Result<Option<sugarsubstitute_supervision::BackendCustodyParent>> {
        if self.custody.is_none() {
            let (executable, installation) =
                self.custody_options.as_ref().ok_or("Custody invocation unconfigured")?;
            self.custody = Some(Box::new(super::coordinator_custody::CoordinatorCustody::new(
                self.owner,
                executable.clone(),
                installation.clone(),
                LaunchEnvironment::from_snapshot_utf16(environment.as_utf16(), &[])?,
                until,
                resources,
            )?));
        }
        let custody = self.custody.as_mut().ok_or("Original custody owner absent")?;
        if !custody.advance(resources)? {
            return Ok(None);
        }
        Ok(custody.established_parent())
    }
    /// Bind installed executable/namespace paths before the first managed request.
    pub(crate) fn configure_custody(
        &mut self,
        executable: std::path::PathBuf,
        installation: std::path::PathBuf,
    ) -> Result<()> {
        if self.custody_options.is_some()
            || self.custody.is_some()
            || self.records.iter().any(Option::is_some)
        {
            return Err(Box::new(LifecycleRejection::WrongPhase));
        }
        self.custody_options = Some((executable, installation));
        Ok(())
    }
    /// Register facts authenticated by the existing native child owner.
    pub(crate) fn register_run(&mut self, registration: ClientRegistration) -> Result<()> {
        self.policy.register_run(registration)?;
        Ok(())
    }

    /// Apply a connection-authenticated request and execute only its fresh effects.
    /// Replayed/shared acquisitions leave `input` untouched. Native failures are
    /// retained by policy and produce actual Failed replies plus checked cleanup.
    pub(crate) fn request(
        &mut self,
        binding: ClientBinding,
        request: ResourceRequest,
        original: Option<ConnectionBudget>,
        input: &mut Option<BackendPreparation>,
        resources: &mut BackendResources<'_>,
    ) -> Result<ResourceTransition> {
        self.validate(resources)?;
        Self::check_request_budget(&request.command, original, resources.client.now()?)?;
        if let ResourceCommand::Acquire(target) = &request.command
            && !self.records.iter().flatten().any(|r| r.target == *target && !r.settled)
            && input.is_none()
        {
            return Err(Box::new(LifecycleRejection::WrongPhase));
        }
        self.begin_delivery()?;
        let mut transition = self.policy.request(binding, request)?;
        self.retain_transition(&transition);
        self.effects(&mut transition, input, original, None, resources)?;
        Ok(self.undelivered.take().unwrap_or_default())
    }

    /// Release only this connection's leases; remaining clients keep shared backends.
    pub(crate) fn disconnect(
        &mut self,
        run: RunKey,
        connection: ConnectionId,
        original: ShutdownBudget,
        resources: &mut BackendResources<'_>,
    ) -> Result<ResourceTransition> {
        self.validate(resources)?;
        if ShutdownBudget::new(original.started)? != original {
            return Err(Box::new(LifecycleRejection::WrongPhase));
        }
        self.begin_delivery()?;
        let mut transition = self.policy.disconnect_run(run, connection)?;
        self.retain_transition(&transition);
        self.effects(&mut transition, &mut None, None, Some(original), resources)?;
        Ok(self.undelivered.take().unwrap_or_default())
    }

    /// Check inert timing separately from actual serving/native binding validation.
    fn check_request_budget(
        command: &ResourceCommand,
        original: Option<ConnectionBudget>,
        now: Moment,
    ) -> Result<()> {
        match command {
            ResourceCommand::Acquire(target) => {
                let budget = original.ok_or(LifecycleRejection::WrongPhase)?;
                let expected = match target {
                    BackendTarget::Managed(_) => ConnectionBudgetKind::Managed,
                    BackendTarget::Attached(_) => ConnectionBudgetKind::Attached,
                };
                if budget.kind() != expected {
                    return Err(Box::new(LifecycleRejection::WrongPhase));
                }
                budget.probe_until(now)?;
                Ok(())
            }
            ResourceCommand::Release(_) | ResourceCommand::Reload(_) if original.is_none() => {
                Ok(())
            }
            _ => Err(Box::new(LifecycleRejection::WrongPhase)),
        }
    }

    /// Retire run/profile policy only after the actual desktop owner checked retirement.
    /// Shared resources leased by other clients remain owned. A connected run or
    /// unresolved last-client backend cleanup still rejects this same receipt.
    pub(crate) fn settle_run(
        &mut self,
        retired: &super::entry_activation::retirement::RetiredDesktop,
    ) -> Result<()> {
        if retired.run().owner != self.owner {
            return Err(Box::new(LifecycleRejection::ForeignOwner));
        }
        if retired.registered() {
            self.policy.settle_run(retired.run())?;
        }
        Ok(())
    }

    /// A retained producer return fences another policy mutation before admission.
    /// This is backpressure only, never native settlement or cancellation evidence.
    pub(crate) fn delivery_pending(&self) -> bool {
        self.undelivered.is_some()
    }

    /// Copy the fixed actual record identities for one bounded completion-pump pass.
    pub(crate) fn resource_keys(&self) -> [Option<ResourceKey>; 64] {
        std::array::from_fn(|at| self.records[at].as_ref().filter(|r| !r.settled).map(|r| r.key))
    }

    /// Report complete backend retirement; desktop/profile settlement remains with its owner.
    pub(crate) fn settled(&self) -> bool {
        self.policy.snapshot().resources == 0
            && self.records.iter().flatten().all(|r| r.settled)
            && self.custody.as_ref().is_none_or(|custody| custody.settled())
    }
    /// Progress final witness retirement after local writers and families close.
    /// The Parent transaction owner retains prior release until its exact ACK is consumed
    /// by finish_cleanup; helper process cleanup requires the final Finished response.
    pub(crate) fn close_custody(
        &mut self,
        budget: ShutdownBudget,
        profiles_retired: bool,
        resources: &mut BackendResources<'_>,
    ) -> Result<()> {
        if self.records.iter().flatten().all(|record| {
            !record.resolution_pending
                && !record.claim_pending
                && (!record.backend_admitted || record.released)
                && (record.launch.is_none() || record.family_settled)
                && record.shutdown.is_some()
        }) && let Some(custody) = &mut self.custody
        {
            custody.close(budget, profiles_retired, resources)?;
        }
        Ok(())
    }

    /// Recover actual replies/effects retained before a later adapter rejection.
    /// Drain this after an error before another mutation; effects were owned here.
    pub(crate) fn take_pending_transition(&mut self) -> Option<ResourceTransition> {
        self.undelivered.take()
    }

    /// Borrow current verified readiness; a frontend caller must revalidate before use.
    pub(crate) fn readiness(&self, key: ResourceKey) -> Option<&VerifiedReadiness> {
        self.records.iter().flatten().find(|record| record.key == key)?.readiness.as_ref()
    }

    /// Borrow the origin accepted by actual readiness and the current resource policy.
    /// Revalidates retained installed/listener custody before returning wire data.
    pub(crate) fn ready_origin(&self, key: ResourceKey) -> Result<Option<LoopbackOrigin>> {
        if self.policy.resource_phase(key)
            != Some(sugarsubstitute_application::supervision::ResourcePhase::Ready)
        {
            return Ok(None);
        }
        let record = self.record(key)?;
        let proof = record.readiness.as_ref().ok_or(LifecycleRejection::WrongPhase)?;
        proof.validate_current()?;
        Ok(record.ready_origin)
    }

    /// Borrow cleanup failure without releasing unresolved native ownership or its charge.
    pub(crate) fn cleanup_failure(
        &self,
        key: ResourceKey,
    ) -> Option<&(dyn Error + Send + Sync + 'static)> {
        let record = self.record(key).ok()?;
        if let Some(cause) = record.cleanup_failure.first() {
            return Some(cause);
        }
        record
            .source_terminal
            .as_ref()
            .and_then(|completion| match &completion.outcome {
                JobOutcome::Failure(cause) => Some(cause as &(dyn Error + Send + Sync + 'static)),
                _ => None,
            })
            .or_else(|| {
                record.output_terminal.as_ref().and_then(|completion| match &completion.outcome {
                    JobOutcome::Failure(cause) => {
                        Some(cause as &(dyn Error + Send + Sync + 'static))
                    }
                    _ => None,
                })
            })
    }

    /// Borrow the retained original preparation cause independently of current cleanup.
    pub(crate) fn failure(&self, key: ResourceKey) -> Option<&LifecycleFailure> {
        self.policy.failure(key)
    }

    /// Return the next original campaign/cleanup deadline for the existing native pump.
    pub(crate) fn next_deadline(&self) -> Option<Moment> {
        self.records
            .iter()
            .flatten()
            .filter(|record| !record.settled)
            .filter_map(|record| match record.shutdown {
                Some((_, budget)) => record.cleanup_failure.next_deadline(budget),
                None => record
                    .campaign
                    .next_deadline()
                    .or_else(|| record.readiness.is_none().then_some(record.budget.until())),
            })
            .chain(self.custody.as_ref().and_then(|custody| custody.next_deadline()))
            .min()
    }

    fn validate(&self, resources: &BackendResources<'_>) -> Result<()> {
        if resources.authority.key() != Some(self.owner)
            || !resources.authority.serving()
            || resources.authority.registry().is_none()
        {
            return Err(Box::new(LifecycleRejection::ForeignOwner));
        }
        Ok(())
    }
    fn index(&self, key: ResourceKey) -> Result<usize> {
        self.records
            .iter()
            .position(|record| record.as_ref().is_some_and(|r| r.key == key))
            .ok_or_else(|| Box::new(LifecycleRejection::StaleIdentity) as _)
    }
    fn record(&self, key: ResourceKey) -> Result<&Backend> {
        self.records[self.index(key)?]
            .as_ref()
            .ok_or_else(|| Box::new(LifecycleRejection::StaleIdentity) as _)
    }
    fn spec(
        resources: &BackendResources<'_>,
        operation: Operation,
        class: JobClass,
        bytes: u64,
        until: Option<Moment>,
    ) -> Result<JobSpec> {
        Ok(JobSpec {
            id: resources.jobs.allocate()?,
            operation,
            class,
            deadline: until,
            reservation_bytes: bytes,
            replacement: None,
        })
    }
    fn merge(into: &mut ResourceTransition, mut next: ResourceTransition) {
        into.replies.append(&mut next.replies);
        into.effects.append(&mut next.effects);
    }
    fn observe(&mut self, observation: ResourceObservation) -> Result<ResourceTransition> {
        let transition = self.policy.observe(observation).map_err(|rejected| {
            Box::new(ObservationFailure(rejected)) as Box<dyn Error + Send + Sync>
        })?;
        self.retain_transition(&transition);
        Ok(transition)
    }
    fn begin_delivery(&mut self) -> Result<()> {
        if self.undelivered.is_some() {
            return Err(Box::new(LifecycleRejection::WrongPhase));
        }
        self.undelivered = Some(ResourceTransition::default());
        Ok(())
    }
    fn retain_transition(&mut self, transition: &ResourceTransition) {
        if let Some(pending) = &mut self.undelivered {
            Self::merge(pending, transition.clone());
        }
    }
}

#[derive(Debug)]
struct ObservationFailure(Box<RejectedObservation<ResourceObservation>>);
impl std::fmt::Display for ObservationFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.reason.fmt(f)
    }
}
impl Error for ObservationFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.0.observation {
            ResourceObservation::Failed { failure, .. } => Some(failure),
            _ => Some(&self.0.reason),
        }
    }
}
