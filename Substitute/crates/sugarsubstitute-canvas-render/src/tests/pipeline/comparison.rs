// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reveal a stretched comparison raster through a viewport-anchored wipe over the document.
use super::*;
use sugarsubstitute_canvas::{Comparison, WipeOrientation};

const PRIMARY: AssetId = AssetId([1; 32]);
const COMPARED: AssetId = AssetId([2; 32]);

/// Two constant-colored originals with their own extents.
struct Pair {
    primary: Extent,
    compared: Extent,
}
impl RasterSource for Pair {
    fn read(
        &mut self,
        asset: AssetId,
        region: Region,
        output: &mut [Pixel],
        _: &JobContext,
    ) -> Result<(), ImageError> {
        let (extent, color) = match asset {
            PRIMARY => (self.primary, Pixel::linear(1.0, 0.0, 0.0, 1.0)?),
            COMPARED => (self.compared, Pixel::linear(0.0, 0.0, 1.0, 1.0)?),
            _ => return Err(ImageError::Invalid("Unknown comparison asset")),
        };
        if region.intersection(extent.region()) != Some(region) {
            return Err(ImageError::Invalid("Comparison read outside its original"));
        }
        output.fill(color);
        Ok(())
    }
}

const RED: u32 = 0xffff_0000;
const BLUE: u32 = 0xff00_00ff;

fn render(
    comparison: Option<Comparison>,
    zoom: f64,
    context: &JobContext,
) -> Result<(Vec<u32>, Extent), RenderError> {
    let primary = Extent::new(64, 32)?;
    let snapshot = image_document(primary, context)?.snapshot();
    let mut view = Viewport::new(primary)?;
    view.zoom_at(zoom, [0.0; 2])?;
    let source = Pair { primary, compared: Extent::new(16, 24)? };
    let mut renderer = CpuRenderer::new(source, 64, context.memory())?;
    let update = renderer.prepare(
        RenderRequest {
            document: &snapshot,
            comparison,
            view,
            reconstruction: Reconstruction::Finished(Filter::Lanczos3),
            matte: black()?,
            target: stamp(1)?,
            base: None,
            damage: DamagePlan::full(view.surface()),
        },
        context,
    )?;
    let mut surface = CpuSurface::new(stamp(1)?.surface, view.surface(), context.memory())?;
    surface.apply(&update)?;
    Ok((surface.pixels().to_vec(), view.surface()))
}

fn compared(orientation: WipeOrientation, boundary: f64) -> Result<Comparison, RenderError> {
    Ok(Comparison::new(COMPARED, Extent::new(16, 24)?, orientation, boundary)?)
}

#[test]
fn a_vertical_wipe_reveals_the_stretched_comparison_left_of_the_boundary() -> harness::TestResult {
    execute(|context| {
        let (pixels, extent) =
            render(Some(compared(WipeOrientation::Vertical, 16.0)?), 1.0, context)?;
        for (index, &pixel) in pixels.iter().enumerate() {
            let x = index as u32 % extent.width();
            assert_eq!(pixel, if x < 16 { BLUE } else { RED }, "pixel {index}");
        }
        Ok(())
    })
}

#[test]
fn a_horizontal_wipe_reveals_the_comparison_above_the_boundary() -> harness::TestResult {
    execute(|context| {
        let (pixels, extent) =
            render(Some(compared(WipeOrientation::Horizontal, 16.0)?), 1.0, context)?;
        for (index, &pixel) in pixels.iter().enumerate() {
            let y = index as u32 / extent.width();
            assert_eq!(pixel, if y < 16 { BLUE } else { RED }, "pixel {index}");
        }
        Ok(())
    })
}

#[test]
fn the_document_space_boundary_follows_the_camera_into_device_pixels() -> harness::TestResult {
    execute(|context| {
        let (pixels, extent) =
            render(Some(compared(WipeOrientation::Vertical, 8.0)?), 2.0, context)?;
        for (index, &pixel) in pixels.iter().enumerate() {
            let x = index as u32 % extent.width();
            assert_eq!(pixel, if x < 16 { BLUE } else { RED }, "pixel {index}");
        }
        Ok(())
    })
}

#[test]
fn boundaries_outside_the_document_show_exactly_one_image() -> harness::TestResult {
    execute(|context| {
        let (closed, _) = render(Some(compared(WipeOrientation::Vertical, -3.0)?), 1.0, context)?;
        let (plain, _) = render(None, 1.0, context)?;
        assert!(closed.iter().all(|&pixel| pixel == RED));
        assert_eq!(closed, plain);
        let (open, _) = render(Some(compared(WipeOrientation::Horizontal, 400.0)?), 1.0, context)?;
        assert!(open.iter().all(|&pixel| pixel == BLUE));
        Ok(())
    })
}

#[test]
fn comparison_boundary_must_be_finite() -> Result<(), RenderError> {
    let extent = Extent::new(16, 24)?;
    for boundary in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(Comparison::new(COMPARED, extent, WipeOrientation::Vertical, boundary).is_err());
    }
    assert_eq!(compared(WipeOrientation::Horizontal, -1.0)?.boundary(), -1.0);
    Ok(())
}
