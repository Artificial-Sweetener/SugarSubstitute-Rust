// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare exact selected-build seed facts on a finite worker before contained child creation.
use super::*;
use std::time::{SystemTime, UNIX_EPOCH};
use sugarsubstitute_application::incident::{IncidentOperationBudget, IncidentRequestContext};
use sugarsubstitute_contracts::{execution::*, incident::*};
use sugarsubstitute_crash::CaptureBuildIdentity;

pub(super) struct SeedMaterial {
    pub(super) seed: IncidentSeed,
    pub(super) context: IncidentRequestContext,
    _storage: MemoryClaim,
}
type MaterialJob = OwnedJob<SeedMaterial, MaterialFailure>;
type MaterialCompletion = JobCompletion<SeedMaterial, MaterialFailure>;

/// Retain real terminal delivery and seed data until the genuine store commit is adopted.
pub(super) struct SeedPreparation {
    job: Option<MaterialJob>,
    completed: Option<MaterialCompletion>,
    notice: Option<TerminalKind>,
    admitted: bool,
}
#[derive(Debug)]
pub(super) struct MaterialFailure {
    cause: Box<dyn Error + Send + Sync>,
    _original: MemoryLease,
}
impl std::fmt::Display for MaterialFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Incident seed preparation failed")
    }
}
impl Error for MaterialFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}
impl SeedPreparation {
    /// Supply deterministic seed facts through a real admitted worker for ownership tests.
    #[cfg(test)]
    pub(super) fn boundary_fixture(
        client: &sugarsubstitute_execution::ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        run: RunKey,
    ) -> Result<Self> {
        let job = client.submit_blocking(scope, spec, move |context| {
            let original = context.memory().clone();
            let result = (|| -> std::result::Result<_, Box<dyn Error + Send + Sync>> {
                let at = IncidentUtc::from_unix_seconds(0)?;
                Ok(SeedMaterial {
                    seed: IncidentSeed::new(
                        IncidentKey::new(IncidentSubject::Desktop(run)),
                        IncidentRevision::new(1)?,
                        BuildIdentity::new("closed-normal-boundary")?,
                        at,
                        IncidentStage::DesktopInitialization,
                    )?,
                    context: IncidentRequestContext::new(
                        IncidentOperationBudget::new(context.now())?,
                        at,
                    ),
                    _storage: context.memory().claim(512)?,
                })
            })();
            result.map_err(|cause| MaterialFailure { cause, _original: original })
        })?;
        Ok(Self { job: Some(job), completed: None, notice: None, admitted: true })
    }
    pub(super) const fn new() -> Self {
        Self { job: None, completed: None, notice: None, admitted: false }
    }
    /// Use only the actual capture owner's cached selected build and current entry cutoff.
    pub(super) fn submit(
        &mut self,
        run: RunKey,
        build: CaptureBuildIdentity,
        resources: &EntryResources<'_>,
        until: Moment,
    ) -> Result<()> {
        if self.admitted {
            return Ok(());
        }
        let spec = JobSpec {
            id: resources.jobs.allocate()?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(until),
            reservation_bytes: 32 * 1024,
            replacement: None,
        };
        self.job =
            Some(resources.client.submit_blocking(resources.finite, spec, move |context| {
                prepare(run, build, &context)
            })?);
        self.admitted = true;
        Ok(())
    }
    /// Adopt only this actual job's terminal; unknown notices create no seed facts.
    pub(super) fn capture(&mut self, batch: &CompletionBatch) -> Result<()> {
        let Some(job) = &mut self.job else { return Ok(()) };
        for notice in &batch.notifications {
            if (notice.scope_id, notice.job_id) == (job.scope_id(), job.spec().id) {
                if self.notice.is_some_and(|old| old != notice.kind) {
                    return Err("Incident seed terminal identity changed".into());
                }
                self.notice = Some(notice.kind);
            }
        }
        if let Some(notice) = self.notice
            && let Some(done) = job.try_take_completion()
        {
            let valid = done.scope_id == job.scope_id()
                && done.spec == job.spec()
                && done.outcome.kind() == notice;
            self.completed = Some(done);
            self.job = None;
            if !valid {
                return Err("Incident seed completion identity changed".into());
            }
        }
        Ok(())
    }
    pub(super) fn material(&self) -> Result<Option<&SeedMaterial>> {
        match self.completed.as_ref().map(|done| &done.outcome) {
            Some(JobOutcome::Success(value)) => Ok(Some(value)),
            Some(_) => {
                Err("Incident seed preparation stopped with retained original outcome".into())
            }
            None => Ok(None),
        }
    }
    pub(super) fn failure(&self) -> Option<&(dyn Error + 'static)> {
        match self.completed.as_ref().map(|done| &done.outcome) {
            Some(JobOutcome::Failure(cause)) => Some(cause),
            _ => None,
        }
    }
    pub(super) fn cancel(&self) {
        if let Some(job) = &self.job {
            job.cancel();
        }
    }
    /// Actual finite preparation admission, independent of terminal delivery.
    pub(super) const fn started(&self) -> bool {
        self.admitted
    }
    pub(super) fn delivered(&self) -> bool {
        self.job.is_none()
    }
}
fn prepare(
    run: RunKey,
    build: CaptureBuildIdentity,
    context: &JobContext,
) -> std::result::Result<SeedMaterial, MaterialFailure> {
    let original = context.memory().clone();
    let result = (|| -> std::result::Result<_, Box<dyn Error + Send + Sync>> {
        if context.cancellation_requested()
            || context.deadline().is_none_or(|until| context.now() >= until)
        {
            return Err("Original incident seed preparation stopped".into());
        }
        let storage = context.memory().claim(512)?;
        let elapsed = SystemTime::now().duration_since(UNIX_EPOCH)?;
        let at = IncidentUtc::from_unix_seconds(i64::try_from(elapsed.as_secs())?)?;
        let budget = IncidentOperationBudget::new(context.now())?;
        let seed = IncidentSeed::new(
            IncidentKey::new(IncidentSubject::Desktop(run)),
            IncidentRevision::new(1)?,
            BuildIdentity::new(build.as_str()?)?,
            at,
            IncidentStage::DesktopInitialization,
        )?;
        if context.cancellation_requested()
            || context.deadline().is_none_or(|until| context.now() >= until)
        {
            return Err("Original incident seed preparation stopped".into());
        }
        Ok(SeedMaterial {
            seed,
            context: IncidentRequestContext::new(budget, at),
            _storage: storage,
        })
    })();
    result.map_err(|cause| MaterialFailure { cause, _original: original })
}
