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
            (Applies::Create(base), Product::Created(mut scene)) => {
                if self.inputs.by_identity(identity).is_none_or(|surface| surface.loaded) {
                    self.inputs.retiring.scenes.push(scene);
                    return Ok(());
                }
                if let Some(surface) = self.inputs.by_identity_mut(identity) {
                    surface.adopt_restored(&mut scene, &memory)?;
                }
                let now = self.navigation_time();
                let viewport =
                    Viewport::new(self.document.tools.navigation().viewport().surface())?;
                let mut document = Box::new(document::DocumentState::new(viewport));
                document.tools.set_refresh_rate(self.display_refresh)?;
                document.tools.set_content(
                    Some(scene.document.snapshot().extent()),
                    Some(&scene.document),
                    now,
                )?;
                document.scene = Some(scene);
                document.input = Some(identity);
                document.initial_preview = true;
                let surface =
                    self.inputs.by_identity_mut(identity).ok_or("Input surface vanished")?;
                surface.reconcile(&mut document, &memory)?;
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
                    if !matches!(outcome, InitialOutcome::Failed(_)) {
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
        let replaced = self
            .with_input_document(identity, |surface, document, current| {
                let rebased = surface.rebase(document, base, &memory, now);
                if current && let Some(scene) = document.scene.as_mut() {
                    scene.source_revision = selection;
                }
                let replaced = rebased?;
                surface.loaded_base = Some(loaded);
                surface.failure = None;
                Ok::<_, Box<dyn Error>>(replaced)
            })
            .ok_or("Input document is unavailable")??;
        self.inputs.retiring.sources.push(replaced);
        Ok(())
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
