// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Apply what a finished input job produced, or keep its failure for the request's reply.
use super::{
    build::{MaskImport, Product},
    jobs::Applies,
    reconcile::NewBase,
    *,
};
use sugarsubstitute_canvas::{LayerContent, Provision};

impl Session {
    /// Keep a failure where its request's reply will report it.
    pub(super) fn record_input_failure(
        &mut self,
        identity: InputDocumentId,
        applies: Applies,
        failure: InputFailure,
    ) {
        tracing::warn!(operation = "canvas_input_work", document_id = identity.get(), cause = %failure, "Input canvas work failed");
        match applies {
            Applies::Create(_) | Applies::Rebase(_) => {
                if let Some(surface) = self.inputs.by_identity_mut(identity) {
                    surface.failure = Some(failure);
                }
            }
            Applies::Import { layer, .. } => self.fail_input_import(identity, layer, failure),
            Applies::Resize { ticket, .. } => self.fail_input_ticket(ticket, identity, failure),
        }
    }

    pub(super) fn fail_input_import(
        &mut self,
        identity: InputDocumentId,
        layer: LayerId,
        failure: InputFailure,
    ) {
        if let Some(mask) = self
            .inputs
            .by_identity_mut(identity)
            .and_then(|surface| surface.masks.iter_mut().find(|mask| mask.layer == layer))
        {
            mask.outcome = Some(InitialOutcome::Failed(failure));
        }
    }

    pub(super) fn fail_input_ticket(
        &mut self,
        ticket: u64,
        identity: InputDocumentId,
        failure: InputFailure,
    ) {
        let Some(key) = self.inputs.by_identity(identity).map(|surface| surface.key.clone()) else {
            return;
        };
        if let Some(pending) = self.inputs.tickets.iter_mut().find(|pending| pending.id == ticket) {
            pending.failures.push((key, failure));
        }
    }

    /// Apply one current product to its surface's document.
    pub(super) fn apply_input_product(
        &mut self,
        identity: InputDocumentId,
        applies: Applies,
        product: Product,
    ) -> Result<(), Box<dyn Error>> {
        let memory = self.resources.as_ref().ok_or("Canvas storage is unavailable")?.memory.clone();
        match (applies, product) {
            (Applies::Create(base), Product::Created(scene)) => {
                if self.inputs.by_identity(identity).is_none_or(|surface| surface.loaded) {
                    self.inputs.retiring.scenes.push(scene);
                    return Ok(());
                }
                let viewport =
                    match Viewport::new(self.document.tools.navigation().viewport().surface()) {
                        Ok(viewport) => viewport,
                        Err(cause) => {
                            self.inputs.retiring.scenes.push(scene);
                            return Err(cause.into());
                        }
                    };
                let mut document = Box::new(document::DocumentState::new(viewport));
                document.scene = Some(scene);
                document.input = Some(identity);
                document.initial_preview = true;
                // A document that cannot be completed, as when the memory budget cannot hold
                // its masks, still holds its source and retires on the worker.
                if let Err(cause) = self.complete_created(identity, &mut document, &memory) {
                    self.inputs.retiring.documents.push(document);
                    return Err(cause);
                }
                let surface =
                    self.inputs.by_identity_mut(identity).ok_or("Input surface vanished")?;
                surface.document = Some(document);
                surface.loaded = true;
                surface.loaded_base = Some(base);
                surface.failure = None;
            }
            (Applies::Rebase(reference), Product::Rebased { source, original }) => {
                self.rebase_input(
                    identity,
                    NewBase::Raster { source, original },
                    Base::File(reference),
                )?;
            }
            (Applies::Import { layer, reference, expected }, Product::Imported(import)) => {
                let outcome = match import {
                    MaskImport::Mismatch(actual) => InitialOutcome::Mismatch { expected, actual },
                    MaskImport::Loaded(coverage) => {
                        let current = self
                            .input_document(identity)
                            .and_then(|document| document.scene.as_ref())
                            .map(|scene| scene.document.snapshot().extent());
                        match current {
                            Some(extent) if extent == coverage.extent() => {
                                let (document, _) = self
                                    .input_document_mut(identity)
                                    .ok_or("Input document vanished")?;
                                let scene =
                                    document.scene.as_mut().ok_or("Input document vanished")?;
                                scene
                                    .document
                                    .provision(&[Provision::Coverage { layer, coverage }])?;
                                InitialOutcome::Loaded
                            }
                            Some(extent) => InitialOutcome::Mismatch {
                                expected: extent,
                                actual: coverage.extent(),
                            },
                            None => InitialOutcome::Failed(InputFailure::message(
                                "The mask's document is unavailable",
                            )),
                        }
                    }
                };
                if let Some(mask) = self
                    .inputs
                    .by_identity_mut(identity)
                    .and_then(|surface| surface.masks.iter_mut().find(|mask| mask.layer == layer))
                {
                    // An import the loader no longer asks for loads nothing a later pick must skip.
                    if !matches!(outcome, InitialOutcome::Failed(_))
                        && mask.desired_initial.as_ref() == Some(&reference)
                    {
                        mask.loaded_initial = Some(reference);
                    }
                    mask.outcome = Some(outcome);
                }
            }
            (Applies::Resize { ticket, revision, extent }, Product::Resized(masks)) => {
                let label = self.inputs.by_identity(identity).map(|surface| surface.label.clone());
                let snapshot = self
                    .input_document(identity)
                    .and_then(|document| document.scene.as_ref())
                    .map(|scene| scene.document.snapshot());
                let Some((label, snapshot)) = label.zip(snapshot) else {
                    return Ok(());
                };
                if snapshot.revision() != revision {
                    self.fail_input_ticket(
                        ticket,
                        identity,
                        InputFailure::message("The document changed during the resize"),
                    );
                    return Ok(());
                }
                let mut layers = vec![build::blank_base(extent, &label)?];
                for layer in snapshot.layers().iter().filter(|layer| layer.id().get() != BASE_LAYER)
                {
                    let resized = masks.iter().find(|(id, _)| *id == layer.id());
                    match (layer.content(), resized) {
                        (LayerContent::Coverage { appearance, .. }, Some((_, coverage))) => {
                            let content = LayerContent::Coverage {
                                coverage: coverage.clone(),
                                appearance: *appearance,
                            };
                            layers.push(layer.clone().with_content(content));
                        }
                        _ => return Err("A resized document lost a mask".into()),
                    }
                }
                let now = self.navigation_time();
                let (document, _) =
                    self.input_document_mut(identity).ok_or("Input document vanished")?;
                let scene = document.scene.as_mut().ok_or("Input document vanished")?;
                scene.document.reframe(extent, &layers)?;
                let scene = document.scene.as_ref().ok_or("Input document vanished")?;
                document.tools.set_content(Some(extent), Some(&scene.document), now)?;
            }
            (_, product) => {
                self.inputs.retiring.products.push(product);
                return Err("Input job produced another kind of result".into());
            }
        }
        Ok(())
    }

    /// Give a newly created document its archived content, tools and synced masks.
    fn complete_created(
        &mut self,
        identity: InputDocumentId,
        document: &mut document::DocumentState,
        memory: &MemoryLease,
    ) -> Result<(), Box<dyn Error>> {
        let now = self.navigation_time();
        let refresh = self.display_refresh;
        let surface = self.inputs.by_identity_mut(identity).ok_or("Input surface vanished")?;
        let scene = document.scene.as_mut().ok_or("Input document vanished")?;
        surface.adopt_restored(scene, memory)?;
        document.tools.set_refresh_rate(refresh)?;
        let scene = document.scene.as_ref().ok_or("Input document vanished")?;
        let extent = scene.document.snapshot().extent();
        document.tools.set_content(Some(extent), Some(&scene.document), now)?;
        surface.reconcile(document, memory)
    }

    /// Replace a loaded document's base, keeping its surface's record of what it holds.
    pub(super) fn rebase_input(
        &mut self,
        identity: InputDocumentId,
        base: NewBase,
        loaded: Base,
    ) -> Result<(), Box<dyn Error>> {
        let memory = self.resources.as_ref().ok_or("Canvas storage is unavailable")?.memory.clone();
        let now = self.navigation_time();
        let current = self.document.input == Some(identity);
        if current {
            self.rebind_input_source()?;
        }
        let selection = self.selection;
        if self.inputs.by_identity(identity).is_none() {
            return Ok(());
        }
        let (replaced, outcome) = self
            .with_input_document(identity, |surface, document, current| {
                let rebased = surface.rebase(document, base, &memory, now);
                if current && let Some(scene) = document.scene.as_mut() {
                    scene.source_revision = selection;
                }
                // Whichever source the document does not keep retires on the worker.
                match rebased {
                    Ok(replaced) => {
                        surface.loaded_base = Some(loaded);
                        surface.failure = None;
                        (replaced, Ok(()))
                    }
                    Err(failure) => (failure.unadopted, Err(failure.cause)),
                }
            })
            .ok_or("Input document is unavailable")?;
        self.inputs.retiring.sources.push(replaced);
        outcome
    }

    /// Fence the shown input document's frames before its source changes, under a new
    /// selection so its renderer is rebuilt for the new source.
    fn rebind_input_source(&mut self) -> Result<(), Box<dyn Error>> {
        let next = self.source_sequence.checked_add(1).ok_or("Canvas source revision exhausted")?;
        self.abandon_cpu_frame(self.navigation_time())?;
        #[cfg(windows)]
        self.abandon_composition_frame(self.navigation_time())?;
        if let Some(rendering) = &mut self.rendering {
            rendering.superseded = true;
            rendering.job.cancel();
        }
        if let Some(displaying) = &self.displaying {
            displaying.job.cancel();
        }
        if let Some(presenting) = &self.presenting {
            presenting.job.cancel();
        }
        self.source_sequence = next;
        self.selection = next;
        Ok(())
    }
}
