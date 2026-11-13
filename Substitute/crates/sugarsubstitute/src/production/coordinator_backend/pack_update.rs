// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Update SugarCubes during the splash, before the managed backend first inspects or loads it.
//!
//! [`PackSchedule`] decides the work: the first preparation of a coordinator's life runs one
//! startup update (managed) or a read-only check (attached ComfyUI, whose folder is never
//! written); a managed update holds that preparation's SugarCubes inspection and launch until
//! it settles, so nothing reads the pack while its files change. Trial observations follow in
//! order: backend ready, the page established, the backend ending before ready, and the page
//! reporting SugarCubes failing (carried by the child's attempt settlement). After a failed
//! page the next managed preparation is held behind a restore of the previous version, so a
//! version that failed never loads twice. This owner runs those jobs one at a time on the
//! execution owner. A failure of any of them never fails the backend.
//!
//! Each run records its update report in the host's storage, and the schedule publishes the
//! live [`sugarsubstitute_contracts::update::pack::PackUpdateStatus`] with a revision, logged
//! here on every change, for the splash line, the one plain line and the window's report.
use super::*;
use std::{path::PathBuf, time::Duration};
use sugarsubstitute_supervision::{
    pack_install::{
        HttpReleaseSources, PackSchedule, PackWork, PackWorkEnded, Restored,
        STARTUP_UPDATE_RESERVATION_BYTES, StartupOutcome, StartupStep, StartupUpdate, TrialEvent,
    },
    update_http::{self, Http},
};

/// The longest the splash waits for a SugarCubes update before starting without it.
pub(super) const PACK_UPDATE_LIMIT: Duration = Duration::from_secs(300);
/// The longest one trial observation may take; it writes one small journal.
const PACK_OBSERVATION_LIMIT: Duration = Duration::from_secs(30);
const PACK_OBSERVATION_BYTES: u64 = 1024 * 1024;
/// Where the host keeps the SugarCubes update transaction, inside the installation's state.
const PACK_STORAGE: [&str; 2] = ["pack-updates", "SugarCubes"];

/// What one pack job did.
pub(super) enum PackJobResult {
    /// The startup update ran.
    Update(StartupOutcome),
    /// A trial observation was recorded.
    Observed,
    /// The backend ended before ready; this is what was restored, if anything.
    Stopped(Option<Restored>),
}

/// A pack job that could not run; the start continues regardless.
#[derive(Debug)]
pub(super) struct PackJobError(String);

impl std::fmt::Display for PackJobError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PackJobError {}

impl CoordinatorBackend {
    /// The attempt's page reported SugarCubes itself unavailable or failed: a SugarCubes
    /// update being judged did not load its page, and is restored before the next attempt.
    pub(crate) fn pack_page_failed(&mut self, resources: &BackendResources<'_>) -> Result<()> {
        self.pack.page_failed(resources)
    }
    /// The page established its session with every SugarCubes node registered; this keeps a
    /// SugarCubes update being judged. Only a managed backend's pack is ever judged.
    pub(crate) fn pack_page_loaded(&mut self, resources: &BackendResources<'_>) -> Result<()> {
        self.pack.observe(
            sugarsubstitute_supervision::pack_install::TrialEvent::PageLoaded {
                nodes_registered: true,
            },
            resources,
        )
    }
}

struct Running {
    job: OwnedJob<PackJobResult, PackJobError>,
    scope: sugarsubstitute_contracts::execution::ScopeId,
    spec: JobSpec,
}

/// The coordinator's single owner of SugarCubes jobs. [`PackSchedule`] decides what runs and
/// which preparation it holds; this owner runs one job at a time on the execution owner.
#[derive(Default)]
pub(super) struct PackUpdates {
    storage: Option<PathBuf>,
    folder: Option<PathBuf>,
    schedule: PackSchedule<ResourceKey>,
    running: Option<Running>,
    /// The latest preparation's budget end, which bounds the update it waits for.
    until: Option<Moment>,
    published: u64,
}

impl PackUpdates {
    /// Keep the transaction in the state folder of the installation that owns `working`.
    pub(super) fn configure(&mut self, working: &std::path::Path) {
        let state =
            sugarsubstitute_configuration::layout::InstallationLayout::for_image_directory(working)
                .state();
        self.storage = Some(PACK_STORAGE.iter().fold(state, |path, part| path.join(part)));
    }

    /// Whether `key`'s inspection and launch still wait for a SugarCubes job.
    pub(super) fn gates(&self, key: ResourceKey) -> bool {
        self.schedule.gates(key)
    }

    /// `key` is being stopped: cancel the running job if it is the one holding `key`. It
    /// observes cancellation between steps, never mid-replace; other jobs run on.
    pub(super) fn cancel(&self, key: ResourceKey) {
        if self.schedule.running_holds(key)
            && let Some(running) = &self.running
        {
            running.job.cancel();
        }
    }

    /// Prepare SugarCubes for one backend preparation; returns whether a job now holds `key`'s
    /// inspection and launch. See [`PackSchedule::prepare`].
    pub(super) fn begin(
        &mut self,
        key: ResourceKey,
        folder: &std::path::Path,
        managed: bool,
        until: Moment,
        resources: &BackendResources<'_>,
    ) -> Result<bool> {
        if self.storage.is_none() {
            return Ok(false);
        }
        if self.folder.is_none() {
            self.folder = Some(folder.to_owned());
        }
        self.until = Some(until);
        let gated = self.schedule.prepare(key, managed);
        self.pump(resources)?;
        Ok(gated)
    }

    /// Record one observation of the version the managed backend runs.
    pub(super) fn observe(
        &mut self,
        event: TrialEvent,
        resources: &BackendResources<'_>,
    ) -> Result<()> {
        self.schedule.observe(event);
        self.pump(resources)
    }

    /// The managed backend ended before it was ready.
    pub(super) fn backend_stopped(&mut self, resources: &BackendResources<'_>) -> Result<()> {
        self.schedule.backend_stopped();
        self.pump(resources)
    }

    /// The attempt's page reported SugarCubes itself failing; the judged version is restored
    /// before the next managed attempt starts it.
    pub(super) fn page_failed(&mut self, resources: &BackendResources<'_>) -> Result<()> {
        self.schedule.page_failed();
        self.pump(resources)
    }

    /// Adopt a finished job; returns the record whose inspection and launch it released.
    pub(super) fn advance(
        &mut self,
        resources: &BackendResources<'_>,
    ) -> Result<Option<ResourceKey>> {
        let Some(running) = &mut self.running else {
            self.pump(resources)?;
            return Ok(None);
        };
        let Some(completion) = running.job.try_take_completion() else {
            return Ok(None);
        };
        let original = completion.scope_id == running.scope && completion.spec == running.spec;
        self.running = None;
        let ended = match completion.outcome {
            JobOutcome::Success(PackJobResult::Update(outcome)) if original => {
                tracing::info!(operation = "pack_update", stage = "Terminal", outcome = ?outcome,
                    "The SugarCubes startup update settled");
                PackWorkEnded::Updated(outcome)
            }
            JobOutcome::Success(PackJobResult::Stopped(restored)) if original => {
                if let Some(restored) = &restored {
                    tracing::warn!(operation = "pack_update", stage = "Restored", version = %restored.version,
                        restored = %restored.restored_version, "The previous SugarCubes version was restored");
                }
                PackWorkEnded::Restored(restored)
            }
            JobOutcome::Success(PackJobResult::Observed) if original => PackWorkEnded::Observed,
            other => {
                let stopped = matches!(other, JobOutcome::Cancelled | JobOutcome::TimedOut);
                tracing::warn!(
                    operation = "pack_update",
                    stage = "Terminal",
                    stopped,
                    "A SugarCubes update job did not finish; the installed version is used"
                );
                PackWorkEnded::Unfinished { stopped }
            }
        };
        let released = self.schedule.ended(ended);
        self.pump(resources)?;
        Ok(released)
    }

    /// Start the schedule's next job when none runs, and log each status change for the
    /// window's update surfaces.
    fn pump(&mut self, resources: &BackendResources<'_>) -> Result<()> {
        if self.running.is_none()
            && let (Some(folder), Some(storage)) = (self.folder.clone(), self.storage.clone())
            && let Some((work, gates)) = self.schedule.start_next()
        {
            let (bytes, limit) = match work {
                PackWork::Update { .. } => (
                    STARTUP_UPDATE_RESERVATION_BYTES + update_http::RESERVATION_BYTES,
                    PACK_UPDATE_LIMIT,
                ),
                _ => (PACK_OBSERVATION_BYTES, PACK_OBSERVATION_LIMIT),
            };
            let limit = resources.client.now()?.checked_add(limit)?;
            let deadline = match self.until {
                Some(until) if gates.is_some() && until < limit => until,
                _ => limit,
            };
            let spec = CoordinatorBackend::spec(
                resources,
                Operation::Infrastructure,
                JobClass::InfrastructureLifecycle,
                bytes,
                Some(deadline),
            )?;
            let job =
                resources.client.submit_blocking(resources.cleanup, spec, move |context| {
                    run(work, &folder, &storage, &context)
                })?;
            tracing::info!(operation = "pack_update", work = ?work, resource = ?gates,
                stage = "Submitted", "A SugarCubes job began");
            self.running = Some(Running { job, scope: resources.cleanup.id(), spec });
        }
        let (status, revision) = self.schedule.status();
        if revision != self.published {
            self.published = revision;
            tracing::info!(operation = "pack_update_status", revision, status = ?status,
                "The SugarCubes update status changed");
        }
        Ok(())
    }
}

/// The blocking body of one SugarCubes job.
fn run(
    work: PackWork,
    folder: &std::path::Path,
    storage: &std::path::Path,
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> std::result::Result<PackJobResult, PackJobError> {
    let failed = |cause: sugarsubstitute_supervision::pack_install::PackInstallError| {
        PackJobError(cause.to_string())
    };
    if let PackWork::Update { managed } = work {
        return Ok(PackJobResult::Update(run_update(folder, storage, managed, context)));
    }
    let update =
        StartupUpdate::at(folder, storage).map_err(|cause| PackJobError(cause.to_string()))?;
    match work {
        PackWork::Observe(event) => {
            update.observe(event).map(|()| PackJobResult::Observed).map_err(failed)
        }
        PackWork::BackendStopped => {
            update.backend_stopped().map(PackJobResult::Stopped).map_err(failed)
        }
        PackWork::RestoreFailedPage => {
            update.restore_failed_page().map(PackJobResult::Stopped).map_err(failed)
        }
        PackWork::Update { .. } => Err(PackJobError("The update ran above".into())),
    }
}

/// The blocking body of the startup update: never fails, always returns what happened.
fn run_update(
    folder: &std::path::Path,
    storage: &std::path::Path,
    managed: bool,
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> StartupOutcome {
    let update = match StartupUpdate::at(folder, storage) {
        Ok(update) => update,
        Err(_) => return StartupOutcome::Failed { step: StartupStep::Checking, code: "io" },
    };
    let config = match update.release_sources() {
        Ok(config) => config,
        Err(outcome) => return update.record(outcome),
    };
    let mut http = Http::new();
    let outcome =
        match HttpReleaseSources::new(&mut http, &config, &storage.join("published"), context) {
            Ok(sources) if managed => update.run(&sources, &|| {
                !context.cancellation_requested()
                    && context.deadline().is_some_and(|deadline| context.now() < deadline)
            }),
            Ok(sources) => update.check(&sources),
            Err(_) => update.record(StartupOutcome::Failed {
                step: StartupStep::Checking,
                code: "release-source",
            }),
        };
    if !http.settled()
        && let Err(cause) = http.retire()
    {
        tracing::error!(operation = "pack_update", error = %cause,
            "The SugarCubes update's HTTP custody could not be retired");
    }
    outcome
}
