// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A mask shape whose job an undo or a brush stroke outdates runs again and lands after that
//! change as its own undo step. Completions are observed only when the fixture settles, so each
//! change below happens while the shape's job is running or finished but unobserved.

use super::*;

/// Paint and erase with hard, opaque 16 px dabs.
fn hard_brushes(fixture: &mut Fixture) -> Result<(), Box<dyn Error>> {
    for tool in [CanvasBrushTool::Brush, CanvasBrushTool::Eraser] {
        let options = CanvasBrushOptions::new(16, 100, 100)?;
        fixture.edit(CanvasEditCommand::Options { tool, options })?;
    }
    Ok(())
}

#[test]
fn a_shape_whose_job_an_undo_outdates_lands_after_the_undo() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    {
        let image = show(&mut fixture, Some("first"))?;
        let first = layer(&image, "first")?;
        hard_brushes(&mut fixture)?;
        fixture.edit(CanvasEditCommand::Tool(CanvasToolId::Brush))?;
        fixture.drag(&[[2.0, 34.0], [60.0, 34.0]])?;
        fixture.settle_inputs()?;
        let stroked = fixture.snapshot(image.document)?.revision();
        fixture.edit(CanvasEditCommand::Tool(CanvasToolId::MaskRectangle))?;
        fixture.drag(&[[20.0, 2.0], [30.0, 12.0]])?;
        assert!(fixture.session.editing.in_flight(), "the shape's job has started");
        // The job read the stroke, which this undo removes before its result is observed.
        fixture.edit(CanvasEditCommand::History(CanvasHistoryAction::Undo))?;
        fixture.settle_inputs()?;
        let after = fixture.snapshot(image.document)?;
        let painted = coverage(&after, first)?;
        assert_eq!(painted.value(25, 7), FULL_COVERAGE, "the shape lands");
        assert_eq!(painted.value(10, 34), 0, "after the undo that came first");
        assert_eq!(after.revision(), stroked + 2, "the undo, then the shape");
        assert_eq!(fixture.session.edit_state()?.history, [true, false]);
        fixture.edit(CanvasEditCommand::History(CanvasHistoryAction::Undo))?;
        assert_eq!(coverage(&fixture.snapshot(image.document)?, first)?.uniform(), Some(0));
    }
    fixture.finish().map(drop)
}

#[test]
fn a_shape_finishing_during_a_brush_stroke_lands_after_the_stroke() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    {
        let image = show(&mut fixture, Some("first"))?;
        let first = layer(&image, "first")?;
        hard_brushes(&mut fixture)?;
        fixture.edit(CanvasEditCommand::Tool(CanvasToolId::Brush))?;
        fixture.drag(&[[2.0, 7.0], [60.0, 7.0]])?;
        fixture.settle_inputs()?;
        let painted = fixture.snapshot(image.document)?.revision();
        fixture.edit(CanvasEditCommand::Tool(CanvasToolId::MaskRectangle))?;
        fixture.drag(&[[20.0, 2.0], [30.0, 12.0]])?;
        assert!(fixture.session.editing.in_flight(), "the shape's job has started");
        // An eraser stroke through the shape begins before the shape's result is observed.
        fixture.edit(CanvasEditCommand::Tool(CanvasToolId::Eraser))?;
        fixture.point([2.0, 7.0], NONE)?;
        fixture.button(true)?;
        fixture.point([60.0, 7.0], NONE)?;
        fixture.settle_inputs()?;
        assert!(fixture.session.document.tools.staging_gesture(), "the stroke is still staged");
        assert!(!fixture.session.editing.in_flight(), "the shape waits for the stroke");
        assert_eq!(fixture.snapshot(image.document)?.revision(), painted);
        fixture.button(false)?;
        fixture.settle_inputs()?;
        let after = fixture.snapshot(image.document)?;
        let mask = coverage(&after, first)?;
        assert_eq!(
            [mask.value(25, 7), mask.value(10, 7), mask.value(45, 7)],
            [FULL_COVERAGE, 0, 0],
            "the shape lands over the erased stroke"
        );
        assert_eq!(after.revision(), painted + 2, "the stroke, then the shape");
        fixture.edit(CanvasEditCommand::History(CanvasHistoryAction::Undo))?;
        let erased = coverage(&fixture.snapshot(image.document)?, first)?;
        assert_eq!([erased.value(25, 7), erased.value(10, 7)], [0, 0], "the shape is one step");
        fixture.edit(CanvasEditCommand::History(CanvasHistoryAction::Undo))?;
        let restored = coverage(&fixture.snapshot(image.document)?, first)?;
        assert_eq!([restored.value(25, 7), restored.value(10, 7)], [FULL_COVERAGE; 2]);
    }
    fixture.finish().map(drop)
}
