// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A mask document under one session tool, driven like the host drives it: input, typed
//! commands, and every deferred request run as an owned job and completed in order.
use super::*;
use crate::{MaskAppearance, PointerSample};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_image_ops::{Coverage, FULL_COVERAGE, Region};

/// The session tool under test.
pub(in crate::tools) const TOOL: u32 = 2;
/// The tool selected before it, which settling returns to.
pub(in crate::tools) const OTHER: u32 = 3;
/// The first mask's identity; further masks count up from it.
pub(in crate::tools) const MASK: u64 = 7;

pub(in crate::tools) struct Canvas {
    pub(in crate::tools) tools: ToolManager,
    pub(in crate::tools) document: Document,
}

/// A 200×200 document holding fully covered rectangular masks `[left, top, right, bottom]`,
/// the first one targeted, with `tool` selected after a selection rectangle tool.
pub(in crate::tools) fn canvas(
    context: &JobContext,
    tool: impl CanvasTool + 'static,
    masks: &[[u32; 4]],
) -> Result<Canvas, CanvasError> {
    let extent = Extent::new(200, 200)?;
    let mut tools = ToolManager::new(Navigation::new(crate::Viewport::new(extent)?));
    tools.register(ToolId::new(TOOL)?, tool, context.memory())?;
    let other = CoverageShapeTool::new(ShapeTarget::Selection, ShapeKind::Rectangle);
    tools.register(ToolId::new(OTHER)?, other, context.memory())?;
    let mut document = Document::new(1, extent, context.memory())?;
    for (index, rect) in masks.iter().enumerate() {
        let layer = mask_layer(MASK + index as u64, rectangle(extent, *rect, context)?)?;
        document.apply(DocumentEdit::Insert { index, layer })?;
    }
    tools.set_target(Some(LayerId::new(MASK)?));
    tools.select(ToolId::new(OTHER)?, Some(&document), Duration::ZERO)?;
    tools.select(ToolId::new(TOOL)?, Some(&document), Duration::ZERO)?;
    Ok(Canvas { tools, document })
}

pub(in crate::tools) fn mask_layer(id: u64, coverage: Coverage) -> Result<Layer, CanvasError> {
    let appearance = MaskAppearance::new([255, 0, 0], 0.5)?;
    Layer::new(LayerId::new(id)?, LayerContent::Coverage { coverage, appearance })
}

/// Full coverage inside `[left, top, right, bottom]`.
pub(in crate::tools) fn rectangle(
    extent: Extent,
    [left, top, right, bottom]: [u32; 4],
    context: &JobContext,
) -> Result<Coverage, CanvasError> {
    let mut coverage = Coverage::new(extent, 0, context.memory())?;
    let region = Region::new(left, top, right - left, bottom - top)?;
    coverage.update(region, context.memory(), |_, _, _| FULL_COVERAGE)?;
    Ok(coverage)
}

pub(in crate::tools) fn held(shift: bool, ctrl: bool, alt: bool) -> ToolModifiers {
    ToolModifiers { shift, ctrl, alt }
}

pub(in crate::tools) fn run(
    test: impl FnOnce(&JobContext) -> Result<(), CanvasError> + Send + 'static,
) -> harness::TestResult {
    harness::run(64 * 1024 * 1024, |context| {
        test(context).map_err(|cause| ImageError::Source(Box::new(cause)))
    })
}

impl Canvas {
    pub(in crate::tools) fn point(
        &mut self,
        position: [f64; 2],
        modifiers: ToolModifiers,
        context: &JobContext,
    ) -> Result<(), CanvasError> {
        let input = ToolInput::Pointer(PointerSample::mouse(position, modifiers));
        self.tools.input(input, Some(&mut self.document), Duration::ZERO)?;
        self.settle(context)
    }
    pub(in crate::tools) fn button(
        &mut self,
        pressed: bool,
        modifiers: ToolModifiers,
        context: &JobContext,
    ) -> Result<(), CanvasError> {
        let input = ToolInput::Button { button: ToolButton::Primary, pressed, modifiers };
        self.tools.input(input, Some(&mut self.document), Duration::ZERO)?;
        self.settle(context)
    }
    /// Press at the first point, move through the rest and release at the last.
    pub(in crate::tools) fn drag(
        &mut self,
        path: &[[f64; 2]],
        modifiers: ToolModifiers,
        context: &JobContext,
    ) -> Result<(), CanvasError> {
        self.point(path[0], modifiers, context)?;
        self.button(true, modifiers, context)?;
        for point in &path[1..] {
            self.point(*point, modifiers, context)?;
        }
        self.button(false, modifiers, context)
    }
    pub(in crate::tools) fn command(
        &mut self,
        command: ToolCommand,
        context: &JobContext,
    ) -> Result<bool, CanvasError> {
        let acted = self.tools.command(command, &mut self.document, Duration::ZERO)?;
        self.settle(context)?;
        Ok(acted)
    }
    /// Run and complete the pending deferred request, as the host does after every call.
    pub(in crate::tools) fn settle(&mut self, context: &JobContext) -> Result<(), CanvasError> {
        if let Some(work) = self.tools.take_deferred() {
            let prepared = work.run(context)?;
            self.tools.complete_deferred(prepared, &mut self.document, Duration::ZERO)?;
        }
        Ok(())
    }
    /// What the canvas shows: the staged preview, or the published document.
    pub(in crate::tools) fn shown(&self) -> DocumentSnapshot {
        self.tools.preview_snapshot().unwrap_or_else(|| self.document.snapshot())
    }
    pub(in crate::tools) fn coverage(snapshot: &DocumentSnapshot, id: u64) -> Option<Coverage> {
        snapshot.layers().iter().find_map(|layer| match layer.content() {
            LayerContent::Coverage { coverage, .. } if layer.id().get() == id => {
                Some(coverage.clone())
            }
            _ => None,
        })
    }
    /// Bounds of a mask as the canvas shows it.
    pub(in crate::tools) fn shown_bounds(&self, id: u64) -> Option<Region> {
        Self::coverage(&self.shown(), id).and_then(|coverage| coverage.bounds())
    }
    /// Bounds of a mask as the document publishes it.
    pub(in crate::tools) fn bounds(&self, id: u64) -> Option<Region> {
        Self::coverage(&self.document.snapshot(), id).and_then(|coverage| coverage.bounds())
    }
    pub(in crate::tools) fn select(
        &mut self,
        rect: [u32; 4],
        context: &JobContext,
    ) -> Result<(), CanvasError> {
        let selection = rectangle(self.document.snapshot().extent(), rect, context)?;
        self.document.apply(DocumentEdit::Select(Some(selection))).map(drop)
    }
}

pub(in crate::tools) fn region(
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<Option<Region>, CanvasError> {
    Ok(Some(Region::new(x, y, width, height)?))
}
