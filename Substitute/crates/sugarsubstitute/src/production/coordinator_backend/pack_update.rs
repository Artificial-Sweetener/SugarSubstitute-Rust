// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Update SugarCubes during the splash, before the managed backend first inspects or loads it.
//!
//! The first managed preparation of a coordinator's life runs one startup update on an admitted
//! blocking worker and holds both the SugarCubes source inspection and the backend launch until
//! it settles, so nothing reads the pack while its files change. Later observations of that
//! version (backend ready, the page established, the backend ending before ready) are written to
//! the host's transaction as one small job each, in order. A failure of any of this never fails
//! the backend: the start continues with whatever SugarCubes the folder now holds. The startup
//! update records its outcome as the update report in the host's storage, from which the one
//! plain line is shown; it is also logged here. An attached ComfyUI, which its user runs and
//! which may already have the pack loaded, only gets a read-only check that reports a newer
//! version; nothing in its folder is ever written.

use super::*;
use std::{collections::VecDeque, path::PathBuf, time::Duration};
use sugarsubstitute_supervision::{
    pack_install::{
        HttpReleaseSources, Restored, STARTUP_UPDATE_RESERVATION_BYTES, StartupOutcome,
        StartupStep, StartupUpdate, TrialEvent,
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

/// One effect on the host's SugarCubes transaction, in the order the coordinator saw it.
enum PackEffect {
    Observe(TrialEvent),
    BackendStopped,
}

struct Running {
    job: OwnedJob<PackJobResult, PackJobError>,
    scope: sugarsubstitute_contracts::execution::ScopeId,
    spec: JobSpec,
    /// The record whose source inspection and launch wait for this job.
    gates: Option<ResourceKey>,
}

/// The coordinator's single owner of SugarCubes updates and their trial observations.
#[derive(Default)]
pub(super) struct PackUpdates {
    storage: Option<PathBuf>,
    folder: Option<PathBuf>,
    checked: bool,
    running: Option<Running>,
    queued: VecDeque<PackEffect>,
}

impl PackUpdates {
    /// Keep the transaction in the state folder of the installation that owns `working`.
    pub(super) fn configure(&mut self, working: &std::path::Path) {
        let state =
            sugarsubstitute_configuration::layout::InstallationLayout::for_image_directory(working)
                .state();
        self.storage = Some(PACK_STORAGE.iter().fold(state, |path, part| path.join(part)));
    }

    /// Whether `key`'s inspection and launch still wait for the startup update.
    pub(super) fn gates(&self, key: ResourceKey) -> bool {
        self.running.as_ref().is_some_and(|running| running.gates == Some(key))
    }

    /// Ask a running job to stop; it observes cancellation between steps, never mid-replace.
    pub(super) fn cancel(&self) {
        if let Some(running) = &self.running {
            running.job.cancel();
        }
    }

    /// Run the launch-time update for the first preparation of this coordinator: for a managed
    /// backend, the update itself, which holds `key`'s inspection and launch (returns true); for
    /// an attached ComfyUI, only a read-only check that reports a newer version.
    pub(super) fn begin(
        &mut self,
        key: ResourceKey,
        folder: &std::path::Path,
        managed: bool,
        scope: &Scope,
        until: Moment,
        resources: &BackendResources<'_>,
    ) -> Result<bool> {
        if self.checked || self.running.is_some() {
            return Ok(false);
        }
        let Some(storage) = self.storage.clone() else {
            return Ok(false);
        };
        self.checked = true;
        let limit = resources.client.now()?.checked_add(PACK_UPDATE_LIMIT)?;
        let deadline = if limit < until { limit } else { until };
        let spec = CoordinatorBackend::spec(
            resources,
            Operation::Infrastructure,
            JobClass::InfrastructureLifecycle,
            STARTUP_UPDATE_RESERVATION_BYTES + update_http::RESERVATION_BYTES,
            Some(deadline),
        )?;
        let path = folder.to_owned();
        let scope = if managed { scope } else { resources.cleanup };
        let job = resources.client.submit_blocking(scope, spec, move |context| {
            Ok(PackJobResult::Update(run_update(&path, &storage, managed, &context)))
        })?;
        tracing::info!(operation = "pack_update", resource = ?key, managed, stage = "Submitted",
            "The SugarCubes startup update began");
        // Only a managed backend's pack is judged; an attached one is never written to.
        if managed {
            self.folder = Some(folder.to_owned());
        }
        let gates = managed.then_some(key);
        self.running = Some(Running { job, scope: scope.id(), spec, gates });
        Ok(managed)
    }

    /// Record one observation of the version the managed backend runs.
    pub(super) fn observe(
        &mut self,
        event: TrialEvent,
        resources: &BackendResources<'_>,
    ) -> Result<()> {
        self.enqueue(PackEffect::Observe(event), resources)
    }

    /// The managed backend ended before it was ready.
    pub(super) fn backend_stopped(&mut self, resources: &BackendResources<'_>) -> Result<()> {
        self.enqueue(PackEffect::BackendStopped, resources)
    }

    fn enqueue(&mut self, effect: PackEffect, resources: &BackendResources<'_>) -> Result<()> {
        if self.folder.is_none() {
            return Ok(());
        }
        self.queued.push_back(effect);
        self.submit_queued(resources)
    }

    /// Adopt a finished job; returns the record whose inspection and launch it released.
    pub(super) fn advance(
        &mut self,
        resources: &BackendResources<'_>,
    ) -> Result<Option<ResourceKey>> {
        let Some(running) = &mut self.running else {
            return self.submit_queued(resources).map(|()| None);
        };
        let Some(completion) = running.job.try_take_completion() else {
            return Ok(None);
        };
        let released = running.gates;
        let original = completion.scope_id == running.scope && completion.spec == running.spec;
        self.running = None;
        match completion.outcome {
            JobOutcome::Success(PackJobResult::Update(outcome)) if original => {
                tracing::info!(operation = "pack_update", stage = "Terminal", outcome = ?outcome,
                    "The SugarCubes startup update settled");
            }
            JobOutcome::Success(PackJobResult::Stopped(Some(restored))) if original => {
                tracing::warn!(operation = "pack_update", stage = "Restored", version = %restored.version,
                    restored = %restored.restored_version, "The previous SugarCubes version was restored");
            }
            JobOutcome::Success(_) if original => {}
            other => {
                let stopped = matches!(other, JobOutcome::Cancelled | JobOutcome::TimedOut);
                tracing::warn!(
                    operation = "pack_update",
                    stage = "Terminal",
                    stopped,
                    gating = released.is_some(),
                    "A SugarCubes update job did not finish; the installed version is used"
                );
            }
        }
        self.submit_queued(resources)?;
        Ok(released)
    }

    fn submit_queued(&mut self, resources: &BackendResources<'_>) -> Result<()> {
        if self.running.is_some() {
            return Ok(());
        }
        let (Some(effect), Some(folder), Some(storage)) =
            (self.queued.pop_front(), self.folder.clone(), self.storage.clone())
        else {
            return Ok(());
        };
        let until = resources.client.now()?.checked_add(PACK_OBSERVATION_LIMIT)?;
        let spec = CoordinatorBackend::spec(
            resources,
            Operation::Infrastructure,
            JobClass::InfrastructureLifecycle,
            PACK_OBSERVATION_BYTES,
            Some(until),
        )?;
        let job = resources.client.submit_blocking(resources.cleanup, spec, move |_context| {
            let update = StartupUpdate::at(&folder, &storage)
                .map_err(|cause| PackJobError(cause.to_string()))?;
            match effect {
                PackEffect::Observe(event) => update
                    .observe(event)
                    .map(|()| PackJobResult::Observed)
                    .map_err(|cause| PackJobError(cause.to_string())),
                PackEffect::BackendStopped => update
                    .backend_stopped()
                    .map(PackJobResult::Stopped)
                    .map_err(|cause| PackJobError(cause.to_string())),
            }
        })?;
        self.running = Some(Running { job, scope: resources.cleanup.id(), spec, gates: None });
        Ok(())
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
