// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Toolbar turns and flips, free-transform handles, selection transforms and settlement.
use super::*;
use crate::tools::TransformAction;
use crate::tools::tests::editing::{
    Canvas, MASK, OTHER, TOOL, canvas, held, mask_layer, region, run,
};
use crate::{DocumentEdit, ResizeDirection, ToolId, tests::harness::TestResult};
use std::time::Duration;
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_image_ops::{Coverage, FULL_COVERAGE, Region};

const NONE: ToolModifiers = ToolModifiers::NONE;

fn turn(action: TransformAction) -> ToolCommand {
    ToolCommand::Transform(action)
}

fn corners(canvas: &Canvas) -> Result<[[f64; 2]; 4], CanvasError> {
    match canvas.tools.overlay() {
        Some(ToolOverlay::Frame(corners)) => Ok(corners),
        _ => Err(CanvasError::Invalid("No transform frame")),
    }
}

fn assert_corners(actual: [[f64; 2]; 4], expected: [[f64; 2]; 4]) {
    for (actual, expected) in actual.iter().zip(&expected) {
        assert!(
            (actual[0] - expected[0]).abs() < 1e-9 && (actual[1] - expected[1]).abs() < 1e-9,
            "{actual:?} != {expected:?}"
        );
    }
}

/// A transform canvas whose mask holds irregular values in an odd-by-even rectangle.
fn irregular(context: &JobContext) -> Result<(Canvas, Coverage), CanvasError> {
    let mut canvas = canvas(context, TransformTool::new(), &[[0, 0, 1, 1]])?;
    let extent = canvas.document.snapshot().extent();
    let mut coverage = Coverage::new(extent, 0, context.memory())?;
    coverage.update(Region::new(21, 30, 43, 62)?, context.memory(), |x, y, _| {
        ((x * 977 + y * 613) % 65_521 + 1) as u16
    })?;
    canvas.document.apply(DocumentEdit::Replace(mask_layer(MASK, coverage.clone())?))?;
    Ok((canvas, coverage))
}

fn sorted(coverage: &Coverage) -> Result<Vec<u16>, CanvasError> {
    let mut values = vec![0; coverage.extent().pixels() as usize];
    coverage.read(coverage.extent().region(), &mut values)?;
    values.sort_unstable();
    Ok(values)
}

#[test]
fn quarter_turns_and_flips_are_exact_and_opposite_actions_cancel() -> TestResult {
    run(|context| {
        let (mut canvas, original) = irregular(context)?;
        assert!(canvas.command(turn(TransformAction::RotateRight), context)?);
        let turned =
            Canvas::coverage(&canvas.shown(), MASK).ok_or(CanvasError::Invalid("No mask"))?;
        assert_eq!(turned.bounds().map(|bounds| [bounds.width(), bounds.height()]), Some([62, 43]));
        assert_eq!(sorted(&turned)?, sorted(&original)?);
        assert!(canvas.command(turn(TransformAction::RotateLeft), context)?);
        // Back at the original: nothing to preview, but the session stays open.
        assert!(canvas.tools.preview_snapshot().is_none());
        assert!(canvas.tools.session().is_some_and(|session| session.undo));
        for _ in 0..4 {
            canvas.command(turn(TransformAction::RotateRight), context)?;
        }
        assert!(canvas.tools.preview_snapshot().is_none());
        for action in [TransformAction::FlipHorizontal, TransformAction::FlipVertical] {
            canvas.command(turn(action), context)?;
            assert!(canvas.tools.preview_snapshot().is_some());
            canvas.command(turn(action), context)?;
            assert!(canvas.tools.preview_snapshot().is_none());
        }
        canvas.command(turn(TransformAction::FlipHorizontal), context)?;
        let flipped =
            Canvas::coverage(&canvas.shown(), MASK).ok_or(CanvasError::Invalid("No mask"))?;
        assert_eq!(flipped.value(63, 30), original.value(21, 30));
        let revision = canvas.document.snapshot().revision();
        assert!(canvas.command(ToolCommand::Commit, context)?);
        assert_eq!(canvas.document.snapshot().revision(), revision + 1);
        assert_eq!(canvas.tools.selected(), ToolId::new(OTHER)?);
        assert!(canvas.tools.session().is_none() && canvas.tools.preview_snapshot().is_none());
        let landed = Canvas::coverage(&canvas.document.snapshot(), MASK);
        assert_eq!(landed.map(|coverage| coverage.fingerprint()), Some(flipped.fingerprint()));
        assert!(canvas.document.undo()?);
        let restored = Canvas::coverage(&canvas.document.snapshot(), MASK);
        assert_eq!(restored.map(|coverage| coverage.fingerprint()), Some(original.fingerprint()));
        Ok(())
    })
}

#[test]
fn cancel_restores_the_base_and_session_history_steps_through_actions() -> TestResult {
    run(|context| {
        let mut canvas = canvas(context, TransformTool::new(), &[[20, 20, 60, 40]])?;
        let revision = canvas.document.snapshot().revision();
        canvas.command(turn(TransformAction::RotateRight), context)?;
        canvas.command(turn(TransformAction::FlipVertical), context)?;
        assert!(canvas.command(ToolCommand::Session(SessionAction::Undo), context)?);
        let session = canvas.tools.session().ok_or(CanvasError::Invalid("No session"))?;
        assert_eq!((session.undo, session.redo), (true, true));
        assert_eq!(canvas.shown_bounds(MASK), region(30, 10, 20, 40)?);
        assert!(canvas.command(ToolCommand::Cancel, context)?);
        assert_eq!(canvas.document.snapshot().revision(), revision);
        assert!(canvas.tools.preview_snapshot().is_none() && canvas.tools.session().is_none());
        assert_eq!(canvas.tools.selected(), ToolId::new(OTHER)?);
        // Enter without a change only returns to the previous tool.
        canvas.tools.select(ToolId::new(TOOL)?, Some(&canvas.document), Duration::ZERO)?;
        assert!(canvas.command(ToolCommand::Commit, context)?);
        assert_eq!(canvas.document.snapshot().revision(), revision);
        assert_eq!(canvas.tools.selected(), ToolId::new(OTHER)?);
        Ok(())
    })
}

#[test]
fn handles_scale_skew_rotate_and_move_the_frame() -> TestResult {
    run(|context| {
        let square = [[20.0, 20.0], [60.0, 20.0], [60.0, 60.0], [20.0, 60.0]];
        let fresh = || canvas(context, TransformTool::new(), &[[20, 20, 60, 60]]);
        let mut canvas = fresh()?;
        assert_corners(corners(&canvas)?, square);
        for (point, cursor) in [
            ([60.0, 40.0], CursorIntent::Resize(ResizeDirection::Horizontal)),
            ([60.0, 60.0], CursorIntent::Resize(ResizeDirection::DiagonalDown)),
            ([40.0, 20.0], CursorIntent::Resize(ResizeDirection::Vertical)),
            ([40.0, 40.0], CursorIntent::Move),
            ([75.0, 40.0], CursorIntent::Rotate),
            ([150.0, 150.0], CursorIntent::Arrow),
        ] {
            canvas.point(point, NONE, context)?;
            assert_eq!(canvas.tools.cursor(Some(&canvas.document)), cursor, "{point:?}");
        }
        // Corners scale proportionally, projecting the drag onto the corner's diagonal.
        canvas.drag(&[[60.0, 60.0], [80.0, 70.0]], NONE, context)?;
        assert_corners(corners(&canvas)?, [[20.0, 20.0], [75.0, 20.0], [75.0, 75.0], [20.0, 75.0]]);
        let scaled =
            Canvas::coverage(&canvas.shown(), MASK).ok_or(CanvasError::Invalid("No mask"))?;
        assert_eq!([scaled.value(70, 70), scaled.value(90, 90)], [FULL_COVERAGE, 0]);
        assert!(canvas.tools.session().is_some_and(|session| session.undo));
        // Shift frees the aspect ratio; Alt scales about the centre.
        let mut canvas = fresh()?;
        canvas.drag(&[[60.0, 60.0], [80.0, 70.0]], held(true, false, false), context)?;
        assert_corners(corners(&canvas)?, [[20.0, 20.0], [80.0, 20.0], [80.0, 70.0], [20.0, 70.0]]);
        let mut canvas = fresh()?;
        canvas.drag(&[[60.0, 60.0], [70.0, 70.0]], held(false, false, true), context)?;
        assert_corners(corners(&canvas)?, [[10.0, 10.0], [70.0, 10.0], [70.0, 70.0], [10.0, 70.0]]);
        // Ctrl+Shift on a side handle skews.
        let mut canvas = fresh()?;
        canvas.drag(&[[60.0, 40.0], [60.0, 50.0]], held(true, true, false), context)?;
        assert_corners(corners(&canvas)?, [[20.0, 20.0], [60.0, 30.0], [60.0, 70.0], [20.0, 60.0]]);
        // Inside the box moves it by whole pixels, exactly.
        let mut canvas = fresh()?;
        canvas.drag(&[[40.0, 40.0], [45.6, 52.2]], NONE, context)?;
        assert_eq!(canvas.shown_bounds(MASK), region(26, 32, 40, 40)?);
        // Outside the box rotates about its centre; Shift snaps to 15°.
        let mut canvas = fresh()?;
        canvas.drag(&[[75.0, 40.0], [40.0, 75.0]], NONE, context)?;
        assert_corners(corners(&canvas)?, [[60.0, 20.0], [60.0, 60.0], [20.0, 60.0], [20.0, 20.0]]);
        let mut canvas = fresh()?;
        canvas.drag(&[[75.0, 40.0], [75.0, 52.0]], held(true, false, false), context)?;
        let [top_left, ..] = corners(&canvas)?;
        let angle = (top_left[1] - 40.0).atan2(top_left[0] - 40.0).to_degrees();
        assert!((angle - (-135.0 + 15.0)).abs() < 1e-9, "{angle}");
        // Each release was one checkpoint.
        assert!(canvas.command(ToolCommand::Session(SessionAction::Undo), context)?);
        assert_corners(corners(&canvas)?, square);
        Ok(())
    })
}

#[test]
fn selections_transform_their_pixels_and_double_clicks_apply() -> TestResult {
    run(|context| {
        let mut canvas = canvas(context, TransformTool::new(), &[[40, 40, 50, 60]])?;
        canvas.select([40, 40, 60, 60], context)?;
        canvas.point([0.0, 0.0], NONE, context)?;
        assert_corners(corners(&canvas)?, [[40.0, 40.0], [60.0, 40.0], [60.0, 60.0], [40.0, 60.0]]);
        canvas.command(turn(TransformAction::FlipHorizontal), context)?;
        assert_eq!(canvas.shown_bounds(MASK), region(50, 40, 10, 20)?);
        let shown = canvas.shown();
        assert_eq!(shown.selection().and_then(Coverage::bounds), region(40, 40, 20, 20)?);
        let revision = canvas.document.snapshot().revision();
        canvas.tools.input(
            ToolInput::DoubleClick([50.0, 50.0]),
            Some(&mut canvas.document),
            Duration::ZERO,
        )?;
        canvas.settle(context)?;
        assert_eq!(canvas.document.snapshot().revision(), revision + 1);
        assert_eq!(canvas.bounds(MASK), region(50, 40, 10, 20)?);
        assert_eq!(canvas.tools.selected(), ToolId::new(OTHER)?);
        Ok(())
    })
}
