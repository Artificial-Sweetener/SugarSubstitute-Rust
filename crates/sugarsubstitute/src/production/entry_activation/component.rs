// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify real profile custody through authenticated entry and the original native pump.
//! The contained child exits without a surface; this proves no browser or Main readiness.
#[path = "component/activity.rs"]
mod activity;
#[cfg(test)]
#[path = "component/caller_tests.rs"]
mod caller_tests;
#[path = "component/retirement.rs"]
mod retirement;
#[cfg(test)]
#[path = "component/tests.rs"]
mod tests;
use super::{CaptureRegistration, Result, projection};
use crate::production::{
    coordinator_custody::CoordinatorCustody,
    entry_runner::{EntryActivation, EntryResources},
};
use std::{error::Error, path::PathBuf, time::Instant};
use sugarsubstitute_contracts::{
    configuration::LaunchConfiguration,
    execution::{
        CompletionBatch, JobClass, JobCompletion, JobOutcome, JobSpec, MemoryClaim, Moment,
        Operation, OwnedJob, ScopeSpec,
    },
    lifecycle::{InstanceId, RunGeneration, RunKey, ShutdownBudget},
};
use sugarsubstitute_desktop::HeadlessStep;
use sugarsubstitute_execution::AuthenticatedEntryLaunch;
use sugarsubstitute_supervision::{LaunchTicket, profiles::WindowsProfileLease};

/// Retain the complete original outcome and independently admitted native owners.
struct Campaign {
    projection: JobCompletion<projection::ProjectedEntry, projection::ProjectionFailure>,
    custody: Option<CoordinatorCustody>,
    profile: Option<WindowsProfileLease>,
    watch: Option<(ScopeSpec, JobSpec)>,
    ticket: Option<LaunchTicket>,
    publication_issued: bool,
    reply_until: Option<Moment>,
    replied: bool,
    started: bool,
    exited: bool,
    registry_retired: bool,
    released: bool,
    _storage: MemoryClaim,
}
impl Campaign {
    fn value(&self) -> Result<&projection::ProjectedEntry> {
        match &self.projection.outcome {
            JobOutcome::Success(value) => Ok(value),
            _ => Err("Profile component retains its unsuccessful projection".into()),
        }
    }
    fn value_mut(&mut self) -> Result<&mut projection::ProjectedEntry> {
        match &mut self.projection.outcome {
            JobOutcome::Success(value) => Ok(value),
            _ => Err("Profile component retains its unsuccessful projection".into()),
        }
    }
}

/// Supply a dedicated qualification child owner to the existing elected entry runner.
/// All native effects use its original scopes, finite jobs, registry and capture owner.
pub(crate) struct ProfileComponent {
    executable: PathBuf,
    installation: PathBuf,
    run: Option<RunKey>,
    projection: Option<OwnedJob<projection::ProjectedEntry, projection::ProjectionFailure>>,
    source: Option<super::source_preparation::Preparation>,
    projected: Option<JobCompletion<projection::ProjectedEntry, projection::ProjectionFailure>>,
    campaign: Option<Box<Campaign>>,
    capture: Option<CaptureRegistration>,
    failure: Option<Box<dyn Error>>,
    shutdown: Option<ShutdownBudget>,
    original_until: Option<Moment>,
    closing: bool,
    complete: bool,
    next_wake: Option<Instant>,
}
impl ProfileComponent {
    /// Freeze one reply probe inside the unchanged original setup ancestor.
    fn reply_probe(now: Moment, original: Moment) -> Result<Moment> {
        let remaining = original.checked_duration_since(now)?;
        Ok(now.checked_add(remaining.min(std::time::Duration::from_secs(2)))?)
    }
    /// Parse the complete existing production entry grammar before qualification effects.
    pub(crate) fn command(
        arguments: impl IntoIterator<Item = std::ffi::OsString>,
    ) -> Result<crate::production::dispatch::ProductionCommand> {
        let mut arguments = arguments.into_iter().peekable();
        if arguments.peek().is_none_or(|argument| argument != "--production") {
            return Err("Profile component requires explicit production arguments".into());
        }
        match crate::production::dispatch::parse(arguments)? {
            crate::production::dispatch::Dispatch::Production(command) => Ok(*command),
            crate::production::dispatch::Dispatch::Existing(_) => {
                Err("Profile component requires the complete production entry grammar".into())
            }
        }
    }
    /// Return one heap child with validated inert locations before authenticated entry begins.
    pub(crate) fn new(executable: PathBuf, installation: PathBuf) -> Result<Box<Self>> {
        LaunchConfiguration::validate_path(&executable)?;
        LaunchConfiguration::validate_path(&installation)?;
        Ok(Box::new(Self {
            executable,
            installation,
            run: None,
            projection: None,
            source: None,
            projected: None,
            campaign: None,
            capture: None,
            failure: None,
            shutdown: None,
            original_until: None,
            closing: false,
            complete: false,
            next_wake: None,
        }))
    }
    /// Require the current profile operation to be consumed before any final caller proceeds.
    fn profile_retirement_ready(&self) -> bool {
        self.campaign
            .as_ref()
            .and_then(|campaign| campaign.profile.as_ref())
            .is_none_or(|profile| profile.snapshot().retirement_ready())
    }
    fn preserve(&mut self, cause: Box<dyn Error>) {
        if self.failure.is_none() {
            self.failure = Some(cause);
        }
    }
    fn spec(
        resources: &EntryResources<'_>,
        class: JobClass,
        bytes: u64,
        until: Option<Moment>,
    ) -> Result<JobSpec> {
        Ok(JobSpec {
            id: resources.jobs.allocate()?,
            operation: Operation::NativeLifecycle,
            class,
            deadline: until,
            reservation_bytes: bytes,
            replacement: None,
        })
    }
    /// Schedule only original absolute transaction boundaries in the existing native pump.
    fn update_wake(&mut self, resources: &EntryResources<'_>) -> Result<()> {
        let now = resources.client.now()?;
        let until = if let Some(budget) = self.shutdown {
            if now >= budget.settle_until && !self.complete {
                return Err("Original component cleanup deadline expired".into());
            }
            if now < budget.graceful_until {
                Some(budget.graceful_until)
            } else if now < budget.settle_until {
                Some(budget.settle_until)
            } else {
                None
            }
        } else if self.campaign.as_ref().is_none_or(|campaign| !campaign.started) {
            if self.original_until.is_some_and(|until| now >= until) {
                return Err("Original component establishment deadline expired".into());
            }
            self.original_until
        } else {
            None
        };
        self.next_wake = until
            .map(|until| {
                Instant::now()
                    .checked_add(until.checked_duration_since(now)?)
                    .ok_or_else(|| Box::<dyn Error>::from("Component native wake overflow"))
            })
            .transpose()?;
        Ok(())
    }
}
impl EntryActivation for ProfileComponent {
    fn presentation_locale(&self) -> Option<sugarsubstitute_contracts::configuration::LocaleId> {
        let projection = self
            .campaign
            .as_ref()
            .map(|campaign| &campaign.projection)
            .or(self.projected.as_ref())?;
        match &projection.outcome {
            JobOutcome::Success(value) => Some(value.locale),
            _ => None,
        }
    }
    fn enter_activation(
        &mut self,
        request: &mut Option<AuthenticatedEntryLaunch>,
        resources: EntryResources<'_>,
    ) -> Result<()> {
        if self.run.is_some() || self.closing {
            return Err("Profile component admits one original entry only".into());
        }
        let original = request.as_ref().ok_or("Authenticated component entry absent")?;
        let owner = resources.authority.key().ok_or("Elected component owner absent")?;
        resources.entry.validate_current_owner(owner, original.peer())?;
        original.view()?;
        let until = original.until();
        if resources.client.now()? >= until {
            return Err("Original component entry expired".into());
        }
        let run =
            RunKey { owner, instance: InstanceId::new(1)?, generation: RunGeneration::new(1)? };
        let installation = resources.authority.running_installation();
        if installation.is_some() && original.view()?.originals().is_some() {
            return Err("Installed component cannot adopt source-host originals".into());
        }
        let admission = if installation.is_none() {
            Some(resources.entry.source_data_admission(owner, original)?)
        } else {
            None
        };
        let spec = Self::spec(
            &resources,
            JobClass::InfrastructureLifecycle,
            projection::PROJECTION_BYTES,
            Some(until),
        )?;
        let source = request.take().ok_or("Original component entry transferred")?;
        let mut input = Some(projection::ProjectionInput::new(
            source,
            sugarsubstitute_configuration::layout::InstallationLayout::for_image_directory(
                &self.installation,
            )
            .capture_root(),
            sugarsubstitute_configuration::layout::InstallationLayout::for_image_directory(
                &self.installation,
            )
            .user(),
            resources.client.clone(),
            run,
            installation,
        ));
        if let Some(admission) = admission {
            let mut preparation = super::source_preparation::Preparation::new(
                input.take().ok_or("Original component source absent")?,
                self.installation.clone(),
                admission,
            );
            if let Err(cause) = preparation.start(&resources, until) {
                *request = preparation.rejected_request();
                return Err(cause);
            }
            self.source = Some(preparation);
        } else {
            match resources.client.submit_blocking_owned_box(
                resources.finite,
                spec,
                &mut input,
                projection::project,
            ) {
                Ok(job) => self.projection = Some(job),
                Err(cause) => {
                    *request = input.take().and_then(projection::ProjectionInput::rejected_request);
                    return Err(Box::new(cause));
                }
            }
        }
        self.run = Some(run);
        self.original_until = Some(until);
        self.capture = Some(CaptureRegistration::new(owner));
        Ok(())
    }
    fn consume(
        &mut self,
        batch: &CompletionBatch,
        mut resources: EntryResources<'_>,
    ) -> Result<HeadlessStep> {
        if let Err(cause) = self.activity(batch, &mut resources) {
            self.preserve(cause);
            return Err("Profile component failed with its retained original cause".into());
        }
        self.update_wake(&resources)?;
        Ok(HeadlessStep::Active)
    }
    fn request_close(&mut self, budget: Option<ShutdownBudget>) -> Result<()> {
        self.closing = true;
        if let Some(budget) = budget {
            self.shutdown = Some(self.shutdown.map_or(budget, |old| ShutdownBudget {
                started: old.started.min(budget.started),
                graceful_until: old.graceful_until.min(budget.graceful_until),
                settle_until: old.settle_until.min(budget.settle_until),
            }));
        }
        if let Some(job) = &self.projection {
            job.cancel();
        }
        if let Some(source) = &self.source {
            source.cancel();
        }
        Ok(())
    }
    fn shutdown_budget(&self) -> Option<ShutdownBudget> {
        self.shutdown
    }
    /// The qualification child has no update surfaces.
    fn publish_update_status(
        &mut self,
        _: u64,
        _: &sugarsubstitute_contracts::update::status::UpdateStatus,
    ) -> Result<()> {
        Ok(())
    }
    /// The qualification child never restarts to apply an update.
    fn restart_to_apply_requested(&self) -> bool {
        false
    }
    fn normal_close_requested(&self) -> bool {
        self.complete && self.failure.is_none() && self.profile_retirement_ready()
    }
    fn failure_source(&self) -> Option<&(dyn Error + 'static)> {
        if let Some(cause) =
            self.source.as_ref().and_then(super::source_preparation::Preparation::failure)
        {
            return Some(cause);
        }
        self.campaign
            .as_ref()
            .and_then(|campaign| match &campaign.projection.outcome {
                JobOutcome::Failure(cause) => Some(cause as &(dyn Error + 'static)),
                _ => None,
            })
            .or_else(|| {
                self.projected.as_ref().and_then(|done| match &done.outcome {
                    JobOutcome::Failure(cause) => Some(cause as &(dyn Error + 'static)),
                    _ => None,
                })
            })
            .or(self.failure.as_deref())
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
            return Ok(self.projection.is_none()
                && self
                    .source
                    .as_ref()
                    .is_none_or(super::source_preparation::Preparation::settled)
                && self.complete
                && self.profile_retirement_ready());
        };
        if let Err(cause) = self.activity(batch, &mut resources) {
            self.preserve(cause);
        }
        self.update_wake(&resources)?;
        self.retire(&mut resources)
    }
    fn finish(&mut self) -> Result<()> {
        if !self.closing
            || !self.complete
            || self.projection.is_some()
            || self.source.as_ref().is_some_and(|source| !source.settled())
            || !self.profile_retirement_ready()
        {
            return Err("Profile component ownership remains unsettled".into());
        }
        tracing::info!(operation="profile_component_finished", run=?self.run, "Real profile component and helper retired through original entry owner");
        Ok(())
    }
}
