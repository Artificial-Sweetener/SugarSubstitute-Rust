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
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Pending {
    Input { surface: InputDocumentId, target: Option<LayerId> },
    Output,
    Original(RetentionKey),
}

impl Session {
    /// Observe a queued input/output route without granting or changing document custody.
    pub(in crate::native_canvas::session) fn input_route_pending(&self) -> bool {
        self.inputs.route.is_some()
    }

    /// Show `surface`'s document on the Input route with `mask` as the editing tools' target.
    /// The swap completes on a later poll once the outgoing document's work has settled and
    /// the surface's document has loaded; a newer route request replaces it. The shown document
    /// is presented without any output selection, so activation requests the embedded drawing
    /// device an output selection would otherwise have requested.
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
        self.request_host_graphics()
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

    /// Whether actual current/parked pixels came from this host-qualified original identity.
    pub(crate) fn output_original_matches(&self, key: &RetentionKey) -> bool {
        let document = if self.input_shown() {
            self.inputs.parked_output.as_deref()
        } else {
            Some(self.document.as_ref())
        };
        document.is_some_and(|document| {
            document.retention.as_ref() == Some(key) && document.scene.is_some()
        })
    }

    /// Promote the same actual immutable originals to the host's verified retention identity.
    /// Keeps the authored document, undo history, tools, camera and native backings intact.
    /// # Errors
    /// Refuses a present original whose decoder-computed full SHA differs from the receipt;
    /// neither the document nor its retention identity changes on refusal. No I/O is performed.
    pub(crate) fn retain_output_original(
        &mut self,
        key: &RetentionKey,
        expected: &[[u8; 32]],
    ) -> Result<bool, Box<dyn Error>> {
        let document = if self.input_shown() {
            self.inputs.parked_output.as_deref_mut()
        } else {
            Some(self.document.as_mut())
        };
        let Some(document) = document else {
            return Ok(false);
        };
        let Some(scene) = &document.scene else {
            return Ok(false);
        };
        let Some(source) = &scene.source else {
            return Ok(false);
        };
        let primary = source.descriptor().original_hash();
        let compared =
            scene.compared.as_ref().map(|compared| compared.source.descriptor().original_hash());
        if !scene.placed.is_empty()
            || expected.first() != Some(&primary)
            || match compared {
                Some(compared) => expected.len() != 2 || expected.get(1) != Some(&compared),
                None => expected.len() != 1,
            }
        {
            return Err(Box::new(InputRejection::Unavailable(
                "The parked output original changed",
            )));
        }
        document.retention = Some(key.clone());
        Ok(true)
    }

    /// Return to an exact original only after its independently admitted load has parked it.
    /// An older parked output stays hidden while the requested original is acquired.
    pub(crate) fn show_output_original(&mut self, key: RetentionKey) {
        self.inputs.route = self.input_shown().then_some(Pending::Original(key));
    }

    /// Complete a pending route change once the current document can move. Returns `true`
    /// while ordinary document admission must wait for the outgoing document's work.
    ///
    /// # Errors
    /// Preserves frame withdrawal, camera geometry and tool cancellation failures.
    pub(in crate::native_canvas::session) fn advance_input_route(
        &mut self,
    ) -> Result<bool, Box<dyn Error>> {
        let Some(pending) = self.inputs.route.clone() else {
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
        match &pending {
            Pending::Input { surface, target } => {
                if self.document.input == Some(*surface) {
                    self.document.tools.set_target(*target);
                    self.inputs.route = None;
                    return Ok(false);
                }
                match self.inputs.by_identity(*surface) {
                    None => {
                        self.inputs.route = None;
                        return Ok(false);
                    }
                    Some(record) if record.document.is_none() => return Ok(false),
                    Some(_) => {}
                }
            }
            Pending::Output | Pending::Original(_) if !self.input_shown() => {
                self.inputs.route = None;
                return Ok(false);
            }
            Pending::Original(key) if !self.output_original_matches(key) => return Ok(false),
            Pending::Output | Pending::Original(_) => {}
        }
        if !self.document_parkable() {
            self.fence_document_work()?;
            return Ok(true);
        }
        let Some(intent) = self.supersede_pending_source()? else {
            return Ok(true);
        };
        let incoming = match &pending {
            Pending::Input { surface, .. } => self
                .inputs
                .by_identity_mut(*surface)
                .and_then(|record| record.document.take())
                .ok_or("Input document vanished")?,
            Pending::Output | Pending::Original(_) => match self.inputs.parked_output.take() {
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
        let frame = self.selection_frame(intent);
        selection_camera::start_at_selection_camera(
            &mut output.tools,
            Some(&scene.document),
            frame,
            now,
        )?;
        self.selection_framed(intent);
        output.scene = Some(scene);
        output.retention = retention;
        output.initial_preview = true;
        self.stow_document(output);
        self.document.rebind(intent);
        self.expose_pending = true;
        Ok(())
    }

    /// Keep a reselected retained output behind the shown input document, starting it at the
    /// selection's camera on the current physical extent as a newly loaded output starts.
    ///
    /// # Errors
    /// Preserves navigation time and geometry failures.
    pub(in crate::native_canvas::session) fn park_retained_output(
        &mut self,
        key: &RetentionKey,
        intent: u64,
    ) -> Result<(), Box<dyn Error>> {
        if self
            .inputs
            .parked_output
            .as_ref()
            .is_none_or(|document| document.retention.as_ref() != Some(key))
            && let Some(document) = self.retained.take(key)
        {
            self.stow_document(document);
        }
        let frame = self.selection_frame(intent);
        let now = self.navigation_time();
        let extent = self.document.tools.navigation().viewport().surface();
        if let Some(output) = self
            .inputs
            .parked_output
            .as_mut()
            .filter(|document| document.retention.as_ref() == Some(key))
        {
            let scene = output.scene.as_ref().map(|scene| &scene.document);
            output.tools.resize(extent, scene, now)?;
            selection_camera::return_to_selection_camera(&mut output.tools, scene, frame, now)?;
            self.selection_framed(intent);
        }
        self.document.rebind(intent);
        self.expose_pending = true;
        Ok(())
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
