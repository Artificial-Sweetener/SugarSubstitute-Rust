// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Seam discovery, sub-pixel and snapped edge moves, endpoint pivots and the session.
use super::seams::Shape;
use super::*;
use crate::tools::tests::editing::{Canvas, MASK, OTHER, TOOL, canvas, held, run};
use crate::{DocumentEdit, LayerId};
use crate::{ToolId, tests::harness::TestResult};
use std::time::Duration;
use sugarsubstitute_image_ops::CoverageAffine;
use sugarsubstitute_image_ops::{Coverage, FULL_COVERAGE};

const NONE: ToolModifiers = ToolModifiers::NONE;

fn rect(
    layer: u64,
    [left, top, right, bottom]: [f64; 4],
    eligible: bool,
) -> Result<Shape, CanvasError> {
    Ok(Shape {
        layer: LayerId::new(layer)?,
        eligible,
        mapping: CoverageMapping::Affine(CoverageAffine::IDENTITY),
        boundary: vec![[left, top], [right, top], [right, bottom], [left, bottom]],
    })
}

fn shown(canvas: &Canvas, layer: u64) -> Result<Coverage, CanvasError> {
    Canvas::coverage(&canvas.shown(), layer).ok_or(CanvasError::Invalid("No mask"))
}

#[test]
fn seams_join_every_mask_along_one_line_and_skip_ineligible_layers() -> Result<(), CanvasError> {
    let shapes = [
        rect(1, [0.0, 0.0, 100.0, 200.0], true)?,
        rect(2, [100.0, 0.0, 200.0, 100.0], true)?,
        rect(3, [100.0, 100.0, 200.0, 200.0], true)?,
        // Touches the first mask's bottom for less than 8 device pixels.
        rect(4, [95.0, 200.0, 99.0, 210.0], true)?,
    ];
    let discovery = Discovery::new(&shapes, 1.0);
    assert_eq!(discovery.seams.len(), 2);
    let vertical = discovery
        .seams
        .iter()
        .find(|seam| seam.edge.tangent() == [0.0, 1.0])
        .ok_or(CanvasError::Invalid("No vertical seam"))?;
    assert_eq!([vertical.start(), vertical.end()], [[100.0, 0.0], [100.0, 200.0]]);
    assert_eq!(vertical.layers().len(), 3);
    // Near the junction both seams are in reach, but only one has an endpoint there.
    assert!(discovery.seam_at([103.0, 103.0], 1.0).is_some());
    assert_eq!(discovery.seam_at([150.0, 50.0], 1.0), None);
    assert_eq!(handle_at(vertical, [101.0, 4.0], 1.0), Handle::Start);
    assert_eq!(handle_at(vertical, [101.0, 50.0], 1.0), Handle::Middle);
    // A layer that cannot move along the seam's line removes it.
    let mut blocked = shapes.to_vec();
    blocked.push(rect(5, [100.0, 0.0, 150.0, 50.0], false)?);
    assert!(
        Discovery::new(&blocked, 1.0).seams.iter().all(|seam| seam.edge.tangent() != [0.0, 1.0])
    );
    // Zoomed far out, a one-pixel gap lies within a quarter device pixel.
    let gapped =
        [rect(1, [0.0, 0.0, 100.0, 200.0], true)?, rect(2, [101.0, 0.0, 200.0, 200.0], true)?];
    assert!(Discovery::new(&gapped, 1.0).seams.is_empty());
    assert_eq!(Discovery::new(&gapped, 5.0).seams.len(), 1);
    Ok(())
}

#[test]
fn middle_drags_move_by_fractions_and_stop_before_a_mask_collapses() -> TestResult {
    run(|context| {
        let mut canvas =
            canvas(context, SharedEdgesTool::new(), &[[0, 0, 100, 200], [100, 0, 200, 200]])?;
        let revision = canvas.document.snapshot().revision();
        canvas.point([101.0, 150.0], NONE, context)?;
        let cursor = canvas.tools.cursor(Some(&canvas.document));
        assert_eq!(cursor, CursorIntent::Resize(ResizeDirection::Horizontal));
        // A fraction of a pixel lands as a soft edge on both sides of the seam.
        canvas.drag(&[[101.0, 150.0], [121.5, 160.0]], NONE, context)?;
        let (left, right) = (shown(&canvas, MASK)?, shown(&canvas, MASK + 1)?);
        assert_eq!([left.value(119, 150), left.value(122, 150)], [FULL_COVERAGE, 0]);
        assert!(left.value(120, 150) > 0 && left.value(120, 150) < FULL_COVERAGE);
        assert_eq!([right.value(119, 150), right.value(122, 150)], [0, FULL_COVERAGE]);
        let session = canvas.tools.session().ok_or(CanvasError::Invalid("No session"))?;
        assert_eq!((session.kind, session.undo), (SessionKind::SharedEdges, true));
        // A drag past the far edge stops where the second mask is two device pixels thin.
        canvas.drag(&[[120.5, 150.0], [400.0, 150.0]], NONE, context)?;
        let right = shown(&canvas, MASK + 1)?;
        assert_eq!([right.value(199, 150), right.value(196, 150)], [FULL_COVERAGE, 0]);
        let other = ToolId::new(OTHER)?;
        assert!(canvas.tools.select(other, Some(&canvas.document), Duration::ZERO).is_err());
        assert!(canvas.command(ToolCommand::Session(SessionAction::Undo), context)?);
        assert_eq!(shown(&canvas, MASK)?.value(119, 150), FULL_COVERAGE);
        assert_eq!(canvas.document.snapshot().revision(), revision);
        assert!(canvas.command(ToolCommand::Commit, context)?);
        assert_eq!(canvas.document.snapshot().revision(), revision + 1);
        assert!(canvas.tools.session().is_none());
        assert_eq!(canvas.tools.selected(), ToolId::new(TOOL)?);
        Ok(())
    })
}

#[test]
fn middle_drags_snap_to_parallel_edges_of_other_layers_unless_ctrl_is_held() -> TestResult {
    run(|context| {
        let masks = [[0, 0, 100, 200], [100, 0, 200, 200], [140, 0, 190, 40]];
        let mut snapped = canvas(context, SharedEdgesTool::new(), &masks)?;
        snapped.drag(&[[101.0, 150.0], [137.5, 150.0]], NONE, context)?;
        let left = shown(&snapped, MASK)?;
        // The seam lands on the third mask's left edge at x = 140, not at 136.5.
        assert_eq!([left.value(138, 150), left.value(141, 150)], [FULL_COVERAGE, 0]);
        let mut free = canvas(context, SharedEdgesTool::new(), &masks)?;
        free.drag(&[[101.0, 150.0], [137.5, 150.0]], held(false, true, false), context)?;
        assert_eq!(shown(&free, MASK)?.value(138, 150), 0);
        Ok(())
    })
}
#[test]
fn endpoints_pivot_along_the_shared_rail_and_join_its_end() -> TestResult {
    run(|context| {
        let mut canvas =
            canvas(context, SharedEdgesTool::new(), &[[0, 0, 100, 100], [100, 0, 200, 100]])?;
        canvas.point([100.0, 2.0], NONE, context)?;
        let cursor = canvas.tools.cursor(Some(&canvas.document));
        assert_eq!(cursor, CursorIntent::Affine(AffineCursor::resize(0.0)));
        // Dragging the top end right turns both masks into trapezoids along the rail.
        canvas.drag(&[[100.0, 2.0], [130.0, 3.0]], NONE, context)?;
        let (left, right) = (shown(&canvas, MASK)?, shown(&canvas, MASK + 1)?);
        assert_eq!([left.value(115, 2), right.value(115, 2)], [FULL_COVERAGE, 0]);
        assert_eq!([left.value(102, 97), right.value(102, 97)], [0, FULL_COVERAGE]);
        // Within 6 device pixels of the rail's end the point joins it and the right mask's top
        // edge collapses into a triangle.
        canvas.drag(&[[130.0, 1.0], [196.0, 1.0]], NONE, context)?;
        let right = shown(&canvas, MASK + 1)?;
        assert_eq!([right.value(190, 90), right.value(150, 20)], [FULL_COVERAGE, 0]);
        assert_eq!(shown(&canvas, MASK)?.value(150, 20), FULL_COVERAGE);
        // Escape cancels the session; a second Escape leaves the tool.
        let revision = canvas.document.snapshot().revision();
        assert!(canvas.command(ToolCommand::Cancel, context)?);
        assert!(canvas.tools.session().is_none() && canvas.tools.preview_snapshot().is_none());
        assert_eq!(canvas.document.snapshot().revision(), revision);
        assert!(canvas.command(ToolCommand::Cancel, context)?);
        assert_eq!(canvas.tools.selected(), ToolId::new(OTHER)?);
        Ok(())
    })
}

#[test]
fn locked_masks_take_part_and_interrupted_drags_become_checkpoints() -> TestResult {
    run(|context| {
        let mut canvas =
            canvas(context, SharedEdgesTool::new(), &[[0, 0, 200, 80], [0, 80, 200, 200]])?;
        let locked = canvas.document.snapshot().layers()[1].clone().with_locked(true);
        canvas.document.apply(DocumentEdit::Replace(locked))?;
        canvas.point([50.0, 80.0], NONE, context)?;
        let cursor = canvas.tools.cursor(Some(&canvas.document));
        assert_eq!(cursor, CursorIntent::Resize(ResizeDirection::Vertical));
        canvas.point([50.0, 80.0], NONE, context)?;
        canvas.button(true, NONE, context)?;
        canvas.point([50.0, 60.0], NONE, context)?;
        canvas.tools.cancel(ToolCancelReason::FocusLost, Some(&canvas.document), Duration::ZERO)?;
        assert!(!canvas.tools.captured());
        assert_eq!(shown(&canvas, MASK + 1)?.value(100, 61), FULL_COVERAGE);
        assert!(canvas.tools.session().is_some_and(|session| session.undo));
        assert!(canvas.command(ToolCommand::Session(SessionAction::Apply), context)?);
        let applied = Canvas::coverage(&canvas.document.snapshot(), MASK + 1);
        assert_eq!(applied.map(|coverage| coverage.value(100, 61)), Some(FULL_COVERAGE));
        Ok(())
    })
}
