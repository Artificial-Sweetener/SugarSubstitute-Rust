// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive the editing tools of the shown input document for SugarCubes' editing UI.
//!
//! Every input document's tool manager registers the same tools. The session keeps one set of
//! editing preferences (the chosen tool and the Brush and Eraser options) and applies it to each
//! input document as it is shown, so switching images keeps the person's tool. Brush and Eraser
//! share their size: a wheel change to one resizes the other.
//!
//! Coverage work a tool defers runs as an owned job, in request order: work requested while
//! earlier work was running reads that work's result, and work whose document an undo, redo or
//! stroke changed while it ran runs again, so it lands after that change. Selection outlines are
//! traced by owned jobs too.
//! A staged gesture renders live: the frame pipeline latches the latest staged step while no
//! frame work is in flight, and each latched step is distinct content for frame reuse. The
//! selection's marching ants, a shape's outline and the brush's Shift line are presentation
//! overlays drawn over every frame and stepped every 100 ms while ants are visible.
mod commands;
mod overlay;
mod registry;
mod state;
mod work;

use super::*;
pub(crate) use commands::EditRejection;
pub(crate) use state::EditState;
use sugarsubstitute_canvas::{
    BrushOptions, CoverageModification, CursorIntent, ModifyTarget, SelectionOutlines,
    ToolCancelReason, ToolId, ToolInput,
};
use sugarsubstitute_contracts::canvas_native::CanvasToolId;

/// Editing preferences, owned coverage work and the presentation overlay of the shown document.
pub(in crate::native_canvas::session) struct Editing {
    /// The tool the person chose, applied to each input document as it is shown.
    tool: CanvasToolId,
    brush: BrushOptions,
    eraser: BrushOptions,
    /// Whether the "Edit layer coverage" panel is open, which blocks canvas input.
    coverage_mode: bool,
    /// The input document the preferences were last applied to.
    applied: Option<inputs::InputDocumentId>,
    /// The open modification's latest step, which Enter applies.
    modification: Option<(ModifyTarget, CoverageModification)>,
    /// Bounds of the shown selection by its fingerprint, so state reads never rescan tiles.
    selection: Option<([u8; 32], Option<sugarsubstitute_image_ops::Region>)>,
    work: work::EditingWork,
    outlines: SelectionOutlines,
    overlay: overlay::ShownOverlay,
}

impl Default for Editing {
    fn default() -> Self {
        Self {
            tool: CanvasToolId::PanZoom,
            brush: BrushOptions::DEFAULT,
            eraser: BrushOptions::DEFAULT,
            coverage_mode: false,
            applied: None,
            modification: None,
            selection: None,
            work: work::EditingWork::default(),
            outlines: SelectionOutlines::default(),
            overlay: overlay::ShownOverlay::default(),
        }
    }
}

impl Editing {
    /// Whether an editing job still awaits its terminal outcome.
    pub(in crate::native_canvas::session) fn in_flight(&self) -> bool {
        self.work.in_flight()
    }
}

impl Session {
    /// Route one tool observation to the shown document, unless the coverage panel blocks
    /// canvas input, then hand deferred work to its job and follow wheel resizing.
    /// # Errors
    /// Preserves tool, document and job admission failures.
    pub(in crate::native_canvas::session) fn tool_input(
        &mut self,
        input: ToolInput,
        now: Duration,
    ) -> Result<(), Box<dyn Error>> {
        if self.editing.coverage_mode && self.input_shown() {
            return Ok(());
        }
        let document = &mut *self.document;
        let routed = document.tools.input(
            input,
            document.scene.as_mut().map(|scene| &mut scene.document),
            now,
        );
        if let Err(cause) = routed {
            if !memory_refused(&cause) {
                return Err(cause.into());
            }
            // The gesture cannot grow within the memory budget: it ends as Esc would end it,
            // leaving the document as it was, and the canvas keeps running.
            let scene = document.scene.as_ref().map(|scene| &scene.document);
            tracing::warn!(
                operation = "canvas_tool_input",
                document_id = scene.map(|document| document.snapshot().identity()),
                cause = %cause,
                "The canvas memory budget refused a gesture; it was cancelled"
            );
            document.tools.cancel(ToolCancelReason::Requested, scene, now)?;
        }
        self.settle_tools()
    }

    /// The effective editing tool's cursor over the shown input document, or `None` while
    /// navigation is effective and the host shows its own hands. The coverage panel blocks the
    /// canvas, which then shows the arrow.
    pub(crate) fn editing_cursor(&self) -> Option<CursorIntent> {
        if !self.input_shown() {
            return None;
        }
        if self.editing.coverage_mode {
            return Some(CursorIntent::Arrow);
        }
        (self.document.tools.effective() != ToolId::PAN_ZOOM).then(|| {
            self.document.tools.cursor(self.document.scene.as_ref().map(|scene| &scene.document))
        })
    }

    /// Prepare the shown document's editing for the next frame: apply preferences, fall back
    /// from unavailable tools, start queued work and traces, latch the staged gesture and rebuild
    /// the overlay.
    /// # Errors
    /// Preserves tool activation and job admission failures.
    pub(in crate::native_canvas::session) fn prepare_editing(
        &mut self,
    ) -> Result<(), Box<dyn Error>> {
        let now = self.navigation_time();
        self.apply_editing(now)?;
        self.submit_editing_work()?;
        self.latch_staged();
        self.refresh_editing(now)
    }

    /// The device point that shows document point `point` in the shown document's viewport, so
    /// host fixtures can deliver tool input at document positions.
    #[cfg(test)]
    pub(crate) fn device_at(&self, point: [f64; 2]) -> Result<[f64; 2], Box<dyn Error>> {
        Ok(self.document.tools.navigation().viewport().device_at(point)?)
    }

    /// Hand the shown tools' deferred work to its job and share a wheel-changed brush size.
    fn settle_tools(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(work) = self.document.tools.take_deferred()
            && !self.editing.work.queue(work)
        {
            tracing::warn!(
                operation = "canvas_coverage_work",
                "Too much coverage work is waiting; the newest request was dropped"
            );
        }
        self.submit_editing_work()?;
        self.follow_brush_size()?;
        self.refresh_editing(self.navigation_time())
    }
}
