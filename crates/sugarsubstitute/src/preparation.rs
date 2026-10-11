// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose governed raster ownership with current-state application publication.

use crate::{
    preparation_task::{
        FramePreparation, InitializationPreparation, PreparationFailure, PreparationOutput,
    },
    render_transfer::RendererTransfer,
};
use std::{error::Error, num::NonZeroUsize, sync::Arc, time::Duration};
use sugarsubstitute_application::presentation::{FramePublicationDecision, FramePublicationGate};
use sugarsubstitute_contracts::{
    execution::{
        AdmissionError, Clock, CompletionWake, ExecutionPort, Generation, IdentityError, JobClass,
        JobId, JobSpec, Operation, OwnedJob, ReplacementKey, Scope, ScopeId, ScopeSpec, SurfaceId,
    },
    presentation::{FrameIdentity, FrameShape, PreparedFrame},
};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner, MonotonicClock};
use sugarsubstitute_splash::{
    AnimationFrame, Appearance, ProgressFrame, SPLASH_CACHE_RESERVATION_BYTES, SplashRenderRequest,
    SplashRenderer,
};

#[path = "preparation_delivery.rs"]
mod delivery;

/// Retain the admitted identity independently of the result's untrusted metadata.
struct ActivePreparation {
    job: OwnedJob<PreparationOutput, PreparationFailure>,
    identity: Option<FrameIdentity>,
    bootstrap: bool,
}

#[cfg(test)]
#[path = "preparation_tests.rs"]
mod tests;

/// Own one active preparation and one latest plain-data request for this surface.
///
/// Cache resources move only when an admitted worker invokes its factory. Ordinary
/// completed preparation returns cache ownership; hidden or stale pixels are
/// rejected separately. Shutdown remains observable while the native loop lives.
pub(crate) struct PreparationDriver {
    locale: String,
    gate: FramePublicationGate,
    clock: Arc<MonotonicClock>,
    owner: Option<ExecutionOwner>,
    client: Option<ExecutionClient>,
    scope: Option<Scope>,
    active: Option<ActivePreparation>,
    renderer: Option<RendererTransfer<SplashRenderer>>,
    pending: Option<SplashRenderRequest>,
    next_identity: Generation,
    state_dirty: bool,
    closing: bool,
    bootstrap_published: bool,
    enriched: bool,
    shutdown_deadline_reported: bool,
}

impl PreparationDriver {
    /// Construct view state without decoding, fonts, native handles or worker creation.
    pub(crate) fn new(locale: &str) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            locale: locale.to_owned(),
            gate: FramePublicationGate::new(
                SurfaceId::new(1)?,
                FrameShape::new(558, 558, 1.0)?,
                Generation::new(1)?,
                false,
            ),
            clock: MonotonicClock::new(),
            owner: None,
            client: None,
            scope: None,
            active: None,
            renderer: None,
            pending: None,
            next_identity: Generation::new(1)?,
            state_dirty: false,
            closing: false,
            bootstrap_published: false,
            enriched: false,
            shutdown_deadline_reported: false,
        })
    }

    /// Create the sole executor after the native wake proxy exists; retain partial ownership.
    pub(crate) fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>> {
        if self.owner.is_some() || self.closing {
            return Err("Preparation lifetime cannot be started again".into());
        }
        self.owner = Some(ExecutionOwner::new(self.clock.clone(), wake)?);
        let client = self.owner.as_ref().ok_or("Execution owner absent")?.client();
        self.client = Some(client.clone());
        self.scope =
            Some(client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?);
        Ok(())
    }

    /// Mark semantic input before subsequently admitting or publishing any pixels.
    pub(crate) fn invalidate(&mut self) {
        self.state_dirty = true;
    }

    /// Apply a pending semantic revision without losing it on exhaustion.
    fn synchronize_state(&mut self) -> Result<(), IdentityError> {
        if self.state_dirty {
            self.gate.invalidate_state()?;
            self.state_dirty = false;
            self.pending = None;
        }
        Ok(())
    }

    /// Update authoritative visibility before any following request or completion.
    pub(crate) fn set_visible(&mut self, visible: bool) -> Result<(), Box<dyn Error>> {
        self.synchronize_state()?;
        self.gate.set_visible(visible && !self.closing)?;
        if !self.gate.visible() {
            self.pending = None;
        }
        Ok(())
    }

    /// Return whether the static view still needs its single enrichment operation.
    pub(crate) const fn enriched(&self) -> bool {
        self.enriched
    }

    /// Capture the latest lightweight request while retaining currently running work.
    pub(crate) fn request(
        &mut self,
        shape: FrameShape,
        appearance: Appearance,
        character: AnimationFrame,
        feedback: ProgressFrame,
    ) -> Result<(), Box<dyn Error>> {
        self.synchronize_state()?;
        self.gate.resize(shape)?;
        if self.closing || !self.gate.visible() {
            return Ok(());
        }
        let sequence = self.allocate_identity()?;
        let request = SplashRenderRequest {
            shape,
            appearance,
            character,
            feedback,
            identity: FrameIdentity {
                surface: self.gate.surface(),
                state_generation: self.gate.state_generation(),
                sequence,
            },
            bootstrap: !self.bootstrap_published,
        };
        self.pending = Some(request);
        self.submit_pending()
    }

    /// Allocate strictly increasing job/sample identities, including across state revisions.
    fn allocate_identity(&mut self) -> Result<Generation, IdentityError> {
        let current = self.next_identity;
        self.next_identity = current.next()?;
        Ok(current)
    }

    /// Reserve capacity before a worker can take cache ownership or allocate frame storage.
    fn submit_pending(&mut self) -> Result<(), Box<dyn Error>> {
        if self.active.is_some() || self.pending.is_none() || self.closing || !self.gate.visible() {
            return Ok(());
        }
        let initializing = self.renderer.is_none();
        let mut request = *self.pending.as_ref().ok_or("Pending preparation absent")?;
        request.bootstrap = !self.bootstrap_published;
        let reservation_bytes =
            if initializing { SPLASH_CACHE_RESERVATION_BYTES } else { request.reservation_bytes() };
        let revision = self.allocate_identity()?;
        let spec = JobSpec {
            id: JobId::new(revision.get())?,
            operation: Operation::FramePrepare,
            class: JobClass::CpuInteractive,
            deadline: None,
            reservation_bytes,
            replacement: Some(ReplacementKey { surface: self.gate.surface(), revision }),
        };
        let client = self.client.as_ref().ok_or("Preparation has not started")?;
        let scope = self.scope.as_ref().ok_or("Preparation scope absent")?;
        let (admission, identity, bootstrap) = if initializing {
            let locale = self.locale.clone();
            (
                client.submit_cpu(scope, spec, move |context| {
                    InitializationPreparation::new(locale, context.memory().clone())
                }),
                None,
                false,
            )
        } else {
            let identity = request.identity;
            let bootstrap = request.bootstrap;
            let transfer = self.renderer.as_ref().ok_or("Idle renderer absent")?.clone();
            // A rejected factory drops this cheap handle; the retained transfer still owns the cache.
            let job = client.submit_cpu(scope, spec, move |context| {
                FramePreparation::new(transfer, request, context.memory().clone())
            });
            (job, Some(identity), bootstrap)
        };
        let job = match admission {
            Ok(job) => job,
            Err(
                AdmissionError::QueueSaturated { .. }
                | AdmissionError::CompletionSaturated
                | AdmissionError::MemorySaturated { .. },
            ) => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        if !initializing {
            self.pending = None;
        }
        self.active = Some(ActivePreparation { job, identity, bootstrap });
        Ok(())
    }

    /// Drain bounded metadata and transfer actual results without waiting for execution.
    pub(crate) fn poll(
        &mut self,
        maximum: NonZeroUsize,
    ) -> Result<Option<PreparedFrame>, Box<dyn Error>> {
        self.synchronize_state()?;
        if let Some(client) = &self.client {
            client.drain_completions(maximum)?;
        }
        let result = self.take_terminal()?;
        self.submit_pending()?;
        Ok(result)
    }

    /// Close admission and cancel accepted work while keeping terminal disposal observable.
    pub(crate) fn request_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        if self.closing {
            return Ok(());
        }
        self.closing = true;
        self.pending = None;
        let visibility_result = self.gate.set_visible(false);
        if let Some(scope) = &self.scope {
            scope.cancel();
        }
        let deadline = self.clock.now().checked_add(Duration::from_secs(5));
        if let Some(owner) = &mut self.owner {
            owner.request_shutdown(
                deadline
                    .unwrap_or(sugarsubstitute_contracts::execution::Moment::from_nanos(u64::MAX)),
            );
        }
        visibility_result?;
        deadline?;
        Ok(())
    }

    /// Observe stopped jobs/workers; retained native/audit buffers do not impersonate work.
    pub(crate) fn poll_shutdown(&mut self) -> Result<bool, Box<dyn Error>> {
        if !self.closing {
            return Ok(false);
        }
        self.poll(NonZeroUsize::MIN)?;
        match &self.owner {
            Some(owner) => {
                let progress = owner.poll_shutdown()?;
                if progress.deadline_expired && !self.shutdown_deadline_reported {
                    self.shutdown_deadline_reported = true;
                    tracing::warn!(operation = "splash_shutdown", unresolved_jobs = ?progress.unresolved_job_ids,
                        queued = progress.queued, running = progress.running, "Owned shutdown deadline expired");
                }
                Ok(progress.settled)
            }
            None => Ok(true),
        }
    }

    /// Release idle cache and observe worker joins only after native window teardown.
    pub(crate) fn finish_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        if !self.poll_shutdown()? {
            return Err("Preparation shutdown is unsettled".into());
        }
        self.renderer = None;
        self.scope = None;
        self.client = None;
        if let Some(owner) = &mut self.owner {
            owner.finish_shutdown()?;
        }
        self.owner = None;
        Ok(())
    }
}
