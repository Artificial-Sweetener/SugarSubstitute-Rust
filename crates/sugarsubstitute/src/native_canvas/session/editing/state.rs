// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The editing state the editing UI reads after every command and whenever it changes.
use super::{registry::canvas_tool, *};
use sugarsubstitute_canvas::LayerId;

/// The complete editing state of the shown document, before protocol identities and page
/// coordinates are assigned.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EditState {
    /// The editable input document shown, if any.
    pub(crate) document: Option<inputs::InputDocumentId>,
    /// The mask layer the editing tools act on, while the shown document has it.
    pub(crate) active_mask: Option<LayerId>,
    /// The chosen tool; it stays chosen while Space or a modification makes navigation
    /// effective.
    pub(crate) tool: CanvasToolId,
    /// Every offered tool, in SugarCubes' order, with whether it can be selected now.
    pub(crate) tools: [(CanvasToolId, bool); 12],
    /// Brush options.
    pub(crate) brush: BrushOptions,
    /// Eraser options.
    pub(crate) eraser: BrushOptions,
    /// The shown selection's bounds as `[left, top, width, height]` in physical pixels of the
    /// canvas viewport, following the current camera.
    pub(crate) selection: Option<[f64; 4]>,
    /// Whether document undo and redo are available.
    pub(crate) history: [bool; 2],
    /// Whether a live-previewed coverage modification is open.
    pub(crate) modifying: bool,
    /// Whether the "Edit layer coverage" panel is open.
    pub(crate) coverage_mode: bool,
}

impl Session {
    /// The current editing state.
    /// # Errors
    /// Rejects selection bounds the camera cannot map.
    pub(crate) fn edit_state(&self) -> Result<EditState, Box<dyn Error>> {
        let published = self.editable_document();
        let document = published.as_ref().and(self.document.input);
        let target = self.document.tools.target();
        let active_mask = target.filter(|layer| {
            published
                .as_ref()
                .is_some_and(|snapshot| snapshot.layers().iter().any(|item| item.id() == *layer))
        });
        let selection = match self.editing.selection.and_then(|(_, bounds)| bounds) {
            Some(region) if document.is_some() => {
                let viewport = self.document.tools.navigation().viewport();
                let [left, top] =
                    viewport.device_at([f64::from(region.x()), f64::from(region.y())])?;
                let [right, bottom] =
                    viewport.device_at([f64::from(region.right()), f64::from(region.bottom())])?;
                Some([left, top, right - left, bottom - top])
            }
            _ => None,
        };
        let history = match (&published, &self.document.scene) {
            (Some(_), Some(scene)) => scene.document.history(),
            _ => [false, false],
        };
        Ok(EditState {
            document,
            active_mask,
            tool: if document.is_some() {
                canvas_tool(self.document.tools.selected())
            } else {
                CanvasToolId::PanZoom
            },
            tools: CanvasToolId::ALL.map(|tool| (tool, self.tool_available(tool))),
            brush: self.editing.brush,
            eraser: self.editing.eraser,
            selection,
            history,
            modifying: document.is_some() && self.document.tools.modifying().is_some(),
            coverage_mode: self.editing.coverage_mode,
        })
    }

    /// Track the shown selection's bounds and overlay after anything that may change them.
    pub(super) fn refresh_editing(&mut self, now: Duration) -> Result<(), Box<dyn Error>> {
        let selection = self
            .input_shown()
            .then(|| self.document.shown())
            .flatten()
            .and_then(|(_, snapshot)| snapshot.selection().cloned());
        self.editing.selection = match (selection.as_ref(), self.editing.selection) {
            (None, _) => None,
            (Some(coverage), Some((fingerprint, bounds)))
                if coverage.fingerprint() == fingerprint =>
            {
                Some((fingerprint, bounds))
            }
            (Some(coverage), _) => Some((coverage.fingerprint(), coverage.bounds())),
        };
        self.request_outline(selection.as_ref())?;
        self.refresh_overlay(now);
        Ok(())
    }
}
