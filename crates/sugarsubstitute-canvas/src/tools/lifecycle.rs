// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Tool lifecycle: geometry, document and policy changes, temporary navigation, cancellation
//! and switching the effective tool.
//!
//! Lifecycle loss cancels capture and gestures. An unresolved coverage modification or edit
//! session survives focus, geometry, policy and temporary-navigation changes with its preview;
//! an explicit cancel, a document change or shutdown ends it.
use super::{CanvasTool, ToolCancelReason, ToolId, ToolManager, ToolView};
use crate::{CanvasError, Document, NavigationInput};
use std::time::Duration;

impl ToolManager {
    /// Park this document at its qualified native-accepted camera and retire active gestures.
    /// The caller supplies evidence for the current document/source. Capture, temporary tools
    /// and uncommitted edits follow the existing document-change cancellation lifecycle.
    ///
    /// # Errors
    /// Rejects camera/time before cancellation and preserves tool reactivation failures.
    pub fn freeze_at(
        &mut self,
        accepted: crate::Viewport,
        document: Option<&Document>,
        now: Duration,
    ) -> Result<(), CanvasError> {
        let mut navigation = self.navigation;
        navigation.freeze_at(accepted, now)?;
        self.cancel(ToolCancelReason::DocumentChanged, document, now)?;
        self.navigation = navigation;
        Ok(())
    }
    /// Change physical geometry and cancel gestures whose pointer basis has changed.
    /// # Errors
    /// Rejects geometry/time before cancellation; preserves selected-tool activation errors.
    pub fn resize(
        &mut self,
        extent: sugarsubstitute_image_ops::Extent,
        document: Option<&Document>,
        now: Duration,
    ) -> Result<(), CanvasError> {
        let mut navigation = self.navigation;
        navigation.resize(extent, now)?;
        if extent == self.navigation.viewport().surface() {
            self.navigation = navigation;
            return Ok(());
        }
        self.cancel(ToolCancelReason::SurfaceChanged, document, now)?;
        self.navigation = navigation;
        Ok(())
    }
    /// Replace content bounds and cancel capture and uncommitted edits from the previous document.
    /// # Errors
    /// Rejects geometry/time before cancellation; preserves selected-tool activation errors.
    pub fn set_content(
        &mut self,
        extent: Option<sugarsubstitute_image_ops::Extent>,
        document: Option<&Document>,
        now: Duration,
    ) -> Result<(), CanvasError> {
        let mut navigation = self.navigation;
        navigation.set_content(extent, now)?;
        self.cancel(ToolCancelReason::DocumentChanged, document, now)?;
        self.navigation = navigation;
        Ok(())
    }
    /// Revoke navigation gestures when locking or reserving drag input for the host.
    /// # Errors
    /// Preserves clock and selected-tool activation errors during cancellation.
    pub fn set_policy(
        &mut self,
        locked: bool,
        drag_out: bool,
        document: Option<&Document>,
        now: Duration,
    ) -> Result<(), CanvasError> {
        self.navigation.advance(now)?;
        if locked || drag_out {
            self.cancel(ToolCancelReason::Locked, document, now)?;
        }
        self.navigation.set_policy(locked, drag_out);
        Ok(())
    }
    /// Enter/leave temporary Space navigation without replacing persistent selection.
    /// Entering first lets the selected tool settle its gesture, as a brush commits its stroke;
    /// whatever edit remains is then discarded.
    /// # Errors
    /// Preserves the tool's settling failure and activation errors; current capture and
    /// uncommitted edits remain bounded.
    pub fn temporary_navigation(
        &mut self,
        held: bool,
        mut document: Option<&mut Document>,
        now: Duration,
    ) -> Result<(), CanvasError> {
        if held == self.space {
            return Ok(());
        }
        if held && self.active().is_some_and(CanvasTool::captures_space) {
            return Ok(());
        }
        if self.modification.is_some() {
            // Coverage modification already holds navigation; the selection returns after it.
            self.space = held;
            return Ok(());
        }
        if held {
            let effective = self.effective;
            self.dispatch_to(effective, document.as_deref_mut(), now, |tool, context| {
                tool.suspend(context)
            })?;
        }
        let id = if held { ToolId::PAN_ZOOM } else { self.selected };
        self.switch(id, document.as_deref(), ToolCancelReason::TemporaryNavigation, now)?;
        self.space = held;
        Ok(())
    }
    /// Cancel capture and staged edits, restoring persistent selection after lifecycle loss.
    /// An unresolved coverage modification or edit session survives focus, geometry and policy
    /// changes with its preview; the person cancelling, a document change or shutdown cancels it
    /// too.
    /// # Errors
    /// Preserves clock and activation failures; staged gesture edits are always discarded.
    pub fn cancel(
        &mut self,
        reason: ToolCancelReason,
        document: Option<&Document>,
        now: Duration,
    ) -> Result<(), CanvasError> {
        if self.modification.is_some() {
            return self.interrupt_modification(reason, document, now);
        }
        self.cancel_active(reason);
        let ends_sessions = matches!(
            reason,
            ToolCancelReason::Requested
                | ToolCancelReason::DocumentChanged
                | ToolCancelReason::Shutdown
        );
        if ends_sessions && self.selected != self.effective {
            self.cancel_tool(self.selected, reason);
        }
        self.capture = false;
        if !self.session_open() {
            self.discard_edit();
            self.deferred = None;
        }
        self.navigation.input(NavigationInput::Cancel, now)?;
        if self.space {
            self.switch(self.selected, document, reason, now)?;
            self.space = false;
        }
        Ok(())
    }
    pub(super) fn cancel_active(&mut self, reason: ToolCancelReason) {
        self.cancel_tool(self.effective, reason);
    }
    pub(super) fn cancel_tool(&mut self, id: ToolId, reason: ToolCancelReason) {
        if id == ToolId::PAN_ZOOM {
            self.pan_zoom.cancel(reason);
        } else if let Some(entry) =
            self.registered.iter_mut().flatten().find(|entry| entry.id == id)
        {
            entry.tool.cancel(reason);
        }
    }
    /// Make `id` the effective tool, cancelling the previous one; an open session keeps its
    /// preview.
    pub(super) fn switch(
        &mut self,
        id: ToolId,
        document: Option<&Document>,
        reason: ToolCancelReason,
        now: Duration,
    ) -> Result<(), CanvasError> {
        if id == self.effective {
            return Ok(());
        }
        self.navigation.advance(now)?;
        let mut navigation = self.navigation;
        navigation.input(NavigationInput::Cancel, now)?;
        let view = ToolView {
            navigation: &navigation,
            document: document.map(Document::snapshot),
            target: self.target,
        };
        if id == ToolId::PAN_ZOOM {
            self.pan_zoom.activate(&view)?;
        } else {
            self.registered
                .iter_mut()
                .flatten()
                .find(|entry| entry.id == id)
                .ok_or(CanvasError::Invalid("Unknown tool identity"))?
                .tool
                .activate(&view)?;
        }
        self.cancel_active(reason);
        self.capture = false;
        self.navigation = navigation;
        if !self.session_open() {
            self.discard_edit();
        }
        self.effective = id;
        Ok(())
    }
}
