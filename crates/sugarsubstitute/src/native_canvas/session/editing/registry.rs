// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The tools every input document offers, their registry identities and when each is usable.
use super::*;
use sugarsubstitute_canvas::{
    BrushMode, BrushTool, CanvasError, CoverageShapeTool, DocumentSnapshot, LayerContent, LayerId,
    ShapeKind, ShapeTarget, ToolManager, ToolOptions,
};
use sugarsubstitute_contracts::execution::MemoryLease;

/// Registry identities of the registered tools; Pan & Zoom is the manager's built-in tool.
const REGISTERED: [(CanvasToolId, u32); 8] = [
    (CanvasToolId::Brush, 2),
    (CanvasToolId::Eraser, 3),
    (CanvasToolId::SelectRectangle, 4),
    (CanvasToolId::SelectEllipse, 5),
    (CanvasToolId::SelectLasso, 6),
    (CanvasToolId::MaskRectangle, 7),
    (CanvasToolId::MaskEllipse, 8),
    (CanvasToolId::MaskLasso, 9),
];

/// What a tool needs before it can be selected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Requirement {
    /// Always usable.
    Nothing,
    /// An editable input document.
    Document,
    /// An editable target mask in that document.
    Mask,
    /// Offered, but not implemented by this host.
    Unimplemented,
}

const fn requirement(tool: CanvasToolId) -> Requirement {
    match tool {
        CanvasToolId::PanZoom => Requirement::Nothing,
        CanvasToolId::SelectRectangle | CanvasToolId::SelectEllipse | CanvasToolId::SelectLasso => {
            Requirement::Document
        }
        CanvasToolId::Brush
        | CanvasToolId::Eraser
        | CanvasToolId::MaskRectangle
        | CanvasToolId::MaskEllipse
        | CanvasToolId::MaskLasso => Requirement::Mask,
        CanvasToolId::Move | CanvasToolId::Transform | CanvasToolId::SharedEdges => {
            Requirement::Unimplemented
        }
    }
}

/// The registry identity of a tool, or `None` for a tool this host does not implement.
pub(super) fn tool_id(tool: CanvasToolId) -> Option<ToolId> {
    if tool == CanvasToolId::PanZoom {
        return Some(ToolId::PAN_ZOOM);
    }
    let (_, id) = REGISTERED.iter().find(|(candidate, _)| *candidate == tool)?;
    ToolId::new(*id).ok()
}

/// The tool a registry identity names.
pub(super) fn canvas_tool(id: ToolId) -> CanvasToolId {
    REGISTERED
        .iter()
        .find(|(_, candidate)| *candidate == id.get())
        .map_or(CanvasToolId::PanZoom, |(tool, _)| *tool)
}

/// Register Brush, Eraser and the selection and mask shapes on an input document's tools.
/// # Errors
/// Preserves registration admission failures.
fn register_tools(tools: &mut ToolManager, memory: &MemoryLease) -> Result<(), CanvasError> {
    for (tool, id) in REGISTERED {
        let id = ToolId::new(id)?;
        let shape = |target, kind| CoverageShapeTool::new(target, kind);
        match tool {
            CanvasToolId::Brush => {
                tools.register(
                    id,
                    BrushTool::new(BrushMode::Paint, BrushOptions::DEFAULT),
                    memory,
                )?;
            }
            CanvasToolId::Eraser => {
                tools.register(
                    id,
                    BrushTool::new(BrushMode::Erase, BrushOptions::DEFAULT),
                    memory,
                )?;
            }
            CanvasToolId::SelectRectangle => {
                tools.register(id, shape(ShapeTarget::Selection, ShapeKind::Rectangle), memory)?;
            }
            CanvasToolId::SelectEllipse => {
                tools.register(id, shape(ShapeTarget::Selection, ShapeKind::Ellipse), memory)?;
            }
            CanvasToolId::SelectLasso => {
                tools.register(id, shape(ShapeTarget::Selection, ShapeKind::Lasso), memory)?;
            }
            CanvasToolId::MaskRectangle => {
                tools.register(id, shape(ShapeTarget::Mask, ShapeKind::Rectangle), memory)?;
            }
            CanvasToolId::MaskEllipse => {
                tools.register(id, shape(ShapeTarget::Mask, ShapeKind::Ellipse), memory)?;
            }
            CanvasToolId::MaskLasso => {
                tools.register(id, shape(ShapeTarget::Mask, ShapeKind::Lasso), memory)?;
            }
            CanvasToolId::PanZoom
            | CanvasToolId::Move
            | CanvasToolId::Transform
            | CanvasToolId::SharedEdges => {}
        }
    }
    Ok(())
}

/// Whether `target` names an unlocked mask layer of `document`.
pub(super) fn editable_mask(document: &DocumentSnapshot, target: Option<LayerId>) -> bool {
    document.layers().iter().any(|layer| {
        Some(layer.id()) == target
            && !layer.locked()
            && matches!(layer.content(), LayerContent::Coverage { .. })
    })
}

impl Session {
    /// The shown input document's published content, when one is loaded.
    pub(super) fn editable_document(&self) -> Option<DocumentSnapshot> {
        self.input_shown()
            .then(|| self.document.scene.as_ref().map(|scene| scene.document.snapshot()))
            .flatten()
    }

    /// Whether `tool` can be selected on the shown document now.
    pub(super) fn tool_available(&self, tool: CanvasToolId) -> bool {
        let document = self.editable_document();
        match requirement(tool) {
            Requirement::Nothing => true,
            Requirement::Document => document.is_some(),
            Requirement::Mask => document
                .is_some_and(|document| editable_mask(&document, self.document.tools.target())),
            Requirement::Unimplemented => false,
        }
    }

    /// Apply the preferences to a newly shown input document and fall back to Pan & Zoom when
    /// the chosen tool became unavailable, as when the active mask goes away.
    pub(super) fn apply_editing(&mut self, now: Duration) -> Result<(), Box<dyn Error>> {
        let shown = self.document.input;
        if shown.is_none() {
            self.editing.applied = None;
            return Ok(());
        }
        self.register_editing_tools()?;
        if self.editing.applied != shown {
            self.configure_brush(CanvasToolId::Brush, self.editing.brush)?;
            self.configure_brush(CanvasToolId::Eraser, self.editing.eraser)?;
            self.editing.applied = shown;
        }
        let tool = if self.tool_available(self.editing.tool) {
            self.editing.tool
        } else {
            CanvasToolId::PanZoom
        };
        self.editing.tool = tool;
        let id = tool_id(tool).ok_or("An offered tool has no registry identity")?;
        if self.document.tools.selected() != id {
            let document = &mut *self.document;
            document.tools.select(id, document.scene.as_ref().map(|scene| &scene.document), now)?;
        }
        Ok(())
    }

    /// Register the editing tools on the shown input document the first time it is edited,
    /// whichever way it was created or restored.
    pub(super) fn register_editing_tools(&mut self) -> Result<(), Box<dyn Error>> {
        let brush = tool_id(CanvasToolId::Brush).ok_or("The brush has no registry identity")?;
        if !self.input_shown() || self.document.tools.contains(brush) {
            return Ok(());
        }
        let memory = self.resources.as_ref().ok_or("Canvas storage is unavailable")?.memory.clone();
        register_tools(&mut self.document.tools, &memory)?;
        Ok(())
    }

    /// Replace a Brush or Eraser tool's options on the shown document.
    pub(super) fn configure_brush(
        &mut self,
        tool: CanvasToolId,
        options: BrushOptions,
    ) -> Result<(), Box<dyn Error>> {
        let id = tool_id(tool).ok_or("The brush has no registry identity")?;
        self.document.tools.configure(id, ToolOptions::Brush(options))?;
        Ok(())
    }

    /// The options a Brush or Eraser tool of the shown document uses now.
    fn brush_options(&self, tool: CanvasToolId) -> Option<BrushOptions> {
        match self.document.tools.options(tool_id(tool)?)? {
            ToolOptions::Brush(options) => Some(options),
        }
    }

    /// Adopt options the shown tools changed themselves, such as a wheel resize, and give the
    /// other brush the same size.
    pub(super) fn follow_brush_size(&mut self) -> Result<(), Box<dyn Error>> {
        if !self.input_shown() {
            return Ok(());
        }
        let (Some(brush), Some(eraser)) =
            (self.brush_options(CanvasToolId::Brush), self.brush_options(CanvasToolId::Eraser))
        else {
            return Ok(());
        };
        if brush != self.editing.brush {
            self.set_brush(CanvasToolId::Brush, brush)
        } else if eraser != self.editing.eraser {
            self.set_brush(CanvasToolId::Eraser, eraser)
        } else {
            Ok(())
        }
    }

    /// Adopt `options` for one brush and its size for the other, on the shown document too.
    pub(super) fn set_brush(
        &mut self,
        tool: CanvasToolId,
        options: BrushOptions,
    ) -> Result<(), Box<dyn Error>> {
        let (other, current) = match tool {
            CanvasToolId::Eraser => (CanvasToolId::Brush, self.editing.brush),
            _ => (CanvasToolId::Eraser, self.editing.eraser),
        };
        let resized = BrushOptions::new(options.size(), current.hardness(), current.opacity())?;
        if tool == CanvasToolId::Eraser {
            (self.editing.eraser, self.editing.brush) = (options, resized);
        } else {
            (self.editing.brush, self.editing.eraser) = (options, resized);
        }
        if self.input_shown() {
            self.configure_brush(tool, options)?;
            self.configure_brush(other, resized)?;
        }
        Ok(())
    }
}
