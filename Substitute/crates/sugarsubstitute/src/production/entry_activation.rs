// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose actual authenticated requests, trusted capture and retained desktop starts.
#[path = "entry_activation/attachment_completion.rs"]
mod attachment_completion;
#[path = "entry_activation/backend_failure.rs"]
mod backend_failure;
#[path = "entry_activation/capture.rs"]
mod capture;
#[path = "entry_activation/cause_transfer.rs"]
mod cause_transfer;
#[path = "entry_activation/component.rs"]
mod component;
pub(crate) use component::ProfileComponent;
#[path = "entry_activation/delivery.rs"]
mod delivery;
#[path = "entry_activation/desktop_output.rs"]
mod desktop_output;
#[path = "entry_activation/desktop_role.rs"]
pub(crate) mod desktop_role;
#[path = "entry_activation/failure.rs"]
mod failure;
#[path = "entry_activation/forwarding.rs"]
pub(in crate::production) mod forwarding;
#[path = "entry_activation/funding.rs"]
pub(in crate::production) mod funding;
#[path = "entry_activation/incident_collection.rs"]
mod incident_collection;
#[path = "entry_activation/incident_flow.rs"]
mod incident_flow;
#[path = "entry_activation/incident_reporting.rs"]
mod incident_reporting;
#[path = "entry_activation/incident_seed.rs"]
mod incident_seed;
pub(in crate::production) use incident_reporting::RetiredReporter;
#[path = "entry_activation/launch.rs"]
mod launch;
#[path = "entry_activation/normal_idle.rs"]
mod normal_idle;
#[path = "entry_activation/normal_retirement.rs"]
mod normal_retirement;
#[path = "entry_activation/parent_construction.rs"]
mod parent_construction;
#[path = "entry_activation/profile_admission.rs"]
mod profile_admission;
#[path = "entry_activation/progress.rs"]
mod progress;
#[cfg(test)]
#[path = "entry_activation/progress_deadline_tests.rs"]
mod progress_deadline_tests;
#[path = "entry_activation/projection.rs"]
mod projection;
#[path = "entry_activation/reactivation.rs"]
mod reactivation;
#[path = "entry_activation/record.rs"]
mod record;
#[path = "entry_activation/record_progress.rs"]
mod record_progress;
#[path = "entry_activation/recovery_supervisor.rs"]
mod recovery_supervisor;
#[path = "entry_activation/relay.rs"]
mod relay;
#[path = "entry_activation/restart.rs"]
mod restart;
#[path = "entry_activation/retirement.rs"]
pub(crate) mod retirement;
#[path = "entry_activation/shutdown.rs"]
mod shutdown;
#[path = "entry_activation/source_preparation.rs"]
mod source_preparation;
#[cfg(test)]
#[path = "entry_activation/tests.rs"]
mod tests;
use super::{
    coordinator_backend::{
        BackendPreparation, BackendResources, CoordinatorBackend, RetainedBackend,
    },
    entry_runner::{EntryActivation, EntryResources},
    parent_lifecycle::{ParentLifecycle, ParentResources},
};
pub(in crate::production) use capture::CaptureRegistration;
use launch::DesktopStart;
use projection::{ProjectedEntry, ProjectionFailure, ProjectionInput};
use std::{error::Error, path::PathBuf, time::Instant};
use sugarsubstitute_contracts::{
    configuration::LaunchConfiguration,
    coordinator_entry::{EntryOperationId, EntryOutcome},
    execution::{
        CompletionBatch, Generation, JobClass, JobCompletion, JobOutcome, JobSpec, MemoryClaim,
        MemoryLease, Operation, OwnedJob,
    },
    lifecycle::{ConnectionId, InstanceId, ResourceKey, RunGeneration, RunKey, ShutdownBudget},
    startup::LaunchId,
};
use sugarsubstitute_desktop::HeadlessStep;
use sugarsubstitute_execution::{
    AuthenticatedEntryLaunch,
    windows_capture_attach::{
        CaptureAttachFailure, CaptureAttachLaunchPin, CaptureAttachRetirement,
        CaptureAttachRetirementReport,
    },
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
fn outer(cause: Box<dyn Error + Send + Sync>) -> Box<dyn Error> {
    cause
}
use record::{PendingProjection, Record, RetainedRecord};
/// Own bounded retained records and the process's one actual capture registration.
/// No constructor grants native effects, trust, serving state or child readiness.
pub(crate) struct ProductionActivation {
    selected_locale: Option<sugarsubstitute_contracts::configuration::LocaleId>,
    #[cfg(any(test, feature = "k1-native-qualification"))]
    idle_gate: std::cell::Cell<Option<normal_idle::qualification::IdleGate>>,
    incidents: incident_flow::IncidentFlow,
    qualification: Option<super::qualification::QualificationIntent>,
    executable: PathBuf,
    working_directory: PathBuf,
    capture_root: PathBuf,
    capture: Option<CaptureRegistration>,
    delivery: Option<delivery::TransitionDelivery>,
    records: [Option<RetainedRecord>; 64],
    pending: [Option<PendingProjection>; 64],
    next_instance: u64,
    next_connection: u64,
    next_recovery_grant: u64,
    recovery_pending: Option<recovery_supervisor::RetainedPreparation>,
    failed_recoveries: [Option<recovery_supervisor::RetainedPreparation>; 64],
    recovery_denial: Option<Box<dyn Error>>,
    closing: bool,
    shutdown: Option<ShutdownBudget>,
    failure: Option<Box<dyn Error>>,
    failure_run: Option<RunKey>,
    failure_resource: Option<ResourceKey>,
    next_wake: Option<Instant>,
    // Policy credit outlives dependent Record/Forwarded/delivery allocations.
    backend: Option<RetainedBackend>,
}
impl ProductionActivation {
    /// Return the unique heap owner before native pumping; locations remain inert until native validation.
    pub(crate) fn new(
        executable: PathBuf,
        working_directory: PathBuf,
        capture_root: PathBuf,
    ) -> Result<Box<Self>> {
        for path in [&executable, &working_directory, &capture_root] {
            LaunchConfiguration::validate_path(path)?;
        }
        Ok(Box::new(Self {
            #[cfg(any(test, feature = "k1-native-qualification"))]
            idle_gate: std::cell::Cell::new(None),
            selected_locale: None,
            incidents: incident_flow::IncidentFlow::new(),
            qualification: None,
            executable,
            working_directory,
            capture_root,
            capture: None,
            delivery: None,
            records: [const { None }; 64],
            pending: [const { None }; 64],
            next_instance: 0,
            next_connection: 0,
            next_recovery_grant: 0,
            recovery_pending: None,
            failed_recoveries: [const { None }; 64],
            recovery_denial: None,
            closing: false,
            shutdown: None,
            failure: None,
            failure_run: None,
            failure_resource: None,
            next_wake: None,
            backend: None,
        }))
    }
    /// Assign inert development intent only to the first original admitted launch.
    pub(crate) fn with_qualification(
        mut self: Box<Self>,
        intent: super::qualification::QualificationIntent,
    ) -> Box<Self> {
        self.qualification = Some(intent);
        self
    }
    fn preserve(&mut self, cause: Box<dyn Error>) {
        if self.failure.is_none() {
            self.failure = Some(cause);
        }
    }
}
impl EntryActivation for ProductionActivation {
    fn presentation_locale(&self) -> Option<sugarsubstitute_contracts::configuration::LocaleId> {
        self.records
            .iter()
            .flatten()
            .filter(|held| Some(held.record.run) == self.failure_run)
            .find_map(|held| {
                let projection = held
                    .record
                    .projected
                    .as_ref()
                    .or_else(|| held.record.start.as_ref().map(|start| &start.projection))?;
                match &projection.outcome {
                    JobOutcome::Success(value) => Some(value.locale),
                    _ => None,
                }
            })
            .or(self.selected_locale)
    }
    fn enter_activation(
        &mut self,
        request: &mut Option<AuthenticatedEntryLaunch>,
        mut resources: EntryResources<'_>,
    ) -> Result<()> {
        if self.closing {
            return Err("Production activation admission closed".into());
        }
        if resources.finite.owner_id() != resources.native.owner_id() {
            return Err("Entry source and persistent execution owner differ".into());
        }
        let original = request.as_ref().ok_or("Original authenticated entry missing")?;
        let owner = resources.authority.key().ok_or("Actual coordinator authority missing")?;
        resources.entry.validate_current_owner(owner, original.peer())?;
        original.view()?;
        let until = original.until();
        if resources.client.now()? >= until {
            return Err("Original entry deadline expired".into());
        }
        self.incidents.prepare(&mut resources)?;
        let slot = self
            .records
            .iter()
            .enumerate()
            .find_map(|(at, record)| (record.is_none() && self.pending[at].is_none()).then_some(at))
            .ok_or("Desktop retained quota occupied")?;
        if self.capture.is_none() {
            self.capture = Some(CaptureRegistration::new(owner));
        }
        let next_instance =
            self.next_instance.checked_add(1).ok_or("Desktop instance identity exhausted")?;
        let next_connection =
            self.next_connection.checked_add(1).ok_or("Desktop connection identity exhausted")?;
        let run = RunKey {
            owner,
            instance: InstanceId::new(next_instance)?,
            generation: RunGeneration::new(1)?,
        };
        let connection = ConnectionId::new(next_connection)?;
        let launch = LaunchId::new(1)?;
        let navigation = Generation::new(1)?;
        let installation = resources.authority.running_installation();
        if installation.is_some() && original.view()?.originals().is_some() {
            return Err("Installed entry cannot adopt a source-host originals reference".into());
        }
        let source_admission = if installation.is_none() {
            Some(resources.entry.source_data_admission(owner, original)?)
        } else {
            None
        };
        let spec = JobSpec {
            id: resources.jobs.allocate()?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(until),
            reservation_bytes: projection::PROJECTION_BYTES,
            replacement: None,
        };
        let frame = request.take().ok_or("Original authenticated entry missing")?;
        let mut input = Some(ProjectionInput::new(
            frame,
            self.capture_root.clone(),
            sugarsubstitute_configuration::layout::InstallationLayout::for_image_directory(
                &self.working_directory,
            )
            .user(),
            resources.client.clone(),
            run,
            installation,
        ));
        let (job, source) = if let Some(admission) = source_admission {
            let mut preparation = source_preparation::Preparation::new(
                input.take().ok_or("Original source input missing")?,
                self.working_directory.clone(),
                admission,
            );
            if let Err(cause) = preparation.start(&resources, until) {
                *request = preparation.rejected_request();
                return Err(cause);
            }
            (None, Some(preparation))
        } else {
            match resources.client.submit_blocking_owned_box(
                resources.finite,
                spec,
                &mut input,
                projection::project,
            ) {
                Ok(job) => (Some(job), None),
                Err(cause) => {
                    *request = input.take().and_then(ProjectionInput::rejected_request);
                    return Err(Box::new(cause));
                }
            }
        };
        self.next_instance = next_instance;
        self.next_connection = next_connection;
        self.pending[slot] = Some(PendingProjection {
            qualification: self.qualification.take().unwrap_or_default(),
            run,
            launch,
            navigation,
            connection,
            job,
            source,
            completion: None,
            restart: false,
        });
        Ok(())
    }
    fn consume(
        &mut self,
        batch: &CompletionBatch,
        mut resources: EntryResources<'_>,
    ) -> Result<HeadlessStep> {
        if let Err(cause) = self.activity(batch, &mut resources) {
            tracing::error!(
                operation = "production_activation_transition_failed",
                cause_chain = %crate::diagnostics::CauseChain(cause.as_ref()),
                "Actual activation turn failed before original cause retention"
            );
            self.preserve(cause);
            return Err("Actual production activation failed with retained source".into());
        }
        Ok(if self.closing { HeadlessStep::Shutdown } else { HeadlessStep::Active })
    }
    fn request_close(&mut self, budget: Option<ShutdownBudget>) -> Result<()> {
        self.closing = true;
        if let Some(budget) = budget {
            self.shutdown = Some(self.shutdown.map_or(budget, |old| ShutdownBudget {
                started: old.started.min(budget.started),
                graceful_until: old.graceful_until.min(budget.graceful_until),
                settle_until: old.settle_until.min(budget.settle_until),
            }));
            if !self.records.iter().flatten().any(|held| held.record.collection.pending()) {
                self.incidents
                    .request_close(self.shutdown.ok_or("Original activation cutoff missing")?);
            }
        }
        for pending in self.pending.iter().flatten() {
            if let Some(source) = &pending.source {
                source.cancel();
            }
            if let Some(job) = &pending.job {
                job.cancel();
            }
        }
        for record in self.records.iter_mut().flatten().map(|retained| retained.record.as_mut()) {
            record.seed.cancel();
            if !record.collection.pending() {
                record.collection.cancel_preparation();
            }
            if let Some(job) = &record.attach {
                job.cancel();
            }
        }
        Ok(())
    }
    fn normal_close_requested(&self) -> bool {
        self.normal_close_budget().is_some()
    }
    fn publish_update_status(
        &mut self,
        revision: u64,
        status: &sugarsubstitute_contracts::update::status::UpdateStatus,
    ) -> Result<()> {
        for retained in self.records.iter_mut().flatten() {
            if retained.record.retired.is_none()
                && let Some(parent) = retained.record.parent.as_deref_mut()
            {
                parent.publish_update_status(revision, status).map_err(outer)?;
            }
        }
        Ok(())
    }
    fn restart_to_apply_requested(&self) -> bool {
        self.records.iter().flatten().any(|retained| {
            retained
                .record
                .completed_normal
                .as_ref()
                .is_some_and(normal_retirement::CompletedNormal::restart_to_apply)
        })
    }
    fn shutdown_budget(&self) -> Option<ShutdownBudget> {
        self.records
            .iter()
            .flatten()
            .filter(|retained| retained.record.retired.is_none())
            .filter_map(|retained| {
                retained.record.parent.as_ref().and_then(|parent| parent.shutdown())
            })
            .chain(self.normal_close_budget())
            .fold(self.shutdown, |current, next| {
                Some(current.map_or(next, |current| ShutdownBudget {
                    started: current.started.min(next.started),
                    graceful_until: current.graceful_until.min(next.graceful_until),
                    settle_until: current.settle_until.min(next.settle_until),
                }))
            })
    }
    fn failure_source(&self) -> Option<&(dyn Error + 'static)> {
        ProductionActivation::failure_source(self)
    }
    fn desktop_close_intent(
        &self,
    ) -> Option<sugarsubstitute_contracts::lifecycle::CompletionIntent> {
        use sugarsubstitute_contracts::lifecycle::CompletionIntent;
        self.records.iter().flatten().find_map(|held| {
            let record = &held.record;
            if record.completed_normal.is_some() {
                return Some(CompletionIntent::Normal);
            }
            record.parent.as_ref()?.close_progress_request().map(|request| request.intent())
        })
    }
    fn failed_process(&self) -> sugarsubstitute_contracts::installation::FailedProcess {
        use sugarsubstitute_contracts::installation::FailedProcess;
        if self.failure_resource.is_some() { FailedProcess::Backend } else { FailedProcess::Host }
    }
    fn next_deadline(&self) -> Option<Instant> {
        self.next_wake
    }
    fn poll_close(
        &mut self,
        batch: &CompletionBatch,
        resources: Option<EntryResources<'_>>,
        budget: ShutdownBudget,
    ) -> Result<bool> {
        self.request_close(Some(budget))?;
        let Some(mut resources) = resources else {
            return Ok(self.records.iter().all(Option::is_none)
                && self.pending.iter().flatten().all(PendingProjection::source_terminal)
                && self.recovery_pending.is_none()
                && self.capture.is_none());
        };
        if let Err(cause) = self.activity(batch, &mut resources) {
            self.preserve(cause);
        }
        // Incident cleanup owns finite jobs independently of Reporter and desktop
        // settlement; attempt it while original cleanup admission is still open.
        let incident_close = self.incidents.close_seed(&resources, budget);
        let records_close = self.close_records(&mut resources);
        let incident_closed = match (incident_close, records_close) {
            (Err(cause), Err(later)) => {
                self.preserve(later);
                return Err(cause);
            }
            (Err(cause), Ok(())) | (Ok(_), Err(cause)) => return Err(cause),
            (Ok(closed), Ok(())) => closed,
        };
        Ok(self.recovery_pending.as_ref().is_none_or(|pending| pending.settled())
            && self.records.iter().flatten().all(|retained| retained.record.closed)
            && self.records.iter().flatten().all(|retained| retained.record.seed.delivered())
            && self.records.iter().flatten().all(|retained| retained.record.collection.delivered())
            && self.incidents.reporting.settled()
            && incident_closed
            && self.incidents.diagnostic_transactions_settled()
            && self.pending.iter().flatten().all(PendingProjection::source_terminal)
            && self.delivery.as_ref().is_none_or(|delivery| !delivery.pending())
            && self.backend.as_ref().is_none_or(|retained| retained.backend.settled())
            && self
                .capture
                .as_mut()
                .map(|capture| capture.close(budget, &resources))
                .transpose()?
                .unwrap_or(true))
    }
    fn finish(&mut self) -> Result<()> {
        if !self.closing
            || !self.incidents.diagnostic_transactions_settled()
            || self.records.iter().flatten().any(|retained| !retained.record.closed)
            || self.pending.iter().flatten().any(|pending| !pending.source_terminal())
            || self.recovery_pending.as_ref().is_some_and(|pending| !pending.settled())
        {
            return Err("Actual activation resources remain unsettled".into());
        }
        Ok(())
    }
}
