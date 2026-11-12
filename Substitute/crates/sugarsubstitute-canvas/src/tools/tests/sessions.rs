// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Session checkpoints, unified undo routing and preview ordering at the manager boundary.
use super::editing::{MASK, OTHER, TOOL, canvas, rectangle, run};
use super::*;
use crate::tools::session::Checkpoints;

#[test]
fn checkpoints_keep_the_newest_256_above_an_immutable_base() -> Result<(), CanvasError> {
    let mut checkpoints = Checkpoints::new(0_u32);
    assert!(!checkpoints.changed() && !checkpoints.record(0)?);
    for state in 1..=300 {
        assert!(checkpoints.record(state)?);
    }
    let state = checkpoints.state(SessionKind::Transform);
    assert_eq!((state.undo, state.redo), (true, false));
    let mut undone = 0;
    while checkpoints.undo() {
        undone += 1;
    }
    // The oldest checkpoints dropped; undo stops at the base and never passes it.
    assert_eq!((undone, *checkpoints.current()), (SESSION_CHECKPOINTS, 0));
    assert!(checkpoints.redo());
    assert_eq!(*checkpoints.current(), 300 - SESSION_CHECKPOINTS as u32 + 1);
    // Recording after undo discards the redoable checkpoints.
    assert!(checkpoints.record(7)?);
    assert!(!checkpoints.redo());
    assert_eq!(*checkpoints.base(), 0);
    Ok(())
}

#[test]
fn sessions_refuse_tool_changes_and_route_undo_until_resolved() -> harness::TestResult {
    run(|context| {
        let mut canvas = canvas(context, TransformTool::new(), &[[20, 20, 60, 40]])?;
        let other = ToolId::new(OTHER)?;
        let revision = canvas.document.snapshot().revision();
        assert!(canvas.command(ToolCommand::Transform(TransformAction::RotateRight), context)?);
        let session = canvas.tools.session().ok_or(CanvasError::Invalid("No session"))?;
        assert_eq!(
            (session.kind, session.undo, session.redo),
            (SessionKind::Transform, true, false)
        );
        assert!(canvas.tools.select(other, Some(&canvas.document), Duration::ZERO).is_err());
        assert_eq!(canvas.tools.selected(), ToolId::new(TOOL)?);
        // Undo inside the session steps it back; the document's history is untouched.
        assert!(canvas.tools.undo(&mut canvas.document, Duration::ZERO)?);
        assert!(!canvas.tools.undo(&mut canvas.document, Duration::ZERO)?);
        assert!(canvas.tools.redo(&mut canvas.document, Duration::ZERO)?);
        assert_eq!(canvas.document.snapshot().revision(), revision);
        // Selection commands wait for the session too.
        assert!(canvas.tools.deselect(&mut canvas.document).is_err());
        // The toolbar's Apply resolves the session and returns to the tool chosen before.
        assert!(canvas.command(ToolCommand::Session(SessionAction::Apply), context)?);
        assert!(canvas.tools.session().is_none());
        assert_eq!(canvas.tools.selected(), other);
        assert!(canvas.tools.undo(&mut canvas.document, Duration::ZERO)?);
        assert_eq!(canvas.bounds(MASK), super::editing::region(20, 20, 40, 20)?);
        Ok(())
    })
}

#[test]
fn obsolete_previews_never_replace_newer_or_discarded_ones() -> harness::TestResult {
    run(|context| {
        let mut canvas = canvas(context, TransformTool::new(), &[[20, 20, 60, 40]])?;
        let turn = |action| ToolCommand::Transform(action);
        canvas.tools.command(
            turn(TransformAction::RotateRight),
            &mut canvas.document,
            Duration::ZERO,
        )?;
        let first = canvas.tools.take_deferred().ok_or(CanvasError::Invalid("No preview"))?;
        canvas.tools.command(
            turn(TransformAction::FlipVertical),
            &mut canvas.document,
            Duration::ZERO,
        )?;
        let second = canvas.tools.take_deferred().ok_or(CanvasError::Invalid("No preview"))?;
        let (first, second) = (first.run(context)?, second.run(context)?);
        assert!(
            canvas.tools.complete_deferred(second, &mut canvas.document, Duration::ZERO)?.applied()
        );
        let staged = canvas.shown();
        assert!(
            !canvas.tools.complete_deferred(first, &mut canvas.document, Duration::ZERO)?.applied()
        );
        assert_eq!(canvas.shown().stage(), staged.stage());
        // Undoing to the base drops the preview; a late result for a checkpoint cannot return.
        canvas.tools.command(
            ToolCommand::Session(SessionAction::Undo),
            &mut canvas.document,
            Duration::ZERO,
        )?;
        let late = canvas.tools.take_deferred().ok_or(CanvasError::Invalid("No preview"))?;
        canvas.tools.command(
            ToolCommand::Session(SessionAction::Undo),
            &mut canvas.document,
            Duration::ZERO,
        )?;
        assert!(canvas.tools.preview_snapshot().is_none());
        let late = late.run(context)?;
        assert!(
            !canvas.tools.complete_deferred(late, &mut canvas.document, Duration::ZERO)?.applied()
        );
        assert!(canvas.tools.preview_snapshot().is_none());
        let extent = canvas.document.snapshot().extent();
        assert_eq!(
            Canvas::coverage(&canvas.shown(), MASK).map(|coverage| coverage.fingerprint()),
            Some(rectangle(extent, [20, 20, 60, 40], context)?.fingerprint())
        );
        Ok(())
    })
}

use super::editing::Canvas;
