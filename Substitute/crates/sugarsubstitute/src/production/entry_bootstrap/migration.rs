// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe the separate finite storage preparation before admitting sealed environment capture.
use super::*;
use sugarsubstitute_supervision::setup_storage::{
    self, ExplicitBackendDirectories, LaunchDataFailure, PreparedStartup,
};

struct Input {
    image: PathBuf,
    source: ConfigurationSource,
}
type Completion = JobCompletion<PreparedStartup, LaunchDataFailure>;

/// Complete native custody stays with the original job outcome after failure or late stop.
#[derive(Default)]
pub(super) struct Migration {
    input: Option<Input>,
    job: Option<OwnedJob<PreparedStartup, LaunchDataFailure>>,
    notice: Option<TerminalKind>,
    failed: Option<Completion>,
    custody: Option<setup_storage::StartupDataCustody>,
}
impl Migration {
    pub(super) fn start(
        &mut self,
        image: PathBuf,
        source: ConfigurationSource,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        original: EntrySetupBudget,
    ) -> Result<(), Box<dyn Error>> {
        self.input = Some(Input { image, source });
        self.job = Some(client.submit_blocking_owned_input(
            scope,
            JobSpec {
                id: jobs.allocate()?,
                operation: Operation::NativeLifecycle,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(original.until()),
                reservation_bytes: setup_storage::LAUNCH_DATA_PREPARATION_BYTES,
                replacement: None,
            },
            &mut self.input,
            |input, context| {
                setup_storage::prepare_launch_data(
                    &input.image,
                    input.source,
                    explicit_directories,
                    |path, context| {
                        crate::input_documents::classify_migration_source(path, context)
                            .map_err(std::io::Error::other)
                    },
                    &context,
                )
            },
        )?);
        Ok(())
    }
    /// Match both original execution identity and actual terminal kind before taking an outcome.
    pub(super) fn consume(
        &mut self,
        batch: &CompletionBatch,
        closing: bool,
    ) -> Result<Option<configuration::ConfigurationInput>, EntryBootstrapIssue> {
        let Some(job) = &mut self.job else {
            return Ok(None);
        };
        for notice in &batch.notifications {
            if (notice.scope_id, notice.job_id) == (job.scope_id(), job.spec().id) {
                if self.notice.is_some_and(|kind| kind != notice.kind) {
                    return Err(EntryBootstrapIssue::Identity);
                }
                self.notice = Some(notice.kind);
            }
        }
        if self.notice.is_none() {
            return Ok(None);
        }
        let Some(completion) = job.try_take_completion() else {
            return Ok(None);
        };
        let matches = completion.scope_id == job.scope_id()
            && completion.spec == job.spec()
            && self.notice == Some(completion.outcome.kind());
        self.job = None;
        self.notice = None;
        if !matches || closing || !matches!(completion.outcome, JobOutcome::Success(_)) {
            self.failed = Some(completion);
            return if !matches {
                Err(EntryBootstrapIssue::Identity)
            } else if closing {
                Ok(None)
            } else {
                Err(EntryBootstrapIssue::Preparation)
            };
        }
        match completion.outcome {
            JobOutcome::Success(prepared) => {
                let (options, configuration, custody) = prepared.into_capture_parts();
                let originals = custody.originals_reference();
                self.custody = Some(custody);
                Ok(Some(configuration::ConfigurationInput::Settled(Box::new(
                    configuration::SettledStartup { options, configuration, originals },
                ))))
            }
            _ => Err(EntryBootstrapIssue::Identity),
        }
    }
    pub(super) fn cancel(&self) {
        if let Some(job) = &self.job {
            job.cancel();
        }
    }
    pub(super) fn pending(&self) -> bool {
        self.job.is_some()
    }
    /// Successful sealed adoption ends original-input custody before normal update publication.
    pub(super) fn captured(&mut self) {
        if let Some(custody) = &mut self.custody {
            custody.original_inputs_adopted();
        }
    }
    pub(super) fn failure(&self) -> Option<&LaunchDataFailure> {
        match &self.failed.as_ref()?.outcome {
            JobOutcome::Failure(cause) => Some(cause),
            _ => None,
        }
    }
    #[cfg(test)]
    pub(super) fn failure_completion(&self) -> Option<&Completion> {
        self.failed.as_ref()
    }
    #[cfg(test)]
    pub(super) fn job_spec(&self) -> Option<JobSpec> {
        self.job.as_ref().map(OwnedJob::spec)
    }
}
fn explicit_directories(
    request: &LaunchConfiguration,
) -> Result<ExplicitBackendDirectories, sugarsubstitute_comfy::launch::ManagedLaunchError> {
    use sugarsubstitute_comfy::launch::{StorageDirectory, explicit_storage_directories};
    let selections = explicit_storage_directories(request)?;
    Ok(ExplicitBackendDirectories {
        base: selections.get(StorageDirectory::Base).is_some(),
        user: selections.get(StorageDirectory::User).is_some(),
        temporary: selections.get(StorageDirectory::Temporary).is_some(),
        input: selections.get(StorageDirectory::Input).is_some(),
        output: selections.get(StorageDirectory::Output).is_some(),
        models: selections.get(StorageDirectory::Models).is_some(),
    })
}
