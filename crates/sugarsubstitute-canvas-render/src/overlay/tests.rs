// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Overlay pixels: dash colours, camera projection, clipping, animation and painting.
use super::*;
use crate::tests::harness;
use std::sync::Arc;
use sugarsubstitute_canvas::{CoverageContours, MarchingAnts};
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
fn ants_follow_the_camera_offset_and_animation_phase() -> harness::TestResult {
    run(|context| {
        let document = Extent::new(32, 32)?;
        let square = Path::rectangle([4.0, 4.0, 12.0, 12.0])?.rasterize(document, context)?;
        let contours = Arc::new(CoverageContours::trace(&square, context)?);
        let mut view = Viewport::new(Extent::new(64, 64)?)?;
        view.reframe([0.0, 0.0], 2.0)?;
        let ants = |phase: u64, offset: [i64; 2]| PresentationOverlay {
            selection: Some(Arc::clone(&contours)),
            selection_offset: offset,
            outline: None,
            ants: MarchingAnts::at(MarchingAnts::STEP * phase as u32, false),
            document,
        };
        let still = OverlayPixels::rasterize(&ants(0, [0, 0]), view, context)?;
        let positions = |overlay: &OverlayPixels| {
            let mut found: Vec<(u32, u32)> =
                overlay.pixels().iter().map(|pixel| (pixel.x, pixel.y)).collect();
            found.sort_unstable();
            found
        };
        // The outline sits on the square's device-space edge, 8 to 24.
        let drawn = positions(&still);
        assert!(drawn.iter().all(|(x, y)| (7..=24).contains(x) && (7..=24).contains(y)));
        assert!(drawn.contains(&(16, 8)) && drawn.contains(&(8, 16)));
        let light = still.pixels().iter().filter(|pixel| pixel.color == ANT_LIGHT).count();
        let dark = still.pixels().iter().filter(|pixel| pixel.color == ANT_DARK).count();
        assert_eq!(light + dark, still.pixels().len());
        assert!(light.abs_diff(dark) <= 8, "{light} light, {dark} dark");
        // Each step shifts the dashes by one pixel along the outline, not the outline itself.
        let stepped = OverlayPixels::rasterize(&ants(1, [0, 0]), view, context)?;
        assert_eq!(positions(&stepped), drawn);
        let changed = still
            .pixels()
            .iter()
            .zip(stepped.pixels())
            .filter(|(left, right)| left.color != right.color)
            .count();
        assert!(changed * 4 >= still.pixels().len() / 2, "{changed}");
        let cycled = OverlayPixels::rasterize(&ants(8, [0, 0]), view, context)?;
        assert_eq!(cycled.pixels(), still.pixels());
        // A dragged outline moves by whole document pixels through the camera.
        let moved = OverlayPixels::rasterize(&ants(0, [3, -2]), view, context)?;
        let shifted: Vec<(u32, u32)> = drawn.iter().map(|(x, y)| (x + 6, y - 4)).collect();
        assert_eq!(positions(&moved), shifted);
        // Ants entirely outside the surface draw nothing.
        view.reframe([100.0, 100.0], 2.0)?;
        assert!(OverlayPixels::rasterize(&ants(0, [0, 0]), view, context)?.pixels().is_empty());
        Ok(())
    })
}
