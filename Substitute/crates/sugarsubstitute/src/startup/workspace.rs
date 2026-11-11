// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain durable workspace work in the application's existing execution and native lifetime.
#[cfg(test)]
mod closing_tests;
mod integration;
mod jobs;
mod media;
mod observations;
#[cfg(test)]
mod open_tests;
mod publication;
#[cfg(feature = "native-composition")]
mod sources;
use super::*;
use crate::workspace_session::{
    self, DriverEvent, OpenedProfile, ProfileStore, RestoreFailure, StoreError, WorkspaceDriver,
    WorkspaceStore,
};
use std::{collections::VecDeque, path::PathBuf, sync::Mutex};
use sugarsubstitute_application::workspace_session::CaptureTicket;
use sugarsubstitute_contracts::{
    desktop_session::wire::*,
    execution::*,
    startup::{MainIdentity, StartupEffect},
};
use sugarsubstitute_desktop::{SessionBootstrap, SessionCommand};
use sugarsubstitute_execution::ExecutionClient;

/// Independent admission for loaded state, one incoming checkpoint, publication scratch and
/// two bounded media responses. This ceiling reserves capacity, not physical allocations.
pub(super) const CAPACITY: u64 = 5 * 1024 * 1024 * 1024;
const DRIVER_BYTES: u64 = WorkspaceDriver::reservation_bytes();
const DELIVERY_BYTES: u64 = 24 * 1024 * 1024;
const LOAD_BYTES: u64 = WorkspaceStore::reservation_bytes()
    + ProfileStore::reservation_bytes()
    + DRIVER_BYTES
    + DELIVERY_BYTES;
struct Loaded {
    profile: OpenedProfile,
    restored: workspace_session::RestoredWorkspace,
    bootstrap: SessionBootstrap,
    driver_memory: MemoryLease,
    delivery_memory: MemoryLease,
    input_plan: crate::input_documents::InputRestorePlan,
}
struct Published {
    captured: workspace_session::CapturedWorkspace,
    receipt: workspace_session::SaveReceipt,
}
struct Decoded {
    captured: workspace_session::CapturedWorkspace,
    association: Option<crate::input_documents::InputRestorePlan>,
}
enum Active {
    Encode {
        request: String,
        transfer: String,
        job: OwnedJob<workspace_session::EncodedMaterialization, StoreError>,
    },
    Decode {
        request: String,
        ticket: CaptureTicket,
        job: OwnedJob<Decoded, StoreError>,
    },
    Publish {
        ticket: CaptureTicket,
        job: OwnedJob<Published, StoreError>,
    },
}
pub(super) struct WorkspaceWork {
    user: PathBuf,
    spec: ScopeSpec,
    jobs: JobSequence,
    client: Option<ExecutionClient>,
    scope: Option<Scope>,
    context: Option<WorkspaceContext>,
    deferred: Option<StartupEffect>,
    pending_open: Option<(WorkspaceContext, StartupEffect)>,
    load: Option<OwnedJob<Loaded, StoreError>>,
    profile: Option<Arc<Mutex<OpenedProfile>>>,
    input_directory: Option<PathBuf>,
    input_plan: crate::input_documents::InputRestorePlan,
    input_association: Option<crate::input_documents::InputRestorePlan>,
    store: Option<WorkspaceStore>,
    media: media::MediaWork,
    #[cfg(feature = "native-composition")]
    deferred_surface: VecDeque<sugarsubstitute_desktop::SurfaceObservation>,
    input_generation: Option<u64>,
    capture: Option<(CaptureTicket, workspace_session::CapturedWorkspace)>,
    driver: Option<WorkspaceDriver>,
    delivery: Option<MemoryLease>,
    bootstrap: Option<SessionBootstrap>,
    ready: bool,
    active: Option<Active>,
    pending: Option<DriverEvent>,
    publish_input: Option<publication::Input>,
    retirements: Arc<Mutex<Vec<Arc<sugarsubstitute_comfy::output_media::ComfyOutputTransfer>>>>,
    retirement_job: Option<OwnedJob<(), StoreError>>,
    messages: VecDeque<DesktopStartupCommand>,
    close_sample: Option<std::num::NonZeroU64>,
    closing: Option<Moment>,
    retirement_until: Option<Moment>,
    retired: bool,
}
impl WorkspaceWork {
    pub(super) fn new(user: PathBuf, spec: ScopeSpec, jobs: JobSequence) -> Self {
        Self {
            user,
            spec,
            jobs,
            client: None,
            scope: None,
            context: None,
            deferred: None,
            pending_open: None,
            load: None,
            profile: None,
            input_directory: None,
            input_plan: crate::input_documents::InputRestorePlan::Current,
            input_association: None,
            store: None,
            media: media::MediaWork::new(),
            #[cfg(feature = "native-composition")]
            deferred_surface: VecDeque::new(),
            input_generation: None,
            capture: None,
            driver: None,
            delivery: None,
            bootstrap: None,
            ready: false,
            active: None,
            pending: None,
            publish_input: None,
            retirements: Arc::new(Mutex::new(vec![])),
            retirement_job: None,
            messages: VecDeque::new(),
            close_sample: None,
            closing: None,
            retirement_until: None,
            retired: false,
        }
    }
    pub(super) fn scope_id(&self) -> ScopeId {
        self.spec.id
    }
    pub(super) fn start(&mut self, client: ExecutionClient) -> Result<(), Box<dyn Error>> {
        if self.client.is_some() {
            return Err("Workspace execution already started".into());
        }
        self.scope = Some(client.create_scope(self.spec, None)?);
        self.client = Some(client);
        Ok(())
    }
    /// Queue the workspace open. It starts once the previous workspace's workers have each
    /// reached an observed terminal outcome; waiting for them has no cutoff, so a slow
    /// predecessor delays the open instead of failing it.
    pub(super) fn begin(
        &mut self,
        context: WorkspaceContext,
        effect: StartupEffect,
    ) -> Result<(), Box<dyn Error>> {
        if self.closing.is_some()
            || self.pending_open.is_some()
            || self.load.is_some()
            || self.deferred.is_some()
        {
            return Err("Workspace main load already owns its original operation".into());
        }
        self.pending_open = Some((context, effect));
        Ok(())
    }
    /// Submit the load, which ends on its observed outcome like the workers it waited for.
    fn open(
        &mut self,
        context: WorkspaceContext,
        effect: StartupEffect,
    ) -> Result<(), Box<dyn Error>> {
        self.messages.clear();
        self.media = media::MediaWork::new();
        self.driver = None;
        self.profile = None;
        self.delivery = None;
        self.bootstrap = None;
        self.ready = false;
        self.retired = false;
        let profile = ProfileStore::new(
            &self.user,
            sugarsubstitute_contracts::desktop_session::InstallationId::new(
                context.installation.as_str().to_owned(),
            )?,
            sugarsubstitute_contracts::desktop_session::ProfileId::new(
                context.profile.as_str().to_owned(),
            )?,
        )?;
        let binding = context.backend;
        let origin = context.origin;
        let user = self.user.clone();
        let preloader = self.media.preloader();
        let spec = self.job(LOAD_BYTES, None)?;
        let client = self.client.as_ref().ok_or("Workspace execution absent")?;
        let scope = self.scope.as_ref().ok_or("Workspace scope absent")?;
        self.load = Some(client.submit_blocking(scope,spec,move |job| {
            let profile = profile.open_worker(binding,&job)?;
            let mut restored = profile.workspace.load_worker(&job)?;
            restored.protected |= profile.protected();
            let input_plan = match &restored.checkpoint {
                Some((_,checkpoint)) => match crate::input_documents::workspace::restore(&profile.workspace.directory().join("input-canvas"),checkpoint,&job) {
                    Ok(Some(source)) => crate::input_documents::InputRestorePlan::Verified(source),
                    Ok(None) => crate::input_documents::InputRestorePlan::Current,
                    Err(cause) => {
                        tracing::warn!(operation="workspace_input_restore",cause=%cause,"Native archive association unavailable; original workspace protected");
                        restored.protected = true;
                        crate::input_documents::InputRestorePlan::Current
                    }
                },
                None if restored.protected => crate::input_documents::InputRestorePlan::Current,
                None => crate::input_documents::legacy::plan(&user, &profile.workspace.directory().join("input-canvas"), &profile.scope, &job).map_err(|cause| StoreError::Storage(std::io::Error::other(cause)))?,
            };
            let webview = sugarsubstitute_supervision::new_artifact_identity(&job)?;
            let bootstrap = SessionBootstrap::new(profile.scope.clone(),origin,webview)?.with_media(true);
            let driver_memory = job.memory().claim(DRIVER_BYTES).map_err(std::io::Error::other)?.into_lease();
            let delivery_memory = job.memory().claim(DELIVERY_BYTES).map_err(std::io::Error::other)?.into_lease();
            preloader.prepare(&profile.workspace, &restored, &delivery_memory, &job)?;
            Ok(Loaded { profile,restored,bootstrap,driver_memory,delivery_memory,input_plan })
        })?);
        self.context = Some(context);
        self.deferred = Some(effect);
        Ok(())
    }
    /// Bind one worker. Without a cutoff it is open-ended: the storage it calls demands a finite
    /// admitted deadline, but only cancellation at shutdown ends it early.
    fn job(&self, bytes: u64, until: Option<Moment>) -> Result<JobSpec, Box<dyn Error>> {
        Ok(JobSpec {
            id: self.jobs.allocate()?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(until.unwrap_or(crate::job_deadline::OPEN_ENDED)),
            reservation_bytes: bytes,
            replacement: None,
        })
    }
    pub(super) fn take_ready(
        &mut self,
    ) -> Option<(
        StartupEffect,
        SessionBootstrap,
        Option<SessionPlacement>,
        PathBuf,
        crate::input_documents::InputRestorePlan,
    )> {
        if !self.ready {
            return None;
        }
        self.ready = false;
        Some((
            self.deferred.take()?,
            self.bootstrap.clone()?,
            self.driver.as_ref()?.placement().cloned(),
            self.input_directory.clone()?,
            self.input_plan.clone(),
        ))
    }
    pub(super) fn next_command(&mut self) -> Result<Option<DesktopStartupCommand>, Box<dyn Error>> {
        if let Some(command) = self.messages.pop_front() {
            return Ok(Some(command));
        }
        if self.close_sample.is_some() || self.retired {
            return Ok(None);
        }
        let Some(driver) = &mut self.driver else {
            return Ok(None);
        };
        let Some(message) = driver.next_message() else {
            return Ok(None);
        };
        let Some(memory) = &self.delivery else {
            driver.return_message(message)?;
            return Ok(None);
        };
        let claim = match memory.claim(SessionCommand::STORAGE_BYTES) {
            Ok(claim) => claim,
            Err(_) => {
                driver.return_message(message)?;
                return Ok(None);
            }
        };
        let context = self.context.as_ref().ok_or("Workspace native lifetime absent")?;
        Ok(Some(DesktopStartupCommand::SendSession(SessionCommand::new(
            context.identity,
            message,
            claim,
            driver.scope(),
            &self.bootstrap.as_ref().ok_or("Workspace bootstrap absent")?.origin_text(),
        )?)))
    }
    pub(super) fn deadline(&self) -> Option<Moment> {
        self.driver
            .as_ref()
            .filter(|_| !self.retired)
            .and_then(WorkspaceDriver::deadline)
            .into_iter()
            .chain(self.closing.filter(|_| !self.retired))
            .min()
    }
    pub(super) fn close_ready(&self, now: Moment) -> bool {
        self.driver
            .as_ref()
            .is_none_or(|driver| self.close_sample.is_none() && driver.close_ready(now))
            || self.closing.is_some_and(|cutoff| now >= cutoff)
    }
    pub(super) fn retain_retirement_until(&mut self, until: Moment) {
        self.retirement_until =
            Some(self.retirement_until.map_or(until, |previous| previous.min(until)));
    }
    pub(super) fn begin_close(
        &mut self,
        now: Moment,
        cutoff: Moment,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(original) = self.closing {
            self.closing = Some(original.min(cutoff));
            return Ok(());
        }
        self.closing = Some(cutoff);
        self.pending_open = None;
        self.deferred = None;
        if let Some(driver) = &mut self.driver {
            if now >= cutoff {
                return Ok(());
            }
            driver.begin_close(now, cutoff)?;
            let capture_id = std::num::NonZeroU64::MIN;
            self.close_sample = Some(capture_id);
            self.messages.push_back(DesktopStartupCommand::CaptureMainPlacement {
                identity: self.context.as_ref().ok_or("Workspace main absent")?.identity,
                capture_id,
            });
        } else if let Some(job) = &self.load {
            job.cancel();
        }
        Ok(())
    }
    pub(super) fn settled(&self) -> bool {
        self.pending_open.is_none()
            && self.load.is_none()
            && self.active.is_none()
            && self.pending.is_none()
            && self.publish_input.is_none()
            && self.capture.is_none()
            && self.retirement_job.is_none()
            && self.retirements.try_lock().is_ok_and(|owners| owners.is_empty())
            && self.media.settled()
    }
}
