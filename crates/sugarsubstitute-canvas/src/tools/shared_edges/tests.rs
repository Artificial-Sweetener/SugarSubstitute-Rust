// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Seam discovery and grouping, coupled resizing, clamping and the shared-edge session.
use super::*;
use crate::tools::tests::editing::{Canvas, MASK, OTHER, TOOL, canvas, region, run};
use crate::{ToolId, Viewport, tests::harness::TestResult};
use std::time::Duration;
use sugarsubstitute_image_ops::{Extent, FULL_COVERAGE};

const NONE: crate::ToolModifiers = crate::ToolModifiers::NONE;

#[test]
fn seams_join_every_mask_along_one_line_and_need_real_overlap() -> Result<(), CanvasError> {
    let frames = [
        [0.0, 0.0, 100.0, 200.0],
        [100.0, 0.0, 200.0, 100.0],
        [100.0, 100.0, 200.0, 200.0],
        // Touches the first mask's bottom corner for less than 8 pixels.
        [95.0, 200.0, 99.0, 210.0],
    ];
    let found = seams::discover(&frames, 1.0);
    assert_eq!(found.len(), 2);
    let vertical = found.iter().find(|seam| seam.axis == Axis::Vertical);
    let horizontal = found.iter().find(|seam| seam.axis == Axis::Horizontal);
    assert_eq!(vertical.map(Seam::ends), Some(([100.0, 0.0], [100.0, 200.0])));
    assert_eq!(horizontal.map(Seam::ends), Some(([100.0, 100.0], [200.0, 100.0])));
    // Dragging the vertical seam moves all three masks beside it.
    let vertical = vertical.ok_or(CanvasError::Invalid("No vertical seam"))?;
    let (moved, delta) = vertical.moved(&frames, 25.0, 1.0);
    assert_eq!(delta, 25.0);
    assert_eq!([moved[0][2], moved[1][0], moved[2][0], moved[3][0]], [125.0, 125.0, 125.0, 95.0]);
    // Masks never collapse below two device pixels.
    assert_eq!(vertical.moved(&frames, 500.0, 1.0).1, 98.0);
    assert_eq!(vertical.moved(&frames, -500.0, 1.0).1, -98.0);
    assert_eq!(vertical.moved(&frames, 500.0, 0.5).1, 96.0);
    // Zoomed far out, a one-pixel gap is within a quarter device pixel.
    let gapped = [[0.0, 0.0, 100.0, 200.0], [101.0, 0.0, 200.0, 200.0]];
    assert!(seams::discover(&gapped, 1.0).is_empty());
    assert_eq!(seams::discover(&gapped, 0.2).len(), 1);
    // Near the junction both seams are in reach, so neither is picked.
    let viewport = Viewport::new(Extent::new(200, 200)?)?;
    assert_eq!(seams::pick(&found, viewport, [103.0, 103.0])?, None);
    assert!(seams::pick(&found, viewport, [104.0, 30.0])?.is_some());
    assert_eq!(seams::pick(&found, viewport, [120.0, 30.0])?, None);
    Ok(())
}

#[test]
fn dragging_a_seam_resizes_both_masks_inside_one_session() -> TestResult {
    run(|context| {
        let mut canvas =
            canvas(context, SharedEdgesTool::new(), &[[0, 0, 100, 200], [100, 0, 200, 200]])?;
        let revision = canvas.document.snapshot().revision();
        canvas.point([101.0, 50.0], NONE, context)?;
        let resize = CursorIntent::Resize(ResizeDirection::Horizontal);
        assert_eq!(canvas.tools.cursor(Some(&canvas.document)), resize);
        assert!(
            matches!(canvas.tools.overlay(), Some(ToolOverlay::Seams(seams)) if seams.len() == 1 && seams[0].focused)
        );
        canvas.drag(&[[101.0, 50.0], [131.4, 90.0]], NONE, context)?;
        assert_eq!(canvas.shown_bounds(MASK), region(0, 0, 130, 200)?);
        assert_eq!(canvas.shown_bounds(MASK + 1), region(130, 0, 70, 200)?);
        let session = canvas.tools.session().ok_or(CanvasError::Invalid("No session"))?;
        assert_eq!(
            (session.kind, session.undo, session.redo),
            (SessionKind::SharedEdges, true, false)
        );
        assert!(
            canvas
                .tools
                .select(ToolId::new(OTHER)?, Some(&canvas.document), Duration::ZERO)
                .is_err()
        );
        // A second drag works from the moved seam.
        canvas.drag(&[[130.0, 20.0], [110.0, 20.0]], NONE, context)?;
        assert_eq!(canvas.shown_bounds(MASK + 1), region(110, 0, 90, 200)?);
        assert!(canvas.command(ToolCommand::Session(SessionAction::Undo), context)?);
        assert_eq!(canvas.shown_bounds(MASK), region(0, 0, 130, 200)?);
        assert_eq!(canvas.document.snapshot().revision(), revision);
        assert!(canvas.command(ToolCommand::Session(SessionAction::Apply), context)?);
        assert!(canvas.tools.session().is_none());
        assert_eq!(canvas.tools.selected(), ToolId::new(TOOL)?);
        assert_eq!(canvas.document.snapshot().revision(), revision + 1);
        let snapshot = canvas.document.snapshot();
        let (left, right) =
            (Canvas::coverage(&snapshot, MASK), Canvas::coverage(&snapshot, MASK + 1));
        let (Some(left), Some(right)) = (left, right) else {
            return Err(CanvasError::Invalid("Missing masks"));
        };
        for x in [0, 99, 100, 129, 130, 199] {
            let total = u32::from(left.value(x, 7)) + u32::from(right.value(x, 7));
            assert_eq!(total, u32::from(FULL_COVERAGE), "column {x}");
        }
        assert!(canvas.document.undo()?);
        assert_eq!(canvas.bounds(MASK), region(0, 0, 100, 200)?);
        Ok(())
    })
}

#[test]
fn escape_cancels_a_session_and_then_leaves_the_tool() -> TestResult {
    run(|context| {
        let mut canvas =
            canvas(context, SharedEdgesTool::new(), &[[0, 0, 200, 80], [0, 80, 200, 200]])?;
        // Every mask takes part, locked or not, while this tool is open.
        let locked = canvas.document.snapshot().layers()[1].clone().with_locked(true);
        canvas.document.apply(DocumentEdit::Replace(locked))?;
        let revision = canvas.document.snapshot().revision();
        canvas.point([50.0, 80.0], NONE, context)?;
        let resize = CursorIntent::Resize(ResizeDirection::Vertical);
        assert_eq!(canvas.tools.cursor(Some(&canvas.document)), resize);
        canvas.drag(&[[50.0, 80.0], [50.0, 60.0]], NONE, context)?;
        assert_eq!(canvas.shown_bounds(MASK + 1), region(0, 60, 200, 140)?);
        // A focus loss mid-drag keeps the drag as a checkpoint.
        canvas.drag(&[[50.0, 60.0], [50.0, 50.0]], NONE, context)?;
        canvas.point([50.0, 50.0], NONE, context)?;
        canvas.button(true, NONE, context)?;
        canvas.point([50.0, 40.0], NONE, context)?;
        canvas.tools.cancel(ToolCancelReason::FocusLost, Some(&canvas.document), Duration::ZERO)?;
        assert_eq!(canvas.shown_bounds(MASK + 1), region(0, 40, 200, 160)?);
        assert!(canvas.command(ToolCommand::Cancel, context)?);
        assert!(canvas.tools.session().is_none() && canvas.tools.preview_snapshot().is_none());
        assert_eq!(canvas.document.snapshot().revision(), revision);
        assert_eq!(canvas.tools.selected(), ToolId::new(TOOL)?);
        assert!(canvas.command(ToolCommand::Cancel, context)?);
        assert_eq!(canvas.tools.selected(), ToolId::new(OTHER)?);
        Ok(())
    })
}
