// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Update SugarCubes during the splash, before the managed backend first inspects or loads it.
//!
//! [`PackSchedule`] decides the work: the first preparation of a coordinator's life runs one
//! startup update (managed) or a read-only check (attached ComfyUI, whose folder is never
//! written); a managed update holds that preparation's SugarCubes inspection and launch until
//! it settles, so nothing reads the pack while its files change. Trial observations follow in
//! order: backend ready, the page established, the backend ending before ready, the page not
//! loading (carried by the child's attempt settlement), and the user cancelling a startup whose
//! page never became ready (carried by the desktop's exit intent).
//!
//! The version being judged fails its judgement when its backend ends before it is ready, when
//! its page reports SugarCubes unavailable or failed, or when the user cancels the startup after
//! its backend became ready and before its page loaded: a page that cannot become ready is
//! never a dead end. After a page failure the next managed preparation of the run is held
//! behind a restore of the previous version, and a run that ends instead leaves the failure in
//! the judgement, so the next start restores it before its backend starts; either way a version
//! that failed never loads twice. A page that did not load for a cause it could not attribute
//! leaves a later page of the run to decide, and the next start restores the previous version
//! when none does. A run that ends without any of these observations is no verdict: the next
//! start judges the version again, and three such endings in a row count as one failed start.
//! This owner runs those jobs one at a time on the execution owner. A failure of any of them
//! never fails the backend.
//!
//! Each run records its update report in the host's storage, and the schedule publishes the
//! live [`sugarsubstitute_contracts::update::pack::PackUpdateStatus`], logged here on every
//! change; the entry runner carries it in its update status to the window's splash line, one
//! plain line and update report.
use super::*;
use std::{path::PathBuf, time::Duration};
use sugarsubstitute_supervision::{
    pack_install::{
        HttpReleaseSources, PackSchedule, PackWork, PackWorkEnded, PackWorkKind, Restored,
        STARTUP_UPDATE_RESERVATION_BYTES, StartupOutcome, StartupStep, StartupUpdate, TrialEvent,
    },
    update_http::{self, Http},
};

/// The longest the splash waits for a SugarCubes update, or a restore of the previous
/// version, before starting without it.
pub(super) const PACK_UPDATE_LIMIT: Duration = Duration::from_secs(300);
/// The longest one trial observation may take; it writes one small journal.
const PACK_OBSERVATION_LIMIT: Duration = Duration::from_secs(30);
const PACK_OBSERVATION_BYTES: u64 = 1024 * 1024;
/// Where the host keeps the SugarCubes update transaction, inside the installation's state.
const PACK_STORAGE: [&str; 2] = ["pack-updates", "SugarCubes"];

/// What one pack job did.
pub(crate) enum PackJobResult {
    /// The startup update ran.
    Update(StartupOutcome),
    /// A trial observation was recorded.
    Observed,
    /// The backend ended before ready; this is what was restored, if anything.
    Stopped(Option<Restored>),
}

/// A pack job that could not run; the start continues regardless.
#[derive(Debug)]
pub(crate) struct PackJobError(String);

impl std::fmt::Display for PackJobError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PackJobError {}

impl CoordinatorBackend {
    /// The attempt's page did not load. A SugarCubes update being judged is restored before
    /// the next attempt of this run, or at the next start, when the page reported SugarCubes
    /// itself unavailable or failed; otherwise a later page of this run may still keep it, and
    /// the next start restores it if none does.
    pub(crate) fn pack_page_failed(
        &mut self,
        failure: sugarsubstitute_contracts::startup::PageFailure,
        resources: &BackendResources<'_>,
    ) -> Result<()> {
        match failure {
            sugarsubstitute_contracts::startup::PageFailure::Sugarcubes => {
                self.pack.page_failed(resources)
            }
            sugarsubstitute_contracts::startup::PageFailure::Unattributed => {
                self.pack.observe(TrialEvent::PageUnconfirmed, resources)
            }
        }
    }

    /// The user cancelled this run's startup. A SugarCubes update whose backend became ready
    /// and whose page never loaded failed its judgement, and the next start restores the
    /// previous version; see [`PackSchedule::startup_cancelled`].
    pub(crate) fn pack_startup_cancelled(
        &mut self,
        resources: &BackendResources<'_>,
    ) -> Result<()> {
        self.pack.startup_cancelled(resources)
    }

    /// Take a finished SugarCubes job's result and start the next one; a preparation it
    /// released goes on to inspection unless it is being stopped. The activation calls this on
    /// every turn, whatever its records or failures, because only this owner takes those
    /// results: one left untaken keeps the execution owner, and so the coordinator, alive.
    pub(crate) fn advance_pack(&mut self, resources: &BackendResources<'_>) -> Result<()> {
        if let Some(released) = self.pack.advance(resources)?
            && let Ok(index) = self.index(released)
            && let Some(record) = self.records[index].as_mut()
            && record.shutdown.is_none()
        {
            Self::submit_source_inspection(record, resources)?;
        }
        Ok(())
    }

    /// Whether no SugarCubes job is outstanding; the backend's owner never settles before.
    pub(crate) fn pack_idle(&self) -> bool {
        self.pack.idle()
    }

    /// Hold the launch-time SugarCubes update while this launch may still restart into an
    /// application update; the restarted copy then runs it.
    pub(crate) fn hold_pack_update(&mut self, held: bool) {
        self.pack.schedule.hold_update(held);
    }

    /// Whether SugarCubes files are being installed or put back now; a restart waits for it.
    pub(crate) fn pack_installing(&self) -> bool {
        self.pack.installing()
    }

    /// Hold `job` as the restore of the previous SugarCubes version that `key`'s backend
    /// ending started, as the pack owner admits one.
    #[cfg(test)]
    pub(crate) fn hold_pack_restore(
        &mut self,
        key: ResourceKey,
        job: OwnedJob<PackJobResult, PackJobError>,
        scope: sugarsubstitute_contracts::execution::ScopeId,
        spec: JobSpec,
    ) {
        let schedule = &mut self.pack.schedule;
        schedule.prepare(key, true);
        let _ = schedule.start_next();
        schedule.ended(PackWorkEnded::Unfinished { stopped: true });
        schedule.backend_stopped();
        let _ = schedule.start_next();
        self.pack.hold(job, scope, spec);
    }

    /// Hold `job` as the running SugarCubes job, as the pack owner admits one.
    #[cfg(test)]
    pub(crate) fn hold_pack_job(
        &mut self,
        job: OwnedJob<PackJobResult, PackJobError>,
        scope: sugarsubstitute_contracts::execution::ScopeId,
        spec: JobSpec,
    ) {
        self.pack.hold(job, scope, spec);
    }

    /// The live SugarCubes update state of this coordinator, for its window.
    pub(crate) fn pack_update_status(
        &self,
    ) -> sugarsubstitute_contracts::update::pack::PackUpdateStatus {
        self.pack.schedule.status().0.clone()
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
    kept: KeptOutcome,
}

/// What a startup update did, kept by the job itself. A stop or its time limit can arrive while
/// the update runs, and the execution owner then ends the job as cancelled or timed out whatever
/// it returned; files the update had already replaced are installed all the same, so its own
/// outcome is what the window and the next start are told.
pub(super) type KeptOutcome = std::sync::Arc<std::sync::OnceLock<StartupOutcome>>;

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

    /// Whether no SugarCubes job is outstanding: none was admitted, or its terminal result was
    /// taken. Only this owner takes it, so the backend settles no record before then; a result
    /// left untaken keeps the execution owner, and so the coordinator, from ever stopping.
    pub(super) fn idle(&self) -> bool {
        self.running.is_none()
    }

    /// Whether SugarCubes files are being installed or put back now.
    pub(super) fn installing(&self) -> bool {
        self.schedule.installing()
    }

    /// Hold `job` as the running SugarCubes job, as `pump` admits one.
    #[cfg(test)]
    pub(super) fn hold(
        &mut self,
        job: OwnedJob<PackJobResult, PackJobError>,
        scope: sugarsubstitute_contracts::execution::ScopeId,
        spec: JobSpec,
    ) {
        self.running = Some(Running { job, scope, spec, kept: KeptOutcome::default() });
    }

    /// Hold `job` as the startup update that `key`'s first managed preparation started, with
    /// its own outcome kept in `kept`, as `pump` admits one.
    #[cfg(test)]
    pub(super) fn hold_update(
        &mut self,
        key: ResourceKey,
        job: OwnedJob<PackJobResult, PackJobError>,
        scope: sugarsubstitute_contracts::execution::ScopeId,
        spec: JobSpec,
        kept: KeptOutcome,
    ) {
        self.schedule.prepare(key, true);
        let _ = self.schedule.start_next();
        self.running = Some(Running { job, scope, spec, kept });
    }

    /// `key` is being stopped: cancel the running job if it is the one holding `key`, and
    /// leave a launch-time update that has not begun to the copy that starts next. A running
    /// job observes cancellation between steps, never mid-replace; other jobs run on.
    pub(super) fn cancel(&mut self, key: ResourceKey) {
        self.schedule.withdraw_update(key);
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

    /// The user cancelled startup; see [`PackSchedule::startup_cancelled`].
    pub(super) fn startup_cancelled(&mut self, resources: &BackendResources<'_>) -> Result<()> {
        self.schedule.startup_cancelled();
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
        let kept = running.kept.get().cloned();
        self.running = None;
        let ended = match (completion.outcome, kept) {
            (JobOutcome::Success(PackJobResult::Update(outcome)), _) if original => {
                tracing::info!(operation = "pack_update", stage = "Terminal", outcome = ?outcome,
                    "The SugarCubes startup update settled");
                PackWorkEnded::Updated(outcome)
            }
            (JobOutcome::Success(PackJobResult::Stopped(restored)), _) if original => {
                if let Some(restored) = &restored {
                    tracing::warn!(operation = "pack_update", stage = "Restored", version = %restored.version,
                        restored = %restored.restored_version, "The previous SugarCubes version was restored");
                }
                PackWorkEnded::Restored(restored)
            }
            (JobOutcome::Success(PackJobResult::Observed), _) if original => {
                PackWorkEnded::Observed
            }
            (JobOutcome::Cancelled | JobOutcome::TimedOut, Some(outcome)) if original => {
                tracing::info!(operation = "pack_update", stage = "Terminal", outcome = ?outcome,
                    "The SugarCubes startup update settled as it was being stopped");
                PackWorkEnded::Updated(outcome)
            }
            (other, _) => {
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
            let (bytes, limit) = match work.kind() {
                PackWorkKind::Download => (
                    STARTUP_UPDATE_RESERVATION_BYTES + update_http::RESERVATION_BYTES,
                    PACK_UPDATE_LIMIT,
                ),
                PackWorkKind::Replace => (STARTUP_UPDATE_RESERVATION_BYTES, PACK_UPDATE_LIMIT),
                PackWorkKind::Record => (PACK_OBSERVATION_BYTES, PACK_OBSERVATION_LIMIT),
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
            let kept = KeptOutcome::default();
            let keeping = std::sync::Arc::clone(&kept);
            let job =
                resources.client.submit_blocking(resources.cleanup, spec, move |context| {
                    let result = run(work, &folder, &storage, &context);
                    if let Ok(PackJobResult::Update(outcome)) = &result {
                        keeping.get_or_init(|| outcome.clone());
                    }
                    result
                })?;
            tracing::info!(operation = "pack_update", work = ?work, resource = ?gates,
                stage = "Submitted", "A SugarCubes job began");
            self.running = Some(Running { job, scope: resources.cleanup.id(), spec, kept });
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
    // An install the last run was cut off in is undone first: until then the pack's own
    // description of its release sources may be in the backup.
    if managed && let Some(outcome) = update.settle_cut_off() {
        return update.record(outcome);
    }
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
