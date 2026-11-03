// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Brush and Eraser strokes through the tool manager: live staging, one undo step, cancel,
//! erasing, pressure, selection clipping, wheel sizing and cursor feedback.
use super::*;
use crate::{Document, MaskAppearance, Viewport, tests::harness};
use sugarsubstitute_contracts::execution::MemoryLease;
use sugarsubstitute_image_ops::{Coverage, Extent, FULL_COVERAGE, ImageError, Region};

const BRUSH: u32 = 2;
const ERASER: u32 = 3;
const MASK: u64 = 7;
const NOW: Duration = Duration::ZERO;

fn run(
    test: impl FnOnce(&MemoryLease) -> Result<(), CanvasError> + Send + 'static,
) -> harness::TestResult {
    harness::run(32 * 1024 * 1024, |context| {
        test(context.memory()).map_err(|cause| ImageError::Source(Box::new(cause)))
    })
}

struct Canvas {
    tools: ToolManager,
    document: Document,
}

impl Canvas {
    /// A 256 pixel document fitted 1:1 into a 256 pixel viewport, with one mask filled to
    /// `value` and the Brush selected.
    fn new(memory: &MemoryLease, value: u16) -> Result<Self, CanvasError> {
        let extent = Extent::new(256, 256)?;
        let mut navigation = Navigation::new(Viewport::new(extent)?);
        navigation.set_content(Some(extent), NOW)?;
        let mut tools = ToolManager::new(navigation);
        let options = BrushOptions::new(20, 1.0, 1.0)?;
        tools.register(ToolId::new(BRUSH)?, BrushTool::new(BrushMode::Paint, options), memory)?;
        tools.register(ToolId::new(ERASER)?, BrushTool::new(BrushMode::Erase, options), memory)?;
        let mut document = Document::new(1, extent, memory)?;
        let content = LayerContent::Coverage {
            coverage: Coverage::new(extent, value, memory)?,
            appearance: MaskAppearance::new([255, 0, 0], 0.5)?,
        };
        let layer = Layer::new(LayerId::new(MASK)?, content)?;
        document.apply(DocumentEdit::Insert { index: 0, layer })?;
        tools.set_target(Some(LayerId::new(MASK)?));
        tools.select(ToolId::new(BRUSH)?, Some(&document), NOW)?;
        Ok(Self { tools, document })
    }

    fn sample(&mut self, sample: PointerSample, document: [f64; 2]) -> Result<(), CanvasError> {
        let device = self.tools.navigation().viewport().device_at(document)?;
        self.tools.input(ToolInput::Pointer(sample.at(device)), Some(&mut self.document), NOW)
    }

    fn mouse(&mut self, document: [f64; 2], modifiers: ToolModifiers) -> Result<(), CanvasError> {
        self.sample(PointerSample::mouse([0.0; 2], modifiers), document)
    }

    fn button(&mut self, pressed: bool, modifiers: ToolModifiers) -> Result<(), CanvasError> {
        let input = ToolInput::Button { button: ToolButton::Primary, pressed, modifiers };
        self.tools.input(input, Some(&mut self.document), NOW)
    }

    fn stroke(&mut self, points: &[[f64; 2]], modifiers: ToolModifiers) -> Result<(), CanvasError> {
        self.mouse(points[0], modifiers)?;
        self.button(true, modifiers)?;
        for point in &points[1..] {
            self.mouse(*point, modifiers)?;
        }
        self.button(false, modifiers)
    }

    fn wheel(&mut self, steps: f64) -> Result<(), CanvasError> {
        let input = ToolInput::Wheel { steps, phase: ScrollPhase::Unphased };
        self.tools.input(input, Some(&mut self.document), NOW)
    }

    fn cursor(&self) -> CursorIntent {
        self.tools.cursor(Some(&self.document))
    }
}

/// The mask's coverage in a snapshot.
fn mask(snapshot: &DocumentSnapshot) -> Result<Coverage, CanvasError> {
    match snapshot.layers().first().map(Layer::content) {
        Some(LayerContent::Coverage { coverage, .. }) => Ok(coverage.clone()),
        _ => Err(CanvasError::Invalid("The test mask is missing")),
    }
}

fn brush_size(canvas: &Canvas, id: u32) -> Result<u32, CanvasError> {
    match canvas.tools.options(ToolId::new(id)?) {
        Some(ToolOptions::Brush(options)) => Ok(options.size()),
        None => Err(CanvasError::Invalid("The brush reports no options")),
    }
}

const NONE: ToolModifiers = ToolModifiers::NONE;
const ALT: ToolModifiers = ToolModifiers { shift: false, ctrl: false, alt: true };

#[test]
fn a_stroke_previews_live_and_commits_as_one_undo_step() -> harness::TestResult {
    run(|memory| {
        let mut canvas = Canvas::new(memory, 0)?;
        canvas.mouse([20.0, 20.0], NONE)?;
        canvas.button(true, NONE)?;
        assert!(canvas.tools.captured());
        let first = canvas.tools.preview_snapshot().ok_or(CanvasError::Invalid("No preview"))?;
        assert_eq!(mask(&first)?.value(20, 20), FULL_COVERAGE);
        canvas.mouse([120.0, 20.0], NONE)?;
        let second = canvas.tools.preview_snapshot().ok_or(CanvasError::Invalid("No preview"))?;
        assert!(second.stage() > first.stage(), "each sample stages a distinct preview");
        assert_eq!(mask(&second)?.value(70, 20), FULL_COVERAGE, "coalesced samples interpolate");
        assert_eq!(canvas.document.snapshot().revision(), 2, "nothing published mid-stroke");
        canvas.button(false, NONE)?;
        assert!(!canvas.tools.captured());
        assert!(canvas.tools.preview_snapshot().is_none());
        let published = mask(&canvas.document.snapshot())?;
        for x in 11..129 {
            assert_eq!(published.value(x, 20), FULL_COVERAGE, "line pixel {x}");
        }
        assert_eq!(published.value(20, 31), 0);
        assert_eq!(published.bounds(), Some(Region::new(10, 10, 120, 20)?));
        assert!(canvas.document.undo()?);
        assert_eq!(mask(&canvas.document.snapshot())?.bounds(), None);
        assert_eq!(canvas.document.snapshot().layers().len(), 1, "the whole stroke was one step");
        assert!(canvas.document.redo()?);
        assert_eq!(
            mask(&canvas.document.snapshot())?.bounds(),
            Some(Region::new(10, 10, 120, 20)?)
        );
        Ok(())
    })
}

#[test]
fn cancelling_discards_the_stroke_and_a_missing_or_locked_target_paints_nothing()
-> harness::TestResult {
    run(|memory| {
        let mut canvas = Canvas::new(memory, 0)?;
        canvas.mouse([20.0, 20.0], NONE)?;
        canvas.button(true, NONE)?;
        canvas.mouse([60.0, 60.0], NONE)?;
        canvas.tools.cancel(ToolCancelReason::FocusLost, Some(&canvas.document), NOW)?;
        assert!(canvas.tools.preview_snapshot().is_none() && !canvas.tools.captured());
        canvas.mouse([80.0, 80.0], NONE)?;
        canvas.button(false, NONE)?;
        assert_eq!(canvas.document.snapshot().revision(), 2);

        canvas.tools.set_target(None);
        canvas.stroke(&[[20.0, 20.0], [40.0, 40.0]], NONE)?;
        assert_eq!(canvas.document.snapshot().revision(), 2);
        assert_eq!(canvas.cursor(), CursorIntent::Arrow);

        let locked = canvas.document.snapshot().layers()[0].clone().with_locked(true);
        canvas.document.apply(DocumentEdit::Replace(locked))?;
        canvas.tools.set_target(Some(LayerId::new(MASK)?));
        canvas.stroke(&[[20.0, 20.0], [40.0, 40.0]], NONE)?;
        assert_eq!(canvas.document.snapshot().revision(), 3);
        assert_eq!(canvas.cursor(), CursorIntent::Arrow);
        Ok(())
    })
}

#[test]
fn alt_and_the_pen_eraser_turn_the_brush_into_an_eraser_and_the_eraser_always_erases()
-> harness::TestResult {
    run(|memory| {
        let mut canvas = Canvas::new(memory, FULL_COVERAGE)?;
        canvas.stroke(&[[30.0, 30.0]], ALT)?;
        assert_eq!(mask(&canvas.document.snapshot())?.value(30, 30), 0);

        let pen = PointerSample {
            kind: PointerKind::Pen,
            eraser: true,
            ..PointerSample::mouse([0.0; 2], NONE)
        };
        canvas.sample(pen, [90.0, 30.0])?;
        canvas.button(true, NONE)?;
        canvas.button(false, NONE)?;
        let erased = mask(&canvas.document.snapshot())?;
        assert_eq!(erased.value(90, 30), 0);

        canvas.tools.select(ToolId::new(ERASER)?, Some(&canvas.document), NOW)?;
        canvas.stroke(&[[150.0, 30.0]], NONE)?;
        assert_eq!(mask(&canvas.document.snapshot())?.value(150, 30), 0);
        assert_eq!(mask(&canvas.document.snapshot())?.value(200, 200), FULL_COVERAGE);
        Ok(())
    })
}

#[test]
fn pen_pressure_narrows_dabs_and_a_selection_clips_strokes() -> harness::TestResult {
    run(|memory| {
        let mut canvas = Canvas::new(memory, 0)?;
        canvas.tools.configure(
            ToolId::new(BRUSH)?,
            ToolOptions::Brush(BrushOptions::new(100, 1.0, 1.0)?),
        )?;
        let light = PointerSample {
            kind: PointerKind::Pen,
            pressure: Pressure::from_range(0, 1024),
            ..PointerSample::mouse([0.0; 2], NONE)
        };
        canvas.sample(light, [128.0, 128.0])?;
        canvas.button(true, NONE)?;
        canvas.button(false, NONE)?;
        let narrow = mask(&canvas.document.snapshot())?;
        assert_eq!(narrow.value(134, 128), FULL_COVERAGE, "15 px dab reaches 7 px out");
        assert_eq!(narrow.value(137, 128), 0);

        let mut selection = Coverage::new(Extent::new(256, 256)?, 0, memory)?;
        selection.update(Region::new(0, 0, 50, 256)?, memory, |_, _, _| FULL_COVERAGE)?;
        canvas.document.apply(DocumentEdit::Select(Some(selection)))?;
        canvas.stroke(&[[20.0, 220.0], [90.0, 220.0]], NONE)?;
        let clipped = mask(&canvas.document.snapshot())?;
        assert_eq!(clipped.value(45, 220), FULL_COVERAGE);
        assert_eq!(clipped.value(55, 220), 0, "outside the selection");
        Ok(())
    })
}

#[test]
fn the_wheel_resizes_by_five_pixels_a_notch_within_the_supported_range() -> harness::TestResult {
    run(|memory| {
        let mut canvas = Canvas::new(memory, 0)?;
        canvas.wheel(1.0)?;
        assert_eq!(brush_size(&canvas, BRUSH)?, 25);
        canvas.wheel(-3.0)?;
        assert_eq!(brush_size(&canvas, BRUSH)?, 10);
        canvas.wheel(0.5)?;
        assert_eq!(brush_size(&canvas, BRUSH)?, 10, "half a notch waits for the rest");
        canvas.wheel(0.5)?;
        assert_eq!(brush_size(&canvas, BRUSH)?, 15);
        canvas.wheel(-100.0)?;
        assert_eq!(brush_size(&canvas, BRUSH)?, BrushOptions::MIN_SIZE);
        canvas.wheel(1000.0)?;
        assert_eq!(brush_size(&canvas, BRUSH)?, BrushOptions::MAX_SIZE);
        assert_eq!(brush_size(&canvas, ERASER)?, 20, "each tool keeps its own options");
        assert!(canvas.tools.options(ToolId::PAN_ZOOM).is_none());
        let options = ToolOptions::Brush(BrushOptions::DEFAULT);
        assert!(canvas.tools.configure(ToolId::PAN_ZOOM, options).is_err());
        assert!(canvas.tools.configure(ToolId::new(9)?, options).is_err());
        assert!(BrushOptions::new(0, 1.0, 1.0).is_err());
        assert!(BrushOptions::new(1001, 1.0, 1.0).is_err());
        assert!(BrushOptions::new(10, 1.5, 1.0).is_err());
        assert!(BrushOptions::new(10, 1.0, f64::NAN).is_err());
        Ok(())
    })
}

#[test]
fn the_cursor_is_the_on_screen_brush_ring_in_the_mask_colour() -> harness::TestResult {
    run(|memory| {
        let mut canvas = Canvas::new(memory, 0)?;
        let zoom = canvas.tools.navigation().viewport().zoom();
        let ring = |diameter: f64, erase| {
            CursorIntent::BrushOutline(BrushOutline::new(
                diameter.round() as u32,
                [255, 0, 0],
                erase,
            ))
        };
        assert_eq!(canvas.cursor(), ring(20.0 * zoom, false));
        canvas.mouse([10.0, 10.0], ALT)?;
        assert_eq!(canvas.cursor(), ring(20.0 * zoom, true), "Alt shows the erase mark");
        canvas.tools.select(ToolId::new(ERASER)?, Some(&canvas.document), NOW)?;
        canvas.mouse([10.0, 10.0], ALT)?;
        assert_eq!(canvas.cursor(), ring(20.0 * zoom, false), "the Eraser needs no mark");
        let huge = ToolOptions::Brush(BrushOptions::new(1000, 1.0, 1.0)?);
        canvas.tools.configure(ToolId::new(ERASER)?, huge)?;
        assert_eq!(canvas.cursor(), CursorIntent::Arrow, "rings larger than the view");
        Ok(())
    })
}
