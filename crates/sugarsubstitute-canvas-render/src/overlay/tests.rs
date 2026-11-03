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
            ants: MarchingAnts::default(),
            document,
        };
        let pixels = OverlayPixels::rasterize(&overlay, view, context)?;
        // Only the document's left edge crosses the view, one column of 32 rows.
        assert_eq!(pixels.pixels().len(), 32);
        assert!(pixels.pixels().iter().all(|pixel| pixel.x == 10));
        Ok(())
    })
}
