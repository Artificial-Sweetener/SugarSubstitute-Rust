// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Serve `geometry.request` from the input document the Input route shows.
//!
//! Geometry reports the shown document and its selection bounds in document pixels. Resizing
//! is not offered: a geometry resize names only an extent, while an input document's size
//! belongs to its workflow's size authority, which changes it through `workflow.request`
//! resizes with an explicit anchor or resampling.
use super::input_protocol::{canvas_document, failure};
use super::*;
use sugarsubstitute_contracts::canvas_native::{
    CanvasDimensions, CanvasFailureStatus, CanvasGeometryCommand, CanvasGeometryContext,
    CanvasGeometryRequest, CanvasSelectionBounds,
};

impl NativeSurface {
    /// Serve `geometry.request`: the shown document and its selection bounds.
    pub(super) fn receive_geometry(
        &mut self,
        identity: MainIdentity,
        request: CanvasGeometryRequest,
        claim: MemoryClaim,
        session: Option<&mut Session>,
    ) -> Result<(), Box<dyn Error>> {
        let payload = match request.command() {
            CanvasGeometryCommand::Context => {
                let shown = session
                    .filter(|session| session.initialized())
                    .and_then(|session| session.shown_input());
                let (document, selection) = match shown {
                    Some(shown) => {
                        let snapshot = &shown.snapshot;
                        let selection =
                            match snapshot.selection().and_then(|coverage| coverage.bounds()) {
                                Some(bounds) => Some(CanvasSelectionBounds::new(
                                    i32::try_from(bounds.x())?,
                                    i32::try_from(bounds.y())?,
                                    CanvasDimensions::new(bounds.width(), bounds.height())?,
                                )?),
                                None => None,
                            };
                        (Some(canvas_document(shown.document, snapshot.extent())?), selection)
                    }
                    None => (None, None),
                };
                request.context_reply(&CanvasGeometryContext {
                    supported: true,
                    resize: false,
                    selection_supported: true,
                    document,
                    selection,
                })?
            }
            CanvasGeometryCommand::Resize { .. } => {
                let failure = failure(
                    CanvasFailureStatus::Unsupported,
                    "Input documents resize through their workflow",
                );
                request.failure_reply(&failure)?
            }
        };
        self.answer(identity, payload, claim)
    }
}
