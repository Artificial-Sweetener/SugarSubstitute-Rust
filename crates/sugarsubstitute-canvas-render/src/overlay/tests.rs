// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Overlay pixels: dash colours, camera projection, clipping, animation and painting.
use super::*;
use crate::tests::harness;
use std::sync::Arc;
use sugarsubstitute_canvas::{CoverageBoundary, MarchingAnts};
use sugarsubstitute_image_ops::Path;

const GREY: u32 = 0xff80_8080;

fn word([red, green, blue]: [u8; 3]) -> u32 {
    u32::from_be_bytes([255, red, green, blue])
}

fn painted(overlay: &OverlayPixels) -> Result<Vec<u32>, RenderError> {
    let extent = overlay.extent();
    let mut pixels = vec![GREY; extent.pixels() as usize];
    overlay.paint(extent, &mut pixels)?;
    Ok(pixels)
}

fn run(
    test: impl FnOnce(&JobContext) -> Result<(), RenderError> + Send + 'static,
) -> harness::TestResult {
    harness::run(16 * 1024 * 1024, |context| {
        test(context).map_err(|cause| ImageError::Source(Box::new(cause)))
    })
}

fn outline(document: Extent, shape: OverlayOutline) -> PresentationOverlay {
    PresentationOverlay {
        selection: None,
        selection_offset: [0, 0],
        outline: Some(shape),
        guides: None,
        snaps: Arc::from([]),
        ants: MarchingAnts::default(),
        document,
    }
}

#[test]
fn drawn_outlines_dash_eight_white_four_black_and_clip_to_the_document() -> harness::TestResult {
    run(|context| {
        let surface = Extent::new(40, 30)?;
        let view = Viewport::new(surface)?;
        let document = Extent::new(30, 20)?;
        let overlay = OverlayPixels::rasterize(
            &outline(document, OverlayOutline::Rectangle([10.0, 10.0, 22.0, 15.0])),
            view,
            context,
        )?;
        let pixels = painted(&overlay)?;
        let at = |x: usize, y: usize| pixels[y * 40 + x];
        let top: Vec<u32> = (10..22).map(|x| at(x, 10)).collect();
        // The closing left edge ends on the first corner pixel and draws over it.
        assert_eq!(top[1..8], [word(WHITE); 7]);
        assert_eq!(top[8..], [word(BLACK); 4]);
        // The dash continues around the corner: the right edge starts 12 pixels in.
        assert_eq!(at(22, 10), word(WHITE));
        assert_eq!(at(9, 9), GREY);
        // An outline beyond the document is clipped to it.
        let clipped = OverlayPixels::rasterize(
            &outline(document, OverlayOutline::Ellipse([20.0, 5.0, 50.0, 25.0])),
            view,
            context,
        )?;
        assert!(!clipped.pixels().is_empty());
        assert!(clipped.pixels().iter().all(|pixel| pixel.x < 30 && pixel.y < 20));
        let mut wrong = vec![GREY; 4];
        assert!(overlay.paint(Extent::new(2, 2)?, &mut wrong).is_err());
        Ok(())
    })
}

#[test]
fn a_previewed_line_is_one_open_dashed_segment_clipped_to_the_document() -> harness::TestResult {
    run(|context| {
        let surface = Extent::new(40, 30)?;
        let document = Extent::new(30, 20)?;
        let line = outline(document, OverlayOutline::Line([[2.0, 5.5], [36.0, 5.5]]));
        let overlay = OverlayPixels::rasterize(&line, Viewport::new(surface)?, context)?;
        let pixels = painted(&overlay)?;
        let row: Vec<u32> = (2..30).map(|x| pixels[5 * 40 + x]).collect();
        assert_eq!(row[..8], [word(WHITE); 8]);
        assert_eq!(row[8..12], [word(BLACK); 4]);
        assert_eq!(row[12..20], [word(WHITE); 8]);
        // Nothing closes the segment, and the part beyond the document is clipped.
        assert!(overlay.pixels().iter().all(|pixel| pixel.y == 5 && pixel.x < 30));
        assert_eq!(overlay.pixels().len(), 28);
        Ok(())
    })
}

#[test]
fn ants_dash_each_boundary_run_through_the_camera_and_phase() -> harness::TestResult {
    run(|context| {
        let document = Extent::new(32, 32)?;
        let square = Path::rectangle([4.0, 4.0, 12.0, 12.0])?.rasterize(document, context)?;
        let boundary = Arc::new(CoverageBoundary::trace(&square, context)?);
        let mut view = Viewport::new(Extent::new(64, 64)?)?;
        view.reframe([0.0, 0.0], 2.0)?;
        let ants = |phase: u32, offset: [i64; 2]| PresentationOverlay {
            selection: Some(Arc::clone(&boundary)),
            selection_offset: offset,
            outline: None,
            guides: None,
            snaps: Arc::from([]),
            ants: MarchingAnts::at(MarchingAnts::STEP * phase, false),
            document,
        };
        let draw =
            |overlay: &PresentationOverlay, view: Viewport| -> Result<Vec<u32>, RenderError> {
                painted(&OverlayPixels::rasterize(overlay, view, context)?)
            };
        let (light, dark) = (word(ANT_LIGHT), word(ANT_DARK));
        let still = draw(&ants(0, [0, 0]), view)?;
        let at = |pixels: &[u32], x: usize, y: usize| pixels[y * 64 + x];
        // The top run spans device columns 8 to 23 on row 8, dashed 4 light, 4 dark.
        let top: Vec<u32> = (8..24).map(|x| at(&still, x, 8)).collect();
        assert_eq!(top, [[light; 4], [dark; 4], [light; 4], [dark; 4]].concat());
        // Every run starts its own dash at its top or left end.
        assert_eq!([at(&still, 8, 12), at(&still, 24, 8), at(&still, 8, 24)], [dark, light, light]);
        assert_eq!([at(&still, 7, 8), at(&still, 9, 9), at(&still, 25, 8)], [GREY; 3]);
        // A step shifts the dashes one pixel back along each run.
        let stepped = draw(&ants(1, [0, 0]), view)?;
        assert_eq!([at(&stepped, 10, 8), at(&stepped, 11, 8)], [light, dark]);
        let drawn = |pixels: &[u32]| pixels.iter().map(|pixel| *pixel != GREY).collect::<Vec<_>>();
        assert_eq!(drawn(&stepped), drawn(&still));
        assert_eq!(draw(&ants(8, [0, 0]), view)?, still);
        // A dragged outline moves by whole document pixels through the camera.
        let moved = draw(&ants(0, [3, -2]), view)?;
        assert_eq!(at(&moved, 14, 4), at(&still, 8, 8));
        assert_eq!(at(&moved, 14 + 4, 4), dark);
        // Ants entirely outside the surface draw nothing.
        view.reframe([100.0, 100.0], 2.0)?;
        assert!(OverlayPixels::rasterize(&ants(0, [0, 0]), view, context)?.pixels().is_empty());
        Ok(())
    })
}

#[test]
fn magnified_strokes_sample_only_the_visible_surface() -> harness::TestResult {
    run(|context| {
        let document = Extent::new(16384, 16384)?;
        let everything =
            sugarsubstitute_image_ops::Coverage::new(document, 65535, context.memory())?;
        let boundary = Arc::new(CoverageBoundary::trace(&everything, context)?);
        let mut view = Viewport::new(Extent::new(32, 32)?)?;
        view.reframe([-10.0 / 4096.0, 100.0], 4096.0)?;
        let overlay = PresentationOverlay {
            selection: Some(boundary),
            selection_offset: [0, 0],
            outline: None,
            guides: None,
            snaps: Arc::from([]),
            ants: MarchingAnts::default(),
            document,
        };
        let pixels = OverlayPixels::rasterize(&overlay, view, context)?;
        // Only the document's left edge crosses the view, one column of 32 rows.
        assert_eq!(pixels.pixels().len(), 32);
        assert_eq!(OverlayPixels::maximum_pixels(&overlay, view)?, PIXEL_CHUNK);
        assert!(pixels.pixels().iter().all(|pixel| pixel.x == 10));
        Ok(())
    })
}

#[test]
fn overlay_admission_bounds_actual_capacity_for_every_primitive_camera_and_clip()
-> harness::TestResult {
    run(|context| {
        let document = Extent::new(181, 107)?;
        let selection = Path::rectangle([3.0, 7.0, 143.0, 81.0])?.rasterize(document, context)?;
        let boundary = Arc::new(CoverageBoundary::trace(&selection, context)?);
        let outlines = [
            OverlayOutline::Line([[-20.0, 30.0], [190.0, 80.0]]),
            OverlayOutline::Rectangle([-10.0, 5.0, 160.0, 103.0]),
            OverlayOutline::Ellipse([-20.0, -30.0, 198.0, 100.0]),
            OverlayOutline::Polygon(Arc::from([
                [7.0, 3.0],
                [93.0, 6.0],
                [170.0, 87.0],
                [12.0, 93.0],
            ])),
        ];
        for (index, shape) in outlines.into_iter().enumerate() {
            for zoom in [0.05, 1.0, 32.0] {
                let mut view = Viewport::new(Extent::new(333, 179)?)?;
                view.reframe([-15.25, -9.75], zoom)?;
                let guides = if index.is_multiple_of(2) {
                    OverlayGuides::Frame([[10.0, 10.0], [153.0, 12.0], [143.0, 97.0], [11.0, 87.0]])
                } else {
                    OverlayGuides::Seams(Arc::from([sugarsubstitute_canvas::OverlaySeam {
                        start: [50.0, -20.0],
                        end: [60.0, 140.0],
                        focused: true,
                        handle: Some(SeamHandle::Start),
                        enabled: [true, false, true],
                    }]))
                };
                let overlay = PresentationOverlay {
                    selection: Some(boundary.clone()),
                    selection_offset: [-3, 5],
                    outline: Some(shape.clone()),
                    guides: Some(guides),
                    snaps: Arc::from([sugarsubstitute_canvas::SnapGuide {
                        start: [-90.0, 63.0],
                        end: [220.0, 63.0],
                    }]),
                    ants: MarchingAnts::default(),
                    document,
                };
                let maximum = OverlayPixels::maximum_pixels(&overlay, view)?;
                let actual = OverlayPixels::rasterize(&overlay, view, context)?;
                assert!(
                    maximum >= actual.pixels.capacity(),
                    "Overlay admission undercounts primitive {index} at {zoom}"
                );
                assert!(maximum.is_multiple_of(PIXEL_CHUNK));
            }
        }
        Ok(())
    })
}

#[test]
fn overlay_admission_allocates_no_pixel_inventory_and_empty_geometry_reserves_nothing()
-> Result<(), RenderError> {
    let extent = Extent::new(333, 179)?;
    let view = Viewport::new(extent)?;
    let mut overlay = outline(extent, OverlayOutline::Ellipse([13.25, 7.75, 290.5, 160.25]));
    let mut counter =
        Raster { extent, pixels: Vec::new(), memory: Vec::new(), context: None, maximum: Some(0) };
    trace(&overlay, view, &mut counter)?;
    assert!(counter.maximum.is_some_and(|count| count > 100));
    assert_eq!(counter.pixels.capacity(), 0);
    assert_eq!(counter.memory.capacity(), 0);
    overlay.outline = None;
    assert_eq!(OverlayPixels::maximum_pixels(&overlay, view)?, 0);
    overlay.outline = Some(OverlayOutline::Line([[400.0, 300.0], [500.0, 400.0]]));
    assert_eq!(OverlayPixels::maximum_pixels(&overlay, view)?, 0);
    Ok(())
}

#[test]
fn guides_draw_a_solid_frame_with_handles_and_heavier_focused_seams() -> harness::TestResult {
    run(|context| {
        let surface = Extent::new(60, 50)?;
        let view = Viewport::new(surface)?;
        let guides = |guides| PresentationOverlay {
            selection: None,
            selection_offset: [0, 0],
            outline: None,
            guides: Some(guides),
            snaps: Arc::from([]),
            ants: MarchingAnts::default(),
            document: surface,
        };
        let corners = [[10.0, 10.0], [40.0, 10.0], [40.0, 30.0], [10.0, 30.0]];
        let frame =
            OverlayPixels::rasterize(&guides(OverlayGuides::Frame(corners)), view, context)?;
        let pixels = painted(&frame)?;
        let at = |pixels: &[u32], x: usize, y: usize| pixels[y * 60 + x];
        // Solid guide edges between handles, a filled handle at every corner and midpoint.
        assert_eq!(at(&pixels, 18, 10), word(GUIDE));
        assert_eq!(at(&pixels, 33, 10), word(GUIDE));
        for [x, y] in [[10, 10], [25, 10], [40, 20], [25, 30]] {
            assert_eq!(at(&pixels, x, y), word(GUIDE_HANDLE), "{x},{y}");
        }
        assert_eq!(at(&pixels, 25, 20), GREY);
        let seam = |focused| {
            let seams = Arc::from([sugarsubstitute_canvas::OverlaySeam {
                start: [20.0, 5.0],
                end: [20.0, 45.0],
                focused,
                handle: None,
                enabled: [false, true, true],
            }]);
            OverlayPixels::rasterize(&guides(OverlayGuides::Seams(seams)), view, context)
        };
        let (plain, focused) = (painted(&seam(false)?)?, painted(&seam(true)?)?);
        assert_eq!([at(&plain, 20, 10), at(&plain, 19, 10)], [word(GUIDE), GREY]);
        assert_eq!([at(&focused, 20, 10), at(&focused, 19, 10)], [word(GUIDE); 2]);
        assert_eq!(at(&plain, 20, 25), word(GUIDE_HANDLE));
        // The start handle cannot be dragged: grey, crossed in the handle colour.
        assert_eq!(
            [at(&plain, 18, 5), at(&plain, 20, 5)],
            [word(GUIDE_DISABLED), word(GUIDE_HANDLE)]
        );
        assert_eq!(at(&plain, 20, 45), word(GUIDE_HANDLE));
        Ok(())
    })
}

#[test]
fn smart_guides_draw_solid_over_frames_through_the_camera() -> harness::TestResult {
    run(|context| {
        let surface = Extent::new(60, 50)?;
        let mut view = Viewport::new(surface)?;
        view.reframe([0.0, 0.0], 2.0)?;
        let corners = [[5.0, 5.0], [20.0, 5.0], [20.0, 20.0], [5.0, 20.0]];
        let guide = sugarsubstitute_canvas::SnapGuide { start: [10.0, 0.0], end: [10.0, 25.0] };
        let overlay = PresentationOverlay {
            selection: None,
            selection_offset: [0, 0],
            outline: None,
            guides: Some(OverlayGuides::Frame(corners)),
            snaps: Arc::from([guide]),
            ants: MarchingAnts::default(),
            document: surface,
        };
        let pixels = painted(&OverlayPixels::rasterize(&overlay, view, context)?)?;
        let at = |x: usize, y: usize| pixels[y * 60 + x];
        // The vertical guide at document x 10 is device column 20, drawn over the frame's top.
        assert_eq!([at(20, 0), at(20, 10), at(20, 30), at(20, 49)], [word(SNAP_GUIDE); 4]);
        assert_eq!([at(19, 30), at(21, 30)], [GREY; 2]);
        assert_eq!(at(30, 10), word(GUIDE));
        Ok(())
    })
}
