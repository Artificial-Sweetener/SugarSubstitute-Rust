// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain real claims before causal Ready and through normal checked child teardown.
mod activity;
mod lifetime;
use super::{
    Result, failure,
    options::{ChildOptions, Role, hex_text},
    receipt::{ChildReceipt, ChildResult},
};
use std::{num::NonZeroUsize, sync::Arc};
use sugarsubstitute_contracts::{
    configuration::LaunchConfiguration,
    execution::{
        Clock, CompletionWake, ExecutionPort, JobClass, JobId, JobSpec, Moment, Operation,
        OwnedJob, Scope, ScopeId, ScopeSpec,
    },
    lifecycle::{CoordinatorKey, ResourceGeneration, ResourceId, ResourceKey},
};
use sugarsubstitute_desktop::{HeadlessLifetime, HeadlessStep, run_headless};
use sugarsubstitute_execution::{
    ExecutionClient, ExecutionOwner, MonotonicClock, WorkerStatus,
    windows::NativeObservationError,
    windows_fixture::{FixtureChildPermit, FixtureChildReport, MIN_FIXTURE_SESSION_BYTES},
};
use sugarsubstitute_supervision::{
    CLAIM_OPERATION_BYTES, CLAIM_POOL_BYTES, ClaimAdmissionError, ClaimObservation, ClaimTarget,
    ResolutionTicket, WindowsProcessRegistry, WindowsResourceClaims,
};

/// Run an isolated inherited child on its native main thread, then emit diagnostics.
/// Ready means this exact role already observed its required native claim fact.
pub(crate) fn run_child(options: ChildOptions) -> Result<ChildResult> {
    let mut child = Child::new(options)?;
    run_headless(&mut child)?;
    let report = child.protocol_report.ok_or("Claim child lacks actual event retirement")?;
    let workers = child.workers.ok_or("Claim child lacks actual worker retirement")?;
    let deadline = child.deadline.ok_or("Claim child lacks original admitted deadline")?;
    let reserved = child.scope.as_ref().map_or(0, |scope| scope.snapshot().reserved_bytes)
        + child.cleanup_scope.as_ref().map_or(0, |scope| scope.snapshot().reserved_bytes)
        + child.native_scope.as_ref().map_or(0, |scope| scope.snapshot().reserved_bytes);
    let receipt = ChildReceipt {
        schema_version: 1,
        role: child.options.role.wire().into(),
        correlation: child.options.correlation.get(),
        original_deadline_qpc: child.options.deadline.ticks(),
        qpc_frequency: child.options.deadline.frequency(),
        original_execution_deadline_nanoseconds: deadline.as_nanos(),
        namespace: child.key.map(|key| hex_text(key.namespace.bytes())),
        epoch: child.key.map(|key| hex_text(key.epoch.bytes())),
        actual_failure_operation: child.expected_failure.map(str::to_owned),
        backend_acquired: child.backend_acquired,
        backend_released: child.backend_released,
        claims_closed: child.claims_closed,
        inheritance_cleared: report.inheritance_cleared,
        ready_signalled: report.ready_signalled,
        response_signalled: report.response_signalled,
        release_observed: report.release_observed,
        async_finished: workers.async_finished,
        cpu_finished: workers.cpu_finished,
        native_finished: workers.native_finished,
        reserved_bytes_after_teardown: reserved,
        native_teardown_finished: child.finished,
    };
    receipt.validate(
        child.options.role,
        child.options.correlation.get(),
        child.options.deadline.ticks(),
        child.options.deadline.frequency(),
    )?;
    Ok(ChildResult {
        exit_code: 37,
        path: child.options.receipt,
        receipt: serde_json::to_value(receipt)?,
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Registry,
    Initializing,
    Resolving,
    Acquiring,
    Releasing,
    Closing,
    Protocol,
}
struct Child {
    options: ChildOptions,
    clock: Arc<MonotonicClock>,
    owner: Option<ExecutionOwner>,
    client: Option<ExecutionClient>,
    scope: Option<Scope>,
    cleanup_scope: Option<Scope>,
    native_scope: Option<Scope>,
    claims: WindowsResourceClaims,
    registry: Option<WindowsProcessRegistry>,
    registry_creation: Option<crate::process_fixture::registry::PendingRegistry>,
    registry_binding_pending: bool,
    key: Option<CoordinatorKey>,
    resolution: Option<ResolutionTicket>,
    resource: Option<ResourceKey>,
    permit: Option<FixtureChildPermit>,
    protocol: Option<OwnedJob<FixtureChildReport, NativeObservationError>>,
    protocol_report: Option<FixtureChildReport>,
    protocol_notice: bool,
    deadline: Option<Moment>,
    native_deadline: Option<std::time::Instant>,
    next_job: u64,
    phase: Phase,
    pending: bool,
    expected_failure: Option<&'static str>,
    backend_acquired: bool,
    backend_released: bool,
    initialization_admitted: bool,
    claims_closed: bool,
    closing: bool,
    execution_stopping: bool,
    deadline_reported: bool,
    wake: Option<Arc<dyn CompletionWake>>,
    workers: Option<WorkerStatus>,
    finished: bool,
}
impl Child {
    /// Retain one actual finite table constructor on the existing cleanup-capable scope.
    fn begin_registry(&mut self, key: CoordinatorKey) -> Result<()> {
        let specification = self.spec(
            sugarsubstitute_supervision::WindowsProcessRegistry::required_reservation_bytes(),
            self.closing,
        )?;
        self.registry_creation = Some(crate::process_fixture::registry::PendingRegistry::submit(
            self.client.as_ref().ok_or("Registry client missing")?,
            self.cleanup_scope.as_ref().ok_or("Registry cleanup scope missing")?,
            specification,
            key,
        )?);
        Ok(())
    }

    fn new(options: ChildOptions) -> Result<Self> {
        Ok(Self {
            options,
            clock: crate::campaign_watch::execution_clock(),
            owner: None,
            client: None,
            scope: None,
            cleanup_scope: None,
            native_scope: None,
            claims: WindowsResourceClaims::new()?,
            registry: None,
            registry_creation: None,
            registry_binding_pending: false,
            key: None,
            resolution: None,
            resource: None,
            permit: None,
            protocol: None,
            protocol_report: None,
            protocol_notice: false,
            deadline: None,
            native_deadline: None,
            next_job: 2,
            phase: Phase::Initializing,
            pending: false,
            expected_failure: None,
            backend_acquired: false,
            backend_released: false,
            initialization_admitted: false,
            claims_closed: false,
            closing: false,
            execution_stopping: false,
            deadline_reported: false,
            wake: None,
            workers: None,
            finished: false,
        })
    }
    fn spec(&mut self, bytes: u64, cleanup: bool) -> Result<JobSpec> {
        let id = JobId::new(self.next_job)?;
        self.next_job = self.next_job.checked_add(1).ok_or("Claim child job identity exhausted")?;
        Ok(JobSpec {
            id,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: if cleanup { None } else { self.deadline },
            reservation_bytes: bytes,
            replacement: None,
        })
    }
    fn notify(&self) -> Result<()> {
        if let Some(wake) = &self.wake {
            wake.wake()?;
        }
        Ok(())
    }
    fn configuration(&self) -> Result<LaunchConfiguration> {
        Ok(LaunchConfiguration::new(
            self.options.backend.clone(),
            std::env::current_exe()?,
            vec!["--internal-native-exit-fixture".into(), "37".into()],
            8188,
        )?)
    }
    fn cleanup_context(&mut self) -> Result<()> {
        if self.registry.is_none() && self.registry_creation.is_none() {
            // This actual parent key supplies only an empty cleanup object when
            // initialization never acquired an owner. Claims does not bind it,
            // seal it or interpret this input as authentication/settlement proof.
            self.begin_registry(self.options.parent)?;
        }
        Ok(())
    }
}
