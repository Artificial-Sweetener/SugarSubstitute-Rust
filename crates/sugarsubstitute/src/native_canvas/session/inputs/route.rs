// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Show an input document on the Input route and return to the output on the Output route.
//!
//! Showing a document swaps it in as the session's current document, the way reselecting a
//! retained output does: once no job or native frame references the outgoing document, it
//! moves out with its camera and backings and the incoming one takes over the display custody.
//! An outgoing input document returns to its surface; an outgoing output document waits in a
//! dedicated slot, never in evictable retention, until the Output route restores it.
use super::*;
use sugarsubstitute_canvas::Document;

/// The route change waiting for the current document's work to settle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Pending {
    Input { surface: InputDocumentId, target: Option<LayerId> },
    Output,
}

impl Session {
    /// Show `surface`'s document on the Input route with `mask` as the editing tools' target.
    /// The swap completes on a later poll once the outgoing document's work has settled and
    /// the surface's document has loaded; a newer route request replaces it.
    ///
    /// # Errors
    /// Rejects an unknown workflow, surface or mask, or a closing canvas, with
    /// [`InputRejection`], changing nothing.
    pub(crate) fn activate_input(
        &mut self,
        workflow: &str,
        surface: &str,
        mask: Option<&str>,
    ) -> Result<(), Box<dyn Error>> {
        if self.closing {
            return Err(Box::new(InputRejection::Unavailable("The canvas is closing")));
        }
        let record = self.inputs.surface(workflow, surface).ok_or(InputRejection::Unknown)?;
        let target = match mask {
            Some(key) => Some(record.mask(key).ok_or(InputRejection::Unknown)?.layer),
            None => None,
        };
        let identity = record.identity;
        if self.document.input == Some(identity) {
            self.document.tools.set_target(target);
            self.inputs.route = None;
        } else {
            self.inputs.route = Some(Pending::Input { surface: identity, target });
        }
        Ok(())
    }

    /// Return to the Output route. Returns whether it shows the output that was showing before
    /// the Input route without a new offer; `false` asks the host to offer its output again.
    pub(crate) fn show_output(&mut self) -> bool {
        if !self.input_shown() {
            self.inputs.route = None;
            return true;
        }
        self.inputs.route = Some(Pending::Output);
        self.inputs.parked_output.is_some()
    }

    /// Complete a pending route change once the current document can move. Returns `true`
    /// while ordinary document admission must wait for the outgoing document's work.
    ///
    /// # Errors
    /// Preserves frame withdrawal, camera geometry and tool cancellation failures.
    pub(in crate::native_canvas::session) fn advance_input_route(
        &mut self,
    ) -> Result<bool, Box<dyn Error>> {
        let Some(pending) = self.inputs.route else {
            return Ok(false);
        };
        if self.document_suspended
            || self.document_transaction.is_some()
            || self.preview_pending()
            || self.presentation.gpu_window().is_some()
            || self.resources.is_none()
        {
            return Ok(false);
        }
        match pending {
            Pending::Input { surface, target } => {
                if self.document.input == Some(surface) {
                    self.document.tools.set_target(target);
                    self.inputs.route = None;
                    return Ok(false);
                }
                match self.inputs.by_identity(surface) {
                    None => {
                        self.inputs.route = None;
                        return Ok(false);
                    }
                    Some(record) if record.document.is_none() => return Ok(false),
                    Some(_) => {}
                }
            }
            Pending::Output if !self.input_shown() => {
                self.inputs.route = None;
                return Ok(false);
            }
            Pending::Output => {}
        }
        if !self.document_parkable() {
            self.fence_document_work()?;
            return Ok(true);
        }
        let Some(intent) = self.supersede_pending_source()? else {
            return Ok(true);
        };
        let incoming = match pending {
            Pending::Input { surface, .. } => self
                .inputs
                .by_identity_mut(surface)
                .and_then(|record| record.document.take())
                .ok_or("Input document vanished")?,
            Pending::Output => match self.inputs.parked_output.take() {
                Some(document) => document,
                None => self.empty_output(intent)?,
            },
        };
        self.show_document(incoming, intent)?;
        if let Pending::Input { target, .. } = pending {
            self.document.tools.set_target(target);
        }
        self.inputs.route = None;
        Ok(false)
    }

    /// Make `incoming` the current document as selection `intent`, keeping the screen's
    /// display custody and stowing the outgoing document.
    fn show_document(
        &mut self,
        incoming: Box<document::DocumentState>,
        intent: u64,
    ) -> Result<(), Box<dyn Error>> {
        let now = self.navigation_time();
        let extent = self.document.tools.navigation().viewport().surface();
        let mut outgoing = std::mem::replace(&mut self.document, incoming);
        self.document.adopt_display(&mut outgoing);
        let settled = reconcile::settle_tools(&mut outgoing, now);
        self.stow_document(outgoing);
        settled?;
        let document = &mut *self.document;
        document.rebind(intent);
        document.tools.set_refresh_rate(self.display_refresh)?;
        document.tools.resize(extent, document.scene.as_ref().map(|scene| &scene.document), now)?;
        document.tools.advance(now)?;
        if self.pacing.period() != self.document.tools.navigation().frame_period() {
            self.pacing.set_frame_period(self.document.tools.navigation().frame_period(), now)?;
        }
        self.expose_pending = true;
        self.title = self.labels.viewing(self.document.tools.navigation().viewport().zoom());
        Ok(())
    }

    /// Keep a document leaving the screen: input documents return to their surfaces and an
    /// output waits in the dedicated slot, displacing an earlier one into retention.
    pub(super) fn stow_document(&mut self, document: Box<document::DocumentState>) {
        if document.input.is_some() {
            self.inputs.stow(document);
        } else if let Some(previous) = self.inputs.parked_output.replace(document) {
            self.park(previous);
        }
    }

    /// Keep a newly loaded output behind the shown input document until the Output route.
    /// The input document stays current and is relabelled as the output's selection.
    pub(in crate::native_canvas::session) fn park_loaded_output(
        &mut self,
        scene: Scene,
        retention: Option<RetentionKey>,
        intent: u64,
    ) -> Result<(), Box<dyn Error>> {
        let now = self.navigation_time();
        let viewport = Viewport::new(self.document.tools.navigation().viewport().surface())?;
        let mut output = Box::new(document::DocumentState::new(viewport));
        output.tools.set_refresh_rate(self.display_refresh)?;
        output.tools.set_content(
            Some(scene.document.snapshot().extent()),
            Some(&scene.document),
            now,
        )?;
        if let Some((_, frame)) =
            self.initial_frame.take().filter(|(offered, _)| *offered == intent)
        {
            let outcome = output.tools.navigate(frame, now)?;
            if !matches!(outcome, sugarsubstitute_canvas::NavigationCommandOutcome::Applied { .. })
            {
                tracing::debug!(operation="canvas_selection_frame", outcome=?outcome, "Selection camera was not admitted; the image starts at Fit");
            }
        }
        output.scene = Some(scene);
        output.retention = retention;
        output.initial_preview = true;
        self.stow_document(output);
        self.document.rebind(intent);
        self.expose_pending = true;
        Ok(())
    }

    /// Keep a reselected retained output behind the shown input document.
    pub(in crate::native_canvas::session) fn park_retained_output(
        &mut self,
        key: &RetentionKey,
        intent: u64,
    ) {
        if self
            .inputs
            .parked_output
            .as_ref()
            .is_none_or(|document| document.retention.as_ref() != Some(key))
            && let Some(document) = self.retained.take(key)
        {
            self.stow_document(document);
        }
        self.document.rebind(intent);
        self.expose_pending = true;
    }

    /// An empty output document, shown when the Output route has nothing parked.
    fn empty_output(&self, intent: u64) -> Result<Box<document::DocumentState>, Box<dyn Error>> {
        let resources = self.resources.as_ref().ok_or("Canvas storage is unavailable")?;
        let viewport = Viewport::new(self.document.tools.navigation().viewport().surface())?;
        let mut document = Box::new(document::DocumentState::new(viewport));
        document.scene = Some(Scene {
            document: Document::new(intent, Extent::new(1, 1)?, &resources.memory)?,
            source: None,
            source_revision: intent,
            original: None,
            compared: None,
            placed: Vec::new(),
        });
        Ok(document)
    }

    /// Keep a shown input document drawing across a selection change it does not own.
    pub(in crate::native_canvas::session) fn follow_selection(&mut self) {
        if self.input_shown() {
            let selection = self.selection;
            self.document.rebind(selection);
        }
    }
}
