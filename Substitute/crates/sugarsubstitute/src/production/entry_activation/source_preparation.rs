// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe source-host settlement before separately admitting the sealed projection.
use super::*;
use std::{fmt, path::PathBuf, sync::Arc};
use sugarsubstitute_execution::ChargedBlockingInput;
use sugarsubstitute_supervision::{
    coordinator_entry::EntrySourceDataAdmission,
    setup_storage::{LAUNCH_DATA_PREPARATION_BYTES, SourceLaunchSettlement},
};

struct Input {
    projection: Option<ProjectionInput>,
    image: PathBuf,
    settlement: Option<SourceLaunchSettlement>,
    cause: Option<Box<dyn Error + Send + Sync>>,
    post: Option<projection::ControlFailure>,
}
/// Original failure, native custody and charged input remain one observed completion.
pub(super) struct Failure {
    retained: ChargedBlockingInput<Input>,
}
impl fmt::Debug for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceLaunchPreparationFailure")
            .field("post_control", &self.retained.input().post)
            .field("original_retained", &self.retained.input().projection.is_some())
            .finish_non_exhaustive()
    }
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Source launch preparation failed with retained original custody")
    }
}
impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.retained.input().cause.as_deref().map(|cause| cause as _)
    }
}

/// One separate finite source job; no capture reservation or original deadline is enlarged.
pub(super) struct Preparation {
    input: Option<Input>,
    job: Option<OwnedJob<Option<ProjectionInput>, Failure>>,
    completion: Option<JobCompletion<Option<ProjectionInput>, Failure>>,
    next: Option<ProjectionInput>,
    failed: Option<Box<dyn Error>>,
}
impl Preparation {
    pub(super) fn new(
        projection: ProjectionInput,
        image: PathBuf,
        admission: EntrySourceDataAdmission,
    ) -> Self {
        Self {
            input: Some(Input {
                projection: Some(projection),
                image,
                settlement: Some(SourceLaunchSettlement::new(admission)),
                cause: None,
                post: None,
            }),
            job: None,
            completion: None,
            next: None,
            failed: None,
        }
    }
    pub(super) fn start(
        &mut self,
        resources: &EntryResources<'_>,
        until: sugarsubstitute_contracts::execution::Moment,
    ) -> Result<()> {
        self.job = Some(resources.client.submit_blocking_owned_box(
            resources.finite,
            JobSpec {
                id: resources.jobs.allocate()?,
                operation: Operation::NativeLifecycle,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(until),
                reservation_bytes: LAUNCH_DATA_PREPARATION_BYTES,
                replacement: None,
            },
            &mut self.input,
            prepare,
        )?);
        Ok(())
    }
    pub(super) fn rejected_request(mut self) -> Option<AuthenticatedEntryLaunch> {
        self.input.take()?.projection?.rejected_request()
    }
    pub(super) fn cancel(&self) {
        if let Some(job) = &self.job {
            job.cancel();
        }
    }
    pub(super) fn settled(&self) -> bool {
        self.job.is_none()
    }
    pub(super) fn failure(&self) -> Option<&(dyn Error + 'static)> {
        self.failed.as_deref().or_else(|| match &self.completion.as_ref()?.outcome {
            JobOutcome::Failure(cause) => Some(cause as _),
            _ => None,
        })
    }
    /// Adopt the exact source completion before attempting the existing four-MiB job.
    /// Rejected second admission retains both the frame and complete settled source here.
    pub(super) fn advance(
        &mut self,
        resources: &EntryResources<'_>,
        closing: bool,
    ) -> Result<Option<OwnedJob<ProjectedEntry, ProjectionFailure>>> {
        if let Some(job) = &mut self.job
            && let Some(completed) = job.try_take_completion()
        {
            let matches = completed.scope_id == job.scope_id() && completed.spec == job.spec();
            self.completion = Some(completed);
            self.job = None;
            if !matches {
                return Err("Source preparation completion identity changed".into());
            }
        }
        if closing {
            self.cancel();
            if let Some(completed) = &mut self.completion {
                match &mut completed.outcome {
                    JobOutcome::Success(Some(input)) => input.release_request(),
                    JobOutcome::Failure(failure) => {
                        if let Some(input) = &mut failure.retained.input_mut().projection {
                            input.release_request();
                        }
                    }
                    _ => {}
                }
            }
            if let Some(input) = &mut self.next {
                input.release_request();
            }
            return Ok(None);
        }
        if self.failed.is_some() {
            return Err("Source preparation retains second admission failure".into());
        }
        if self.next.is_none()
            && let Some(completed) = &mut self.completion
        {
            match &mut completed.outcome {
                JobOutcome::Success(input) => self.next = input.take(),
                _ => {
                    return Err(
                        "Source preparation failed with its original retained outcome".into()
                    );
                }
            }
        }
        if let Some(input) = &mut self.next
            && let Err(cause) = input.observed_source_terminal()
        {
            self.failed = Some(cause);
            return Err("Source terminal custody could not be released after observation".into());
        }
        let Some(input) = self.next.as_ref() else {
            return Ok(None);
        };
        let until = input.original_until().ok_or("Source prepared frame missing")?;
        let spec = JobSpec {
            id: resources.jobs.allocate()?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(until),
            reservation_bytes: projection::PROJECTION_BYTES,
            replacement: None,
        };
        match resources.client.submit_blocking_owned_box(
            resources.finite,
            spec,
            &mut self.next,
            projection::project,
        ) {
            Ok(job) => Ok(Some(job)),
            Err(cause) => {
                self.failed = Some(Box::new(cause));
                Err("Source projection admission failed with retained input".into())
            }
        }
    }
}

fn prepare(
    mut input: ChargedBlockingInput<Input>,
    context: sugarsubstitute_contracts::execution::JobContext,
) -> std::result::Result<Option<ProjectionInput>, Failure> {
    let outcome = (|| -> std::result::Result<(), Box<dyn Error + Send + Sync>> {
        let input = input.input_mut();
        let projection = input.projection.as_ref().ok_or("Original source projection absent")?;
        let request =
            projection.authenticated_request().ok_or("Original authenticated source absent")?;
        let run = projection.run_key().ok_or("Original source run absent")?;
        input.settlement.as_mut().ok_or("Original source settlement absent")?.settle(
            &input.image,
            request,
            run,
            &mut |path, context| {
                crate::input_documents::classify_migration_source(path, context)
                    .map_err(std::io::Error::other)
            },
            &context,
        )?;
        Ok(())
    })();
    if let Err(cause) = outcome {
        input.input_mut().cause = Some(cause);
        let post = input
            .input()
            .projection
            .as_ref()
            .and_then(ProjectionInput::authenticated_request)
            .and_then(|request| {
                projection::check(&context, request.received_at(), request.until()).err()
            });
        input.input_mut().post = post;
        return Err(Failure { retained: input });
    }
    if input.input().settlement.is_none() || input.input().projection.is_none() {
        input.input_mut().cause = Some("Completed source preparation custody is incomplete".into());
        return Err(Failure { retained: input });
    }
    let Some(mut projection) = input.input_mut().projection.take() else {
        return Err(Failure { retained: input });
    };
    let Some(settlement) = input.input_mut().settlement.take() else {
        input.input_mut().projection = Some(projection);
        return Err(Failure { retained: input });
    };
    projection.retain_source(Arc::new(settlement));
    Ok(Some(projection))
}
