// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Shape gesture modes, constraints, snapping, outline dragging and mask shapes, applied through
//! the deferred-work boundary the host runs.
use super::*;
use crate::{
    Document, DocumentEdit, Layer, LayerContent, LayerId, MaskAppearance, Navigation,
    PointerSample, ToolId, ToolManager, Viewport, overlay::OverlayShape, tests::harness,
};
use std::time::Duration;
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_image_ops::{Coverage, Extent, FULL_COVERAGE, ImageError, Region};

const TOOL: u32 = 2;
const MASK: u64 = 7;

struct Canvas {
    tools: ToolManager,
    document: Document,
}

fn canvas(
    context: &JobContext,
    target: ShapeTarget,
    kind: ShapeKind,
) -> Result<Canvas, CanvasError> {
    let extent = Extent::new(200, 200)?;
    let mut tools = ToolManager::new(Navigation::new(Viewport::new(extent)?));
    let id = ToolId::new(TOOL)?;
    tools.register(id, CoverageShapeTool::new(target, kind), context.memory())?;
    let document = Document::new(1, extent, context.memory())?;
    tools.select(id, Some(&document), Duration::ZERO)?;
    Ok(Canvas { tools, document })
}

fn modifiers(shift: bool, ctrl: bool, alt: bool) -> ToolModifiers {
    ToolModifiers { shift, ctrl, alt }
}

impl Canvas {
    fn send(&mut self, input: ToolInput) -> Result<(), CanvasError> {
        self.tools.input(input, Some(&mut self.document), Duration::ZERO)
    }
    fn point(&mut self, position: [f64; 2], held: ToolModifiers) -> Result<(), CanvasError> {
        self.send(ToolInput::Pointer(PointerSample::mouse(position, held)))
    }
    fn button(&mut self, pressed: bool, held: ToolModifiers) -> Result<(), CanvasError> {
        self.send(ToolInput::Button { button: ToolButton::Primary, pressed, modifiers: held })
    }
    fn drag(
        &mut self,
        path: &[[f64; 2]],
        held: ToolModifiers,
        context: &JobContext,
    ) -> Result<bool, CanvasError> {
        self.point(path[0], held)?;
        self.button(true, held)?;
        for point in &path[1..] {
            self.point(*point, held)?;
        }
        self.button(false, held)?;
        assert!(!self.tools.captured());
        self.settle(context)
    }
    fn settle(&mut self, context: &JobContext) -> Result<bool, CanvasError> {
        let Some(work) = self.tools.take_deferred() else {
            return Ok(false);
        };
        let prepared = work.run(context)?;
        self.tools.complete_deferred(prepared, &mut self.document, Duration::ZERO)
    }
    fn selection(&self) -> Option<Region> {
        self.document.snapshot().selection().and_then(Coverage::bounds)
    }
    fn mask(&self) -> Option<Coverage> {
        self.document.snapshot().layers().iter().find_map(|layer| match layer.content() {
            LayerContent::Coverage { coverage, .. } => Some(coverage.clone()),
            _ => None,
        })
    }
}

fn run(
    test: impl FnOnce(&JobContext) -> Result<(), CanvasError> + Send + 'static,
) -> harness::TestResult {
    harness::run(32 * 1024 * 1024, |context| {
        test(context).map_err(|cause| ImageError::Source(Box::new(cause)))
    })
}

fn region(x: u32, y: u32, width: u32, height: u32) -> Result<Option<Region>, CanvasError> {
    Ok(Some(Region::new(x, y, width, height)?))
}

const NONE: ToolModifiers = ToolModifiers::NONE;

#[test]
fn drags_replace_add_and_subtract_with_subtraction_winning() -> harness::TestResult {
    run(|context| {
        let mut canvas = canvas(context, ShapeTarget::Selection, ShapeKind::Rectangle)?;
        assert!(canvas.drag(&[[20.0, 30.0], [60.0, 70.0]], NONE, context)?);
        assert_eq!(canvas.selection(), region(20, 30, 40, 40)?);
        // Replace is the default: a drag outside the selection starts over.
        assert!(canvas.drag(&[[120.0, 130.0], [150.0, 160.0]], NONE, context)?);
        assert_eq!(canvas.selection(), region(120, 130, 30, 30)?);
        let shift = modifiers(true, false, false);
        assert!(canvas.drag(&[[20.0, 30.0], [60.0, 70.0]], shift, context)?);
        assert_eq!(canvas.selection(), region(20, 30, 130, 130)?);
        let both = modifiers(true, false, true);
        assert!(canvas.drag(&[[110.0, 110.0], [170.0, 170.0]], both, context)?);
        assert_eq!(canvas.selection(), region(20, 30, 40, 40)?);
        let alt = modifiers(false, false, true);
        assert!(canvas.drag(&[[10.0, 10.0], [65.0, 50.0]], alt, context)?);
        assert_eq!(canvas.selection(), region(20, 50, 40, 20)?);
        // Each drag was one undo step.
        assert!(canvas.document.undo()?);
        assert_eq!(canvas.selection(), region(20, 30, 40, 40)?);
        // Subtracting everything deselects.
        assert!(canvas.drag(&[[15.0, 15.0], [80.0, 80.0]], alt, context)?);
        assert_eq!(canvas.document.snapshot().selection(), None);
        Ok(())
    })
}

#[test]
fn shift_squares_and_alt_squares_only_the_first_selection() -> harness::TestResult {
    run(|context| {
        let mut canvas = canvas(context, ShapeTarget::Selection, ShapeKind::Rectangle)?;
        let alt = modifiers(false, false, true);
        assert!(canvas.drag(&[[60.0, 70.0], [20.0, 50.0]], alt, context)?);
        assert_eq!(canvas.selection(), region(20, 30, 40, 40)?);
        let shift = modifiers(true, false, false);
        assert!(canvas.drag(&[[120.0, 120.0], [150.0, 130.0]], shift, context)?);
        assert_eq!(canvas.selection(), region(20, 30, 130, 120)?);
        assert_eq!(
            canvas.document.snapshot().selection().map(|selection| selection.value(149, 149)),
            Some(FULL_COVERAGE)
        );
        Ok(())
    })
}

#[test]
fn corners_snap_to_document_edges_and_centres_unless_ctrl_is_held() -> harness::TestResult {
    run(|context| {
        let mut canvas = canvas(context, ShapeTarget::Selection, ShapeKind::Rectangle)?;
        assert!(canvas.drag(&[[4.0, 30.0], [96.0, 194.0]], NONE, context)?);
        assert_eq!(canvas.selection(), region(0, 30, 100, 170)?);
        canvas.document.apply(DocumentEdit::Select(None))?;
        let ctrl = modifiers(false, true, false);
        assert!(canvas.drag(&[[4.0, 30.0], [96.0, 194.0]], ctrl, context)?);
        assert_eq!(canvas.selection(), region(4, 30, 92, 164)?);
        // The new selection's own edges become targets for the next drag.
        assert!(canvas.drag(&[[30.0, 20.0], [91.0, 28.0]], NONE, context)?);
        assert_eq!(canvas.selection(), region(30, 20, 66, 10)?);
        Ok(())
    })
}

#[test]
fn dragging_inside_the_selection_moves_its_outline() -> harness::TestResult {
    run(|context| {
        let mut canvas = canvas(context, ShapeTarget::Selection, ShapeKind::Ellipse)?;
        canvas.drag(&[[20.0, 30.0], [60.0, 70.0]], NONE, context)?;
        let before = canvas.document.snapshot().selection().cloned();
        canvas.point([40.0, 50.0], NONE)?;
        canvas.button(true, NONE)?;
        canvas.point([55.4, 44.6], NONE)?;
        assert_eq!(canvas.tools.overlay(), Some(ToolOverlay::SelectionOffset([15, -5])));
        canvas.button(false, NONE)?;
        assert_eq!(canvas.tools.overlay(), None);
        assert!(canvas.settle(context)?);
        let moved = canvas.document.snapshot().selection().cloned();
        let expected =
            before.map(|selection| selection.translate([15, -5], context)).transpose()?;
        assert_eq!(moved, expected);
        // Alt inside the selection subtracts instead of moving.
        let unsnapped_alt = modifiers(false, true, true);
        canvas.drag(&[[40.0, 40.0], [100.0, 100.0]], unsnapped_alt, context)?;
        let selection = canvas.document.snapshot().selection().cloned();
        assert_eq!(
            selection.map(|selection| [selection.value(55, 50), selection.value(55, 30)]),
            Some([0, FULL_COVERAGE])
        );
        // A click without motion moves nothing.
        assert!(!canvas.drag(&[[50.0, 30.0], [50.0, 30.0]], NONE, context)?);
        Ok(())
    })
}

#[test]
fn outlines_follow_the_drag_and_escape_cancels_it() -> harness::TestResult {
    run(|context| {
        let mut canvas = canvas(context, ShapeTarget::Selection, ShapeKind::Ellipse)?;
        canvas.point([20.0, 30.0], NONE)?;
        canvas.button(true, NONE)?;
        assert!(canvas.tools.captured());
        assert_eq!(canvas.tools.overlay(), None);
        canvas.point([60.0, 70.0], NONE)?;
        assert_eq!(
            canvas.tools.overlay(),
            Some(ToolOverlay::Outline(OverlayShape::Ellipse([20.0, 30.0, 60.0, 70.0])))
        );
        canvas.tools.cancel(ToolCancelReason::Requested, Some(&canvas.document), Duration::ZERO)?;
        assert_eq!(canvas.tools.overlay(), None);
        assert!(!canvas.tools.captured());
        canvas.button(false, NONE)?;
        assert!(canvas.tools.take_deferred().is_none());
        assert_eq!(canvas.document.snapshot().revision(), 1);

        let mut lasso = canvas_with_lasso(context)?;
        let path = [[20.0, 20.0], [80.0, 20.0], [80.0, 20.0], [50.0, 90.0]];
        lasso.point(path[0], NONE)?;
        lasso.button(true, NONE)?;
        for point in &path[1..] {
            lasso.point(*point, NONE)?;
        }
        let Some(ToolOverlay::Outline(OverlayShape::Polygon(points))) = lasso.tools.overlay()
        else {
            return Err(CanvasError::Invalid("No lasso outline"));
        };
        assert_eq!(points, &[[20.0, 20.0], [80.0, 20.0], [50.0, 90.0]]);
        lasso.button(false, NONE)?;
        assert!(lasso.settle(context)?);
        let selection = lasso.document.snapshot().selection().cloned();
        assert_eq!(
            selection.as_ref().map(|s| (s.value(50, 40), s.value(25, 80))),
            Some((FULL_COVERAGE, 0))
        );
        Ok(())
    })
}

fn canvas_with_lasso(context: &JobContext) -> Result<Canvas, CanvasError> {
    canvas(context, ShapeTarget::Selection, ShapeKind::Lasso)
}

fn with_mask(canvas: &mut Canvas, context: &JobContext, locked: bool) -> Result<(), CanvasError> {
    let coverage = Coverage::new(canvas.document.snapshot().extent(), 0, context.memory())?;
    let appearance = MaskAppearance::new([255, 0, 0], 0.5)?;
    let layer = Layer::new(LayerId::new(MASK)?, LayerContent::Coverage { coverage, appearance })?
        .with_locked(locked);
    canvas.document.apply(DocumentEdit::Insert { index: 0, layer })?;
    Ok(())
}

#[test]
fn mask_shapes_add_to_the_target_and_subtract_with_alt() -> harness::TestResult {
    run(|context| {
        let mut canvas = canvas(context, ShapeTarget::Mask, ShapeKind::Rectangle)?;
        with_mask(&mut canvas, context, false)?;
        // Without a target there is nothing to draw into.
        assert!(!canvas.drag(&[[20.0, 30.0], [60.0, 70.0]], NONE, context)?);
        canvas.tools.set_target(Some(LayerId::new(MASK)?));
        assert!(canvas.drag(&[[20.0, 30.0], [60.0, 70.0]], NONE, context)?);
        assert!(canvas.drag(&[[110.0, 30.0], [150.0, 70.0]], NONE, context)?);
        assert_eq!(canvas.mask().and_then(|mask| mask.bounds()), region(20, 30, 130, 40)?);
        assert_eq!(canvas.document.snapshot().selection(), None);
        let alt = modifiers(false, false, true);
        assert!(canvas.drag(&[[10.0, 10.0], [65.0, 80.0]], alt, context)?);
        assert_eq!(canvas.mask().and_then(|mask| mask.bounds()), region(110, 30, 40, 40)?);
        // Pressing inside a selection draws a mask shape rather than moving the selection.
        canvas.document.apply(DocumentEdit::Select(canvas.mask()))?;
        assert!(canvas.drag(&[[120.0, 40.0], [130.0, 120.0]], NONE, context)?);
        assert_eq!(canvas.mask().and_then(|mask| mask.bounds()), region(110, 30, 40, 90)?);
        let mut locked = canvas_locked(context)?;
        assert!(!locked.drag(&[[20.0, 30.0], [60.0, 70.0]], NONE, context)?);
        Ok(())
    })
}

fn canvas_locked(context: &JobContext) -> Result<Canvas, CanvasError> {
    let mut canvas = canvas(context, ShapeTarget::Mask, ShapeKind::Ellipse)?;
    with_mask(&mut canvas, context, true)?;
    canvas.tools.set_target(Some(LayerId::new(MASK)?));
    Ok(canvas)
}

#[test]
fn feedback_reports_precise_modes_translation_and_unavailability() -> harness::TestResult {
    run(|context| {
        let extent = Extent::new(200, 200)?;
        let navigation = Navigation::new(Viewport::new(extent)?);
        let mut document = Document::new(1, extent, context.memory())?;
        let view = |document: &Document| crate::ToolView {
            navigation: &navigation,
            document: Some(document.snapshot()),
            target: None,
        };
        let mut tool = CoverageShapeTool::new(ShapeTarget::Selection, ShapeKind::Rectangle);
        assert_eq!(tool.feedback(&view(&document)), Feedback::Unavailable);
        tool.available = true;
        tool.position = Some([40.0, 40.0]);
        assert_eq!(tool.feedback(&view(&document)), Feedback::Precise);
        tool.modifiers = modifiers(true, false, false);
        assert_eq!(tool.feedback(&view(&document)), Feedback::PreciseAdd);
        // Alt before the first selection constrains rather than subtracts.
        tool.modifiers = modifiers(false, false, true);
        assert_eq!(tool.feedback(&view(&document)), Feedback::Precise);
        let square = sugarsubstitute_image_ops::Path::rectangle([20.0, 20.0, 60.0, 60.0])?
            .rasterize(extent, context)?;
        document.apply(DocumentEdit::Select(Some(square)))?;
        assert_eq!(tool.feedback(&view(&document)), Feedback::PreciseSubtract);
        tool.modifiers = NONE;
        assert_eq!(tool.feedback(&view(&document)), Feedback::Translate);
        tool.position = Some([100.0, 100.0]);
        assert_eq!(tool.feedback(&view(&document)), Feedback::Precise);
        let mut mask = CoverageShapeTool::new(ShapeTarget::Mask, ShapeKind::Lasso);
        mask.available = true;
        mask.position = Some([40.0, 40.0]);
        assert_eq!(mask.feedback(&view(&document)), Feedback::Precise);
        Ok(())
    })
}

#[test]
fn visible_mask_content_bounds_are_snap_targets() -> harness::TestResult {
    run(|context| {
        let mut canvas = canvas(context, ShapeTarget::Selection, ShapeKind::Rectangle)?;
        let extent = canvas.document.snapshot().extent();
        let coverage = sugarsubstitute_image_ops::Path::rectangle([30.0, 40.0, 70.0, 90.0])?
            .rasterize(extent, context)?;
        let appearance = MaskAppearance::new([255, 0, 0], 0.5)?;
        let layer =
            Layer::new(LayerId::new(MASK)?, LayerContent::Coverage { coverage, appearance })?;
        canvas.document.apply(DocumentEdit::Insert { index: 0, layer: layer.clone() })?;
        assert!(canvas.drag(&[[36.0, 44.0], [120.0, 91.0]], NONE, context)?);
        assert_eq!(canvas.selection(), region(30, 40, 90, 50)?);
        canvas.document.apply(DocumentEdit::Select(None))?;
        canvas.document.apply(DocumentEdit::Replace(layer.with_visibility(false)))?;
        assert!(canvas.drag(&[[36.0, 44.0], [120.0, 91.0]], NONE, context)?);
        assert_eq!(canvas.selection(), region(36, 44, 84, 47)?);
        Ok(())
    })
}
