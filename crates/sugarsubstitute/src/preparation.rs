// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose governed raster ownership with current-state application publication.

use crate::job_sequence::JobSequence;
use crate::preparation_task::RenderConsole;
use crate::{
    preparation_task::{
        FramePreparation, InitializationPreparation, PendingPreparation, PreparationFailure,
        PreparationFrame, PreparationOutput,
    },
    render_transfer::PreparationTransfer,
};
use std::error::Error;
use sugarsubstitute_application::presentation::{FramePublicationDecision, FramePublicationGate};
use sugarsubstitute_contracts::lifecycle::ResourceKey;
use sugarsubstitute_contracts::{
    execution::{
        AdmissionError, CompletionBatch, ExecutionPort, Generation, IdentityError, JobClass,
        JobSpec, Operation, OwnedJob, ReplacementKey, Scope, SurfaceId,
    },
    presentation::{FrameIdentity, FrameShape, PreparedFrame},
};
use sugarsubstitute_execution::ExecutionClient;
use sugarsubstitute_splash::{
    AnimationFrame, Appearance, ProgressFrame, SPLASH_CACHE_RESERVATION_BYTES, SplashRenderRequest,
    SplashRenderer,
};

/// Select the first accepted raster's owned presentation coverage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FirstFrameContent {
    /// Include status text and all native control chrome before startup admission.
    Complete,
    /// Qualify the renderer's explicit character-first demonstration path.
    Character,
}

#[path = "preparation_delivery.rs"]
mod delivery;

/// Retain the admitted identity independently of the result's untrusted metadata.
struct ActivePreparation {
    job: OwnedJob<PreparationOutput, PreparationFailure>,
    identity: Option<FrameIdentity>,
    bootstrap: bool,
    // Actual accepted request remains retained until its observed terminal disposal, even if uninvoked.
    _request: Option<PreparationTransfer<SplashRenderRequest<RenderConsole>>>,
}

#[cfg(test)]
#[path = "preparation_tests.rs"]
mod tests;

/// Own one active preparation and one latest immutable request for this surface.
///
/// Cache resources move only when an admitted worker invokes its factory. Ordinary
/// completed preparation returns cache ownership; hidden or stale pixels are
/// rejected separately. Render retirement remains observable while the composition
/// owner stays alive.
pub(crate) struct PreparationDriver {
    locale: String,
    gate: FramePublicationGate,
    client: ExecutionClient,
    scope: Scope,
    jobs: JobSequence,
    active: Option<ActivePreparation>,
    renderer: Option<PreparationTransfer<SplashRenderer>>,
    pending: Option<PendingPreparation>,
    console_binding: Option<(ResourceKey, u64)>,
    console_presentation: sugarsubstitute_contracts::presentation::ConsolePresentation,
    next_identity: Generation,
    state_dirty: bool,
    closing: bool,
    bootstrap_published: bool,
    first_frame: FirstFrameContent,
    enriched: bool,
}

impl PreparationDriver {
    /// Bind an exclusive render scope and the composition owner's shared job sequence.
    ///
    /// This constructor performs no decoding or worker creation. The caller owns
    /// execution, mailbox routing, original shutdown budget and actual worker joins.
    /// The supplied scope must belong to `client`; real admission rejects foreign
    /// or closed scopes before a render factory can acquire cache ownership.
    /// `console_presentation` assigns the sole text owner for this surface lifetime;
    /// Native mode admits the empty native peer independently of source availability.
    pub(crate) fn new(
        locale: &str,
        client: ExecutionClient,
        scope: Scope,
        jobs: JobSequence,
        console_presentation: sugarsubstitute_contracts::presentation::ConsolePresentation,
        first_frame: FirstFrameContent,
    ) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            locale: locale.to_owned(),
            gate: FramePublicationGate::new(
                SurfaceId::new(1)?,
                FrameShape::new(558, 558, 1.0)?,
                Generation::new(1)?,
                false,
            ),
            client,
            scope,
            jobs,
            active: None,
            renderer: None,
            pending: None,
            console_binding: None,
            console_presentation,
            next_identity: Generation::new(1)?,
            state_dirty: false,
            closing: false,
            bootstrap_published: false,
            first_frame,
            enriched: false,
        })
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
        console: Option<RenderConsole>,
    ) -> Result<(), Box<dyn Error>> {
        let binding = console.as_ref().map(|view| (view.resource(), view.revision()));
        if matches!((self.console_binding,binding), (Some((old_resource,old_revision)),Some((resource,revision)))
            if old_resource == resource && revision < old_revision)
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Console revision regressed",
            )
            .into());
        }
        if self.console_binding != binding {
            self.state_dirty = true;
        }
        self.synchronize_state()?;
        self.console_binding = binding;
        self.gate.resize(shape)?;
        if self.closing || !self.gate.visible() {
            return Ok(());
        }
        let sequence = self.allocate_identity()?;
        // Native text has its own affine copied-document owner. Raster work needs
        // only the accepted source revision, not an extra immutable snapshot credit.
        let console = if self.console_presentation
            == sugarsubstitute_contracts::presentation::ConsolePresentation::Native
        {
            None
        } else {
            console
        };
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
            bootstrap: !self.bootstrap_published
                && self.first_frame == FirstFrameContent::Character,
            console,
            console_presentation: self.console_presentation,
        };
        self.pending = Some(PendingPreparation::new(request));
        self.submit_pending()
    }

    /// Allocate surface sample and replacement revisions across semantic state changes.
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
        let request = self.pending.as_ref().ok_or("Pending preparation absent")?;
        let identity = request.identity;
        let bytes = request.bytes;
        let input = request.input.clone();
        let bootstrap =
            !self.bootstrap_published && self.first_frame == FirstFrameContent::Character;
        let reservation_bytes = if initializing { SPLASH_CACHE_RESERVATION_BYTES } else { bytes };
        let revision = self.allocate_identity()?;
        let spec = JobSpec {
            id: self.jobs.allocate()?,
            operation: Operation::FramePrepare,
            class: JobClass::CpuInteractive,
            deadline: None,
            reservation_bytes,
            replacement: Some(ReplacementKey { surface: self.gate.surface(), revision }),
        };
        let client = &self.client;
        let scope = &self.scope;
        let (admission, identity, bootstrap, retained) = if initializing {
            let locale = self.locale.clone();
            (
                client.submit_cpu(scope, spec, move |context| {
                    InitializationPreparation::new(locale, context.memory().clone())
                }),
                None,
                false,
                None,
            )
        } else {
            let retained = input.clone();
            let transfer = self.renderer.as_ref().ok_or("Idle renderer absent")?.clone();
            // A rejected factory drops this cheap handle; the retained transfer still owns the cache.
            let job = client.submit_cpu(scope, spec, move |context| {
                FramePreparation::new(transfer, input, bootstrap, context.memory().clone())
            });
            (job, Some(identity), bootstrap, Some(retained))
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
        self.active = Some(ActivePreparation { job, identity, bootstrap, _request: retained });
        Ok(())
    }

    /// Consume the composition owner's bounded batch without draining its shared mailbox.
    ///
    /// Foreign notices remain available to their owners. Empty batches still retry
    /// pending admission after an execution-owned capacity wake.
    pub(crate) fn consume(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<Option<PreparedFrame>, Box<dyn Error>> {
        self.synchronize_state()?;
        let matching = self.active.as_ref().is_some_and(|active| {
            batch.notifications.iter().any(|notice| {
                notice.job_id == active.job.spec().id && notice.scope_id == self.scope.id()
            })
        });
        let result = if matching { self.take_terminal()? } else { None };
        self.submit_pending()?;
        Ok(result)
    }

    /// Close rendering admission and cancel only its scope, retaining actual result ownership.
    pub(crate) fn request_retirement(&mut self) -> Result<(), Box<dyn Error>> {
        if self.closing {
            return Ok(());
        }
        self.closing = true;
        self.pending = None;
        self.scope.cancel();
        self.gate.set_visible(false)?;
        Ok(())
    }

    /// Observe rendering settlement after its completion was routed by the composition owner.
    ///
    /// Retained frame/cache byte charges do not impersonate queued or running work.
    pub(crate) fn poll_retirement(&mut self) -> Result<bool, Box<dyn Error>> {
        let snapshot = self.scope.snapshot();
        Ok(self.closing
            && self.active.is_none()
            && snapshot.queued == 0
            && snapshot.running == 0
            && snapshot.terminal_unobserved == 0
            && snapshot.children_live == 0)
    }

    /// Release idle cache only after actual rendering settlement; execution joins belong to root.
    pub(crate) fn finish_retirement(&mut self) -> Result<(), Box<dyn Error>> {
        if !self.poll_retirement()? {
            return Err("Preparation retirement is unsettled".into());
        }
        self.renderer = None;
        Ok(())
    }
}
