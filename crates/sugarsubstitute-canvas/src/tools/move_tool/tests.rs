// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Layer moves, nudges, 45° snapping and floating selections with their sessions.
use super::*;
use crate::tools::tests::editing::{Canvas, MASK, OTHER, TOOL, canvas, held, region, run};
use crate::{DocumentEdit, ToolId, tests::harness::TestResult};
use std::time::Duration;
use sugarsubstitute_image_ops::{Coverage, FULL_COVERAGE, Region};

const NONE: ToolModifiers = ToolModifiers::NONE;

fn mask(snapshot: &DocumentSnapshot) -> Result<Coverage, CanvasError> {
    Canvas::coverage(snapshot, MASK).ok_or(CanvasError::Invalid("No mask"))
}

fn selected(snapshot: &DocumentSnapshot) -> Option<Region> {
    snapshot.selection().and_then(Coverage::bounds)
}

#[test]
fn layer_drags_move_by_whole_pixels_and_land_as_one_undo_step() -> TestResult {
    run(|context| {
        let mut canvas = canvas(context, MoveTool::new(), &[[20, 20, 60, 60]])?;
        let revision = canvas.document.snapshot().revision();
        canvas.point([30.0, 30.0], NONE, context)?;
        assert_eq!(canvas.tools.cursor(Some(&canvas.document)), CursorIntent::Move);
        canvas.button(true, NONE, context)?;
        canvas.point([45.4, 37.6], NONE, context)?;
        assert!(canvas.tools.captured());
        assert_eq!(canvas.shown_bounds(MASK), region(35, 28, 40, 40)?);
        assert_eq!(canvas.document.snapshot().revision(), revision);
        assert!(canvas.tools.session().is_none());
        canvas.button(false, NONE, context)?;
        assert!(!canvas.tools.captured());
        assert_eq!(canvas.bounds(MASK), region(35, 28, 40, 40)?);
        assert!(canvas.document.undo()?);
        assert_eq!(canvas.bounds(MASK), region(20, 20, 40, 40)?);
        // Escape mid-drag drops the move.
        canvas.point([30.0, 30.0], NONE, context)?;
        canvas.button(true, NONE, context)?;
        canvas.point([70.0, 70.0], NONE, context)?;
        assert!(canvas.command(ToolCommand::Cancel, context)?);
        canvas.button(false, NONE, context)?;
        assert!(canvas.tools.preview_snapshot().is_none() && !canvas.tools.captured());
        assert_eq!(canvas.bounds(MASK), region(20, 20, 40, 40)?);
        Ok(())
    })
}

#[test]
fn shift_turns_drags_to_the_nearest_45_degrees_keeping_their_length() -> TestResult {
    run(|context| {
        let mut canvas = canvas(context, MoveTool::new(), &[[20, 20, 60, 60]])?;
        let shift = held(true, false, false);
        canvas.drag(&[[30.0, 30.0], [60.0, 42.0]], shift, context)?;
        // atan2(12, 30) ≈ 21.8° turns to 0°, keeping the length √(30² + 12²) ≈ 32.3.
        assert_eq!(canvas.bounds(MASK), region(52, 20, 40, 40)?);
        assert!(canvas.document.undo()?);
        canvas.drag(&[[30.0, 30.0], [50.0, 48.0]], shift, context)?;
        // ≈ 42° turns to 45°: √(20² + 18²) ≈ 26.9 along the diagonal is 19 pixels each way.
        assert_eq!(canvas.bounds(MASK), region(39, 39, 40, 40)?);
        assert_eq!(constrain([0.0, 0.0], true), [0.0, 0.0]);
        assert_eq!(constrain([3.0, 4.0], false), [3.0, 4.0]);
        Ok(())
    })
}

#[test]
fn nudges_move_the_layer_one_undo_step_each() -> TestResult {
    run(|context| {
        let mut canvas = canvas(context, MoveTool::new(), &[[20, 20, 60, 60]])?;
        assert!(canvas.command(ToolCommand::Nudge([1, 0]), context)?);
        assert!(canvas.command(ToolCommand::Nudge([0, -10]), context)?);
        assert_eq!(canvas.bounds(MASK), region(21, 10, 40, 40)?);
        assert!(canvas.tools.session().is_none());
        assert!(canvas.document.undo()?);
        assert_eq!(canvas.bounds(MASK), region(21, 20, 40, 40)?);
        let far = ToolCommand::Nudge([101, 0]);
        assert!(canvas.tools.command(far, &mut canvas.document, Duration::ZERO).is_err());
        // Nudges are ignored mid-drag.
        canvas.point([30.0, 30.0], NONE, context)?;
        canvas.button(true, NONE, context)?;
        assert!(!canvas.command(ToolCommand::Nudge([1, 0]), context)?);
        Ok(())
    })
}

#[test]
fn selected_pixels_float_until_enter_and_land_with_the_selection() -> TestResult {
    run(|context| {
        let mut canvas = canvas(context, MoveTool::new(), &[[20, 20, 100, 100]])?;
        canvas.select([40, 40, 60, 60], context)?;
        let revision = canvas.document.snapshot().revision();
        canvas.point([10.0, 10.0], NONE, context)?;
        assert_eq!(canvas.tools.cursor(Some(&canvas.document)), CursorIntent::Arrow);
        // A press outside the selection does nothing.
        canvas.button(true, NONE, context)?;
        assert!(!canvas.tools.captured());
        canvas.button(false, NONE, context)?;
        canvas.point([50.0, 50.0], NONE, context)?;
        assert_eq!(canvas.tools.cursor(Some(&canvas.document)), CursorIntent::MoveCut);
        canvas.drag(&[[50.0, 50.0], [80.0, 50.0]], NONE, context)?;
        assert_eq!(canvas.document.snapshot().revision(), revision);
        let session = canvas.tools.session().ok_or(CanvasError::Invalid("No session"))?;
        assert_eq!((session.kind, session.undo), (SessionKind::Move, true));
        let shown = canvas.shown();
        let lifted = mask(&shown)?;
        // The lifted square leaves a hole and lands 30 pixels right.
        let values = [lifted.value(45, 45), lifted.value(75, 45), lifted.value(25, 25)];
        assert_eq!(values, [0, FULL_COVERAGE, FULL_COVERAGE]);
        assert_eq!(selected(&shown), region(70, 40, 20, 20)?);
        // Dragging again continues the same float from inside its moved selection.
        canvas.drag(&[[75.0, 45.0], [75.0, 50.0]], NONE, context)?;
        assert!(canvas.command(ToolCommand::Nudge([0, 1]), context)?);
        assert_eq!(selected(&canvas.shown()), region(70, 46, 20, 20)?);
        // Undo walks back through the session without touching the document.
        assert!(canvas.tools.undo(&mut canvas.document, Duration::ZERO)?);
        canvas.settle(context)?;
        assert_eq!(selected(&canvas.shown()), region(70, 45, 20, 20)?);
        assert!(canvas.command(ToolCommand::Commit, context)?);
        assert!(canvas.tools.session().is_none());
        assert_eq!(canvas.document.snapshot().revision(), revision + 1);
        let snapshot = canvas.document.snapshot();
        assert_eq!(selected(&snapshot), region(70, 45, 20, 20)?);
        let landed = mask(&snapshot)?;
        let values = [landed.value(45, 45), landed.value(85, 60), landed.value(55, 62)];
        assert_eq!(values, [0, FULL_COVERAGE, FULL_COVERAGE]);
        assert!(canvas.document.undo()?);
        assert_eq!(canvas.bounds(MASK), region(20, 20, 80, 80)?);
        Ok(())
    })
}

#[test]
fn escape_discards_a_float_and_alt_copies_instead_of_cutting() -> TestResult {
    run(|context| {
        let mut canvas = canvas(context, MoveTool::new(), &[[20, 20, 100, 100]])?;
        canvas.select([40, 40, 60, 60], context)?;
        let revision = canvas.document.snapshot().revision();
        canvas.drag(&[[50.0, 50.0], [150.0, 150.0]], NONE, context)?;
        assert!(canvas.tools.preview_snapshot().is_some());
        assert!(canvas.command(ToolCommand::Cancel, context)?);
        assert!(canvas.tools.session().is_none() && canvas.tools.preview_snapshot().is_none());
        assert_eq!(canvas.document.snapshot().revision(), revision);
        let alt = held(false, false, true);
        canvas.point([50.0, 50.0], alt, context)?;
        assert_eq!(canvas.tools.cursor(Some(&canvas.document)), CursorIntent::Move);
        canvas.drag(&[[50.0, 50.0], [150.0, 150.0]], alt, context)?;
        assert!(canvas.command(ToolCommand::Commit, context)?);
        let copied = mask(&canvas.document.snapshot())?;
        assert_eq!([copied.value(45, 45), copied.value(155, 155)], [FULL_COVERAGE; 2]);
        Ok(())
    })
}

#[test]
fn floats_survive_navigation_and_focus_loss_but_not_document_changes() -> TestResult {
    run(|context| {
        let mut canvas = canvas(context, MoveTool::new(), &[[20, 20, 100, 100]])?;
        canvas.select([40, 40, 60, 60], context)?;
        assert!(canvas.command(ToolCommand::Nudge([5, 0]), context)?);
        let other = ToolId::new(OTHER)?;
        assert!(canvas.tools.select(other, Some(&canvas.document), Duration::ZERO).is_err());
        canvas.tools.temporary_navigation(true, Some(&mut canvas.document), Duration::ZERO)?;
        assert!(canvas.tools.preview_snapshot().is_some() && canvas.tools.session().is_some());
        canvas.tools.temporary_navigation(false, Some(&mut canvas.document), Duration::ZERO)?;
        // An interrupted drag is kept as a checkpoint.
        canvas.point([50.0, 50.0], NONE, context)?;
        canvas.button(true, NONE, context)?;
        canvas.point([55.0, 60.0], NONE, context)?;
        let now = Duration::ZERO;
        canvas.tools.cancel(ToolCancelReason::FocusLost, Some(&canvas.document), now)?;
        assert!(!canvas.tools.captured());
        assert_eq!(selected(&canvas.shown()), region(50, 50, 20, 20)?);
        assert!(canvas.tools.session().is_some_and(|session| session.undo));
        canvas.tools.cancel(ToolCancelReason::DocumentChanged, Some(&canvas.document), now)?;
        assert!(canvas.tools.session().is_none() && canvas.tools.preview_snapshot().is_none());
        assert_eq!(canvas.tools.selected(), ToolId::new(TOOL)?);
        Ok(())
    })
}

#[test]
fn locked_and_missing_masks_are_never_moved() -> TestResult {
    run(|context| {
        let mut canvas = canvas(context, MoveTool::new(), &[[20, 20, 60, 60]])?;
        let layer = canvas.document.snapshot().layers()[0].clone().with_locked(true);
        canvas.document.apply(DocumentEdit::Replace(layer))?;
        let revision = canvas.document.snapshot().revision();
        canvas.point([30.0, 30.0], NONE, context)?;
        assert_eq!(canvas.tools.cursor(Some(&canvas.document)), CursorIntent::Arrow);
        canvas.drag(&[[30.0, 30.0], [50.0, 50.0]], NONE, context)?;
        assert!(!canvas.command(ToolCommand::Nudge([1, 1]), context)?);
        canvas.tools.set_target(None);
        assert!(!canvas.command(ToolCommand::Nudge([1, 1]), context)?);
        assert_eq!(canvas.document.snapshot().revision(), revision);
        assert!(canvas.tools.preview_snapshot().is_none());
        Ok(())
    })
}

#[test]
fn masks_and_floating_pixels_moved_off_the_document_come_back_whole() -> TestResult {
    run(|context| {
        let mut canvas = canvas(context, MoveTool::new(), &[[150, 20, 190, 60]])?;
        let original = mask(&canvas.document.snapshot())?;
        canvas.drag(&[[160.0, 30.0], [200.0, 30.0]], NONE, context)?;
        assert_eq!(canvas.bounds(MASK), region(190, 20, 10, 40)?);
        assert_eq!(mask(&canvas.document.snapshot())?.content_bounds(), Some([190, 20, 230, 60]));
        canvas.drag(&[[195.0, 30.0], [155.0, 30.0]], NONE, context)?;
        assert_eq!(mask(&canvas.document.snapshot())?, original);
        // Floating pixels landed half beyond the edge return whole with a later move.
        canvas.select([150, 20, 190, 60], context)?;
        canvas.drag(&[[160.0, 30.0], [190.0, 30.0]], NONE, context)?;
        assert!(canvas.command(ToolCommand::Commit, context)?);
        let landed = canvas.document.snapshot();
        assert_eq!(selected(&landed), region(180, 20, 20, 40)?);
        assert_eq!(mask(&landed)?.content_bounds(), Some([180, 20, 220, 60]));
        canvas.document.apply(DocumentEdit::Select(None))?;
        canvas.drag(&[[185.0, 30.0], [155.0, 30.0]], NONE, context)?;
        assert_eq!(mask(&canvas.document.snapshot())?, original);
        Ok(())
    })
}
