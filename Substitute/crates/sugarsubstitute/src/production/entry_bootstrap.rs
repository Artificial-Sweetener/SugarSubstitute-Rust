// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive real configuration, environment, namespace discovery and coordinator election.
//! Original scopes, typed failures and native owner custody survive partial startup.
#[path = "entry_bootstrap/failure.rs"]
mod failure;
#[path = "entry_bootstrap/migration.rs"]
mod migration;
use super::configuration;
use crate::job_sequence::JobSequence;
use std::{error::Error, fmt, path::PathBuf};
use sugarsubstitute_configuration::paths::ConfigurationSource;
use sugarsubstitute_contracts::{
    configuration::{ConfigurationError, LaunchConfiguration},
    coordinator_entry::{EntryOwnerDegradation, EntrySetupBudget},
    execution::{
        CompletionBatch, JobClass, JobCompletion, JobOutcome, JobSpec, Moment, Operation, OwnedJob,
        Scope, TerminalKind,
    },
};
use sugarsubstitute_execution::{
    EntryPreparationFailure, EntryPreparationSuccess, EntryRequestAdoptionRejection,
    EntryRequestInput, ExecutionClient,
};
use sugarsubstitute_supervision::coordinator_entry::{
    CoordinatorEntryInput, CoordinatorEntryOwner, ENTRY_PREPARATION_BYTES, EntryAdmissionError,
    EntryError, EntryInputError, EntryObservation, EntryPhase,
};

type Prepared = EntryPreparationSuccess<EntryRequestInput>;
type PreparationFailure = EntryPreparationFailure<configuration::ConfigurationCause>;
type PreparationJob = OwnedJob<Prepared, PreparationFailure>;
type PreparationCompletion = JobCompletion<Prepared, PreparationFailure>;

/// Report a rejected transition; the complete original failure stays with this owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EntryBootstrapIssue {
    Started,
    Closed,
    Preparation,
    Identity,
    Adoption,
    Input,
    NativeOwner,
    Degraded(EntryOwnerDegradation),
    Stopped(TerminalKind),
    Cleanup,
}
impl fmt::Display for EntryBootstrapIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Production entry bootstrap failed: {self:?}")
    }
}
impl Error for EntryBootstrapIssue {}

enum Failure {
    Submission(Box<dyn Error>),
    Adoption(EntryRequestAdoptionRejection),
    Input(EntryInputError),
    Admission(EntryAdmissionError),
    Native(EntryError),
}
enum CleanupFailure {
    Admission(EntryAdmissionError),
    Native(EntryError),
}

/// Compose the concrete admitted request producer with the real namespace/election owner.
/// Cached phase observations grant no coordinator, backend or browser authority.
pub(crate) struct EntryBootstrap {
    options: Option<ConfigurationSource>,
    installation: Option<PathBuf>,
    diagnostics: Option<sugarsubstitute_supervision::coordinator_entry::DiagnosticStorageLocation>,
    executable: Option<PathBuf>,
    original: EntrySetupBudget,
    client: Option<ExecutionClient>,
    scope: Option<Scope>,
    preparation: Option<PreparationJob>,
    migration: migration::Migration,
    notice: Option<TerminalKind>,
    owner: Option<CoordinatorEntryOwner>,
    failure: Option<Failure>,
    failed_preparation: Option<PreparationCompletion>,
    cleanup_failure: Option<CleanupFailure>,
    closing: bool,
    started: bool,
    retirement_until: Option<Moment>,
}
impl EntryBootstrap {
    /// Retain lexically valid installed-image inputs without native effects.
    /// # Errors
    /// Rejects path grammar before transferring a prepared request to its native owner.
    pub(crate) fn new(
        options: ConfigurationSource,
        installation: PathBuf,
        diagnostics: sugarsubstitute_supervision::coordinator_entry::DiagnosticStorageLocation,
        executable: PathBuf,
        original: EntrySetupBudget,
    ) -> Result<Self, ConfigurationError> {
        LaunchConfiguration::validate_path(&installation)?;
        LaunchConfiguration::validate_path(&executable)?;
        Ok(Self {
            options: Some(options),
            installation: Some(installation),
            diagnostics: Some(diagnostics),
            executable: Some(executable),
            original,
            client: None,
            scope: None,
            preparation: None,
            migration: migration::Migration::default(),
            notice: None,
            owner: None,
            failure: None,
            failed_preparation: None,
            cleanup_failure: None,
            closing: false,
            started: false,
            retirement_until: None,
        })
    }

    /// Retain the actual original scope before the first fallible configuration admission.
    /// # Errors
    /// Rejects restart/closure or actual admission, preserving its original cause.
    pub(crate) fn start(
        &mut self,
        client: &ExecutionClient,
        scope: Scope,
        jobs: &JobSequence,
    ) -> Result<(), EntryBootstrapIssue> {
        if self.started {
            return Err(EntryBootstrapIssue::Started);
        }
        self.started = true;
        self.scope = Some(scope);
        self.client = Some(client.clone());
        if self.closing {
            return Err(EntryBootstrapIssue::Closed);
        }
        let options = self.options.take().ok_or(EntryBootstrapIssue::Input)?;
        let scope = self.scope.as_ref().ok_or(EntryBootstrapIssue::Identity)?;
        let image = self.installation.as_ref().ok_or(EntryBootstrapIssue::Identity)?;
        if let Err(cause) =
            self.migration.start(image.clone(), options, client, scope, jobs, self.original)
        {
            self.failure = Some(Failure::Submission(cause));
            self.request_close();
            return Err(EntryBootstrapIssue::Preparation);
        }
        Ok(())
    }

    /// Route matching real terminal metadata, then the unchanged native owner observations.
    /// Every composing recipient receives this same immutable batch.
    /// # Errors
    /// Retains complete failed preparation, rejected adoption or native owner cause.
    pub(crate) fn consume(
        &mut self,
        batch: &CompletionBatch,
        jobs: &JobSequence,
    ) -> Result<Option<EntryObservation>, EntryBootstrapIssue> {
        let settled = match self.migration.consume(batch, self.closing) {
            Ok(settled) => settled,
            Err(cause) => {
                self.request_close();
                return Err(cause);
            }
        };
        if let Some(settled) = settled {
            let client = self.client.as_ref().ok_or(EntryBootstrapIssue::Identity)?;
            let scope = self.scope.as_ref().ok_or(EntryBootstrapIssue::Identity)?;
            match configuration::submit(client, scope, jobs, settled, self.original) {
                Ok(job) => self.preparation = Some(job),
                Err(cause) => {
                    self.failure = Some(Failure::Submission(cause));
                    self.request_close();
                    return Err(EntryBootstrapIssue::Preparation);
                }
            }
        }
        if let Some(job) = &self.preparation {
            for notice in &batch.notifications {
                if (notice.scope_id, notice.job_id) == (job.scope_id(), job.spec().id) {
                    if self.notice.is_some_and(|kind| kind != notice.kind) {
                        self.request_close();
                        return Err(EntryBootstrapIssue::Identity);
                    }
                    self.notice = Some(notice.kind);
                }
            }
        }
        if self.notice.is_some()
            && let Some(job) = &mut self.preparation
            && let Some(completion) = job.try_take_completion()
        {
            let matches = completion.scope_id == job.scope_id()
                && completion.spec == job.spec()
                && Some(completion.outcome.kind()) == self.notice;
            self.preparation = None;
            self.notice = None;
            let closing = self.closing;
            if !matches || closing || !matches!(completion.outcome, JobOutcome::Success(_)) {
                self.failed_preparation = Some(completion);
                self.request_close();
                if !matches {
                    return Err(EntryBootstrapIssue::Identity);
                }
                if closing {
                    return Ok(None);
                }
                return Err(EntryBootstrapIssue::Preparation);
            }
            self.adopt(completion)?;
            self.migration.captured();
        }
        let observation = if let Some(owner) = &mut self.owner {
            match owner.consume(batch) {
                Ok(observation) => observation,
                Err(cause) => {
                    self.retain_native_failure(cause);
                    return Err(EntryBootstrapIssue::NativeOwner);
                }
            }
        } else {
            None
        };
        let observation = self.observe(observation)?;
        self.advance(jobs)?;
        Ok(observation)
    }

    fn adopt(&mut self, completion: PreparationCompletion) -> Result<(), EntryBootstrapIssue> {
        let JobOutcome::Success(prepared) = completion.outcome else {
            return Err(EntryBootstrapIssue::Preparation);
        };
        let client = self.client.as_ref().ok_or(EntryBootstrapIssue::Identity)?;
        let request = match client.adopt_prepared_entry_request(prepared, self.original) {
            Ok(request) => request,
            Err(rejected) => {
                self.failure = Some(Failure::Adoption(rejected));
                self.request_close();
                return Err(EntryBootstrapIssue::Adoption);
            }
        };
        let installation = self.installation.take().ok_or(EntryBootstrapIssue::Input)?;
        let executable = self.executable.take().ok_or(EntryBootstrapIssue::Input)?;
        let diagnostics = self.diagnostics.take().ok_or(EntryBootstrapIssue::Input)?;
        match CoordinatorEntryInput::from_owned(installation, executable, diagnostics, request) {
            Ok(input) => self.owner = Some(CoordinatorEntryOwner::new(input, self.original)),
            Err(cause) => {
                self.failure = Some(Failure::Input(cause));
                self.request_close();
                return Err(EntryBootstrapIssue::Input);
            }
        }
        Ok(())
    }

    pub(super) fn advance(&mut self, jobs: &JobSequence) -> Result<(), EntryBootstrapIssue> {
        if self.closing {
            return Ok(());
        }
        let Some(owner) = &mut self.owner else { return Ok(()) };
        let phase = owner.snapshot().phase;
        if !matches!(phase, EntryPhase::Inert | EntryPhase::Discovered) {
            return Ok(());
        }
        let client = self.client.as_ref().ok_or(EntryBootstrapIssue::Identity)?;
        let scope = self.scope.as_ref().ok_or(EntryBootstrapIssue::Identity)?;
        let id = jobs.allocate().map_err(|cause| {
            self.failure = Some(Failure::Submission(Box::new(cause)));
            EntryBootstrapIssue::Identity
        })?;
        let spec = JobSpec {
            id,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(self.original.until()),
            reservation_bytes: ENTRY_PREPARATION_BYTES,
            replacement: None,
        };
        let result = if phase == EntryPhase::Inert {
            owner.submit_discovery(client, scope, spec)
        } else {
            owner.submit_election(client, scope, spec)
        };
        if let Err(cause) = result {
            self.failure = Some(Failure::Admission(cause));
            self.request_close();
            return Err(EntryBootstrapIssue::NativeOwner);
        }
        Ok(())
    }

    /// Borrow the actual retained owner for its existing claims and serving contracts.
    pub(crate) fn owner(&self) -> Option<&CoordinatorEntryOwner> {
        self.owner.as_ref()
    }
    /// Mutate the actual owner; this exposes no native handles or raw scheduler.
    pub(crate) fn owner_mut(&mut self) -> Option<&mut CoordinatorEntryOwner> {
        self.owner.as_mut()
    }
    /// Fence ordinary work and cancel actual outstanding preparation once.
    pub(crate) fn request_close(&mut self) {
        self.closing = true;
        self.migration.cancel();
        if let Some(job) = &self.preparation {
            job.cancel();
        }
        if let Some(owner) = &mut self.owner {
            owner.close_admission();
        }
    }
    /// Admit checked namespace/election retirement through the separate real cleanup scope.
    /// # Errors
    /// Rejects cutoff renewal; actual cleanup causes remain retained without replacing startup.
    pub(crate) fn retire(
        &mut self,
        cleanup: &Scope,
        jobs: &JobSequence,
        until: Moment,
    ) -> Result<bool, EntryBootstrapIssue> {
        self.request_close();
        if self.retirement_until.is_some_and(|original| until > original) {
            return Err(EntryBootstrapIssue::Cleanup);
        }
        self.retirement_until =
            Some(self.retirement_until.map_or(until, |original| original.min(until)));
        if self.preparation.is_some() || self.migration.pending() {
            return Ok(false);
        }
        let Some(owner) = &mut self.owner else { return Ok(true) };
        let snapshot = owner.snapshot();
        if snapshot.phase == EntryPhase::Settled {
            return Ok(true);
        }
        if snapshot.pending.is_some() || self.cleanup_failure.is_some() {
            return Ok(false);
        }
        let client = self.client.as_ref().ok_or(EntryBootstrapIssue::Identity)?;
        let spec = JobSpec {
            id: jobs.allocate().map_err(|_| EntryBootstrapIssue::Identity)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: self.retirement_until,
            reservation_bytes: ENTRY_PREPARATION_BYTES,
            replacement: None,
        };
        if let Err(cause) = owner.submit_close(client, cleanup, spec) {
            self.cleanup_failure = Some(CleanupFailure::Admission(cause));
            return Err(EntryBootstrapIssue::Cleanup);
        }
        Ok(false)
    }
}

#[cfg(test)]
#[path = "entry_bootstrap/tests.rs"]
mod tests;
