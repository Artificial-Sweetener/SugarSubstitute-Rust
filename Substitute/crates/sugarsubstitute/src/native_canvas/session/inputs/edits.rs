// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Appearance, hover emphasis and canvas resolution changes on synced input documents.
use super::{
    jobs::{Target, Task, Work},
    *,
};
use sugarsubstitute_canvas::{DocumentEdit, LayerContent};

impl Session {
    /// Change mask overlay appearances as one undoable step in each affected document.
    /// Appearance never changes coverage or what a workflow captures.
    ///
    /// # Errors
    /// Rejects unknown surfaces or masks and documents that are not loaded with
    /// [`InputRejection`], changing nothing; preserves document and admission failures.
    pub(crate) fn set_input_appearance(
        &mut self,
        workflow: &str,
        changes: &[AppearanceChange],
    ) -> Result<(), Box<dyn Error>> {
        let mut grouped: Vec<(InputDocumentId, Vec<(LayerId, MaskAppearance)>)> = Vec::new();
        for change in changes {
            let surface =
                self.inputs.surface(workflow, &change.surface).ok_or(InputRejection::Unknown)?;
            let layer = surface.mask(&change.mask).ok_or(InputRejection::Unknown)?.layer;
            if self.input_document(surface.identity).is_none() {
                return Err(Box::new(InputRejection::Unavailable(
                    "The input document is not loaded",
                )));
            }
            match grouped.iter_mut().find(|(identity, _)| *identity == surface.identity) {
                Some((_, masks)) => masks.push((layer, change.appearance)),
                None => grouped.push((surface.identity, vec![(layer, change.appearance)])),
            }
        }
        for (identity, masks) in grouped {
            let session = self.inputs.edit_identity()?;
            let (document, _) =
                self.input_document_mut(identity).ok_or("Input document vanished")?;
            let scene = document.scene.as_mut().ok_or("Input document vanished")?;
            let snapshot = scene.document.snapshot();
            let mut edit = scene.document.begin_edit(session);
            for (layer, appearance) in masks {
                let current = snapshot
                    .layers()
                    .iter()
                    .find(|candidate| candidate.id() == layer)
                    .ok_or("Input mask vanished")?;
                let LayerContent::Coverage { coverage, .. } = current.content() else {
                    return Err("An input mask is not a coverage layer".into());
                };
                let content = LayerContent::Coverage { coverage: coverage.clone(), appearance };
                edit.stage(DocumentEdit::Replace(current.clone().with_content(content)))?;
            }
            scene.document.commit_edit(edit)?;
        }
        Ok(())
    }

    /// Emphasize one mask's overlay while its row is hovered, or none. Emphasis is
    /// presentation only and never part of history.
    ///
    /// # Errors
    /// Rejects unknown surfaces or masks and documents that are not loaded with
    /// [`InputRejection`]; preserves document failures.
    pub(crate) fn hover_input_mask(
        &mut self,
        workflow: &str,
        surface: &str,
        mask: Option<&str>,
    ) -> Result<(), Box<dyn Error>> {
        let record = self.inputs.surface(workflow, surface).ok_or(InputRejection::Unknown)?;
        let layer = match mask {
            Some(key) => Some(record.mask(key).ok_or(InputRejection::Unknown)?.layer),
            None => None,
        };
        let identity = record.identity;
        let (document, _) = self
            .input_document_mut(identity)
            .ok_or(InputRejection::Unavailable("The input document is not loaded"))?;
        let scene = document.scene.as_mut().ok_or("Input document vanished")?;
        scene.document.set_highlight(layer)?;
        Ok(())
    }

    /// Change a blank-based surface's canvas resolution: resize the canvas around `mode`'s
    /// anchor or scale it with every mask. The change is one undoable step, computed by an
    /// owned job; the returned ticket's reply reports the updated document, or a failure when
    /// the document changed while the masks were being resized.
    ///
    /// # Errors
    /// Rejects unknown surfaces, file-based surfaces and documents that are not loaded with
    /// [`InputRejection`], changing nothing.
    pub(crate) fn resize_input(
        &mut self,
        workflow: &str,
        surface: &str,
        extent: Extent,
        mode: ResizeMode,
    ) -> Result<InputTicket, Box<dyn Error>> {
        if self.closing {
            return Err(Box::new(InputRejection::Unavailable("The canvas is closing")));
        }
        let record = self.inputs.surface(workflow, surface).ok_or(InputRejection::Unknown)?;
        if !record.loaded {
            return Err(Box::new(InputRejection::Unavailable("The input document is not loaded")));
        }
        if !matches!(record.loaded_base, Some(Base::Blank(_))) {
            return Err(Box::new(InputRejection::Invalid(
                "Only a blank-based surface can be resized",
            )));
        }
        let (identity, key) = (record.identity, record.key.clone());
        let ticket = self.input_ticket(Arc::from(workflow), vec![key])?;
        self.inputs.request(Task {
            surface: identity,
            target: Target::Geometry,
            work: Work::Resize { extent, mode, ticket: ticket.0 },
        });
        Ok(ticket)
    }
}
