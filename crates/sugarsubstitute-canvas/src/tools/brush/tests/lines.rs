// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Space commits a stroke, and Shift with a mouse press paints CuteCanvas's straight lines.
use super::*;

const SHIFT: ToolModifiers = ToolModifiers { shift: true, ctrl: false, alt: false };

#[test]
fn space_commits_the_stroke_before_temporary_navigation() -> harness::TestResult {
    run(|memory| {
        let mut canvas = Canvas::new(memory, 0)?;
        canvas.mouse([20.0, 20.0], NONE)?;
        canvas.button(true, NONE)?;
        canvas.mouse([60.0, 20.0], NONE)?;
        canvas.tools.temporary_navigation(true, Some(&mut canvas.document), NOW)?;
        assert_eq!(canvas.document.snapshot().revision(), 3, "the stroke is published");
        assert!(canvas.tools.preview_snapshot().is_none() && !canvas.tools.captured());
        assert_eq!(canvas.tools.effective(), ToolId::PAN_ZOOM);
        assert_eq!(mask(&canvas.document.snapshot())?.value(40, 20), FULL_COVERAGE);
        canvas.tools.temporary_navigation(false, Some(&mut canvas.document), NOW)?;
        canvas.mouse([60.0, 120.0], NONE)?;
        canvas.button(false, NONE)?;
        assert_eq!(canvas.document.snapshot().revision(), 3, "the stroke does not resume");
        assert_eq!(mask(&canvas.document.snapshot())?.value(60, 100), 0);
        assert!(canvas.document.undo()?);
        assert_eq!(mask(&canvas.document.snapshot())?.bounds(), None);
        Ok(())
    })
}

#[test]
fn shift_click_paints_a_straight_line_from_the_last_stroke_as_one_step() -> harness::TestResult {
    run(|memory| {
        let mut canvas = Canvas::new(memory, 0)?;
        canvas.stroke(&[[20.0, 20.0]], SHIFT)?;
        let first = mask(&canvas.document.snapshot())?;
        assert_eq!(first.bounds(), Some(Region::new(10, 10, 20, 20)?), "no line without a start");
        canvas.mouse([120.0, 120.0], SHIFT)?;
        canvas.button(true, SHIFT)?;
        assert!(!canvas.tools.captured() && canvas.tools.preview_snapshot().is_none());
        assert_eq!(canvas.document.snapshot().revision(), 4, "the line is published at once");
        let line = mask(&canvas.document.snapshot())?;
        for point in 20..=120 {
            assert_eq!(line.value(point, point), FULL_COVERAGE, "line pixel {point}");
        }
        canvas.button(false, SHIFT)?;
        assert_eq!(canvas.document.snapshot().revision(), 4);
        assert!(canvas.document.undo()?);
        let undone = mask(&canvas.document.snapshot())?;
        assert_eq!([undone.value(70, 70), undone.value(20, 20)], [0, FULL_COVERAGE]);
        Ok(())
    })
}

#[test]
fn shift_mouse_drags_and_shifted_pen_presses_draw_no_line() -> harness::TestResult {
    run(|memory| {
        let mut canvas = Canvas::new(memory, 0)?;
        canvas.mouse([200.0, 20.0], NONE)?;
        canvas.button(true, NONE)?;
        canvas.mouse([200.0, 120.0], SHIFT)?;
        canvas.button(false, SHIFT)?;
        let dragged = mask(&canvas.document.snapshot())?;
        assert_eq!([dragged.value(200, 20), dragged.value(200, 70)], [FULL_COVERAGE, 0]);

        let pen = PointerSample { kind: PointerKind::Pen, ..PointerSample::mouse([0.0; 2], SHIFT) };
        canvas.sample(pen, [120.0, 200.0])?;
        canvas.button(true, SHIFT)?;
        canvas.button(false, SHIFT)?;
        let pressed = mask(&canvas.document.snapshot())?;
        assert_eq!([pressed.value(120, 200), pressed.value(160, 110)], [FULL_COVERAGE, 0]);
        Ok(())
    })
}
