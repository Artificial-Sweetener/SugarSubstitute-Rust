// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Map a retained physical viewport into another camera without changing document content.
use crate::{CanvasError, Viewport};

/// Source-to-output display transform for disposable camera-motion pixels.
/// Sampling uses physical pixel centers and returns no source outside retained coverage.
/// This geometry grants no document identity or exact-pixel repair dependency.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewProjection {
    source: Viewport,
    target: Viewport,
    scale: f64,
    offset: [f64; 2],
}
impl ViewProjection {
    /// Build a phase-preserving transform from canonical source pixels to the target camera.
    /// # Errors
    /// Rejects transforms whose physical scale or translation cannot be represented finitely.
    pub fn new(source: Viewport, target: Viewport) -> Result<Self, CanvasError> {
        let (scale, offset) = source.projection_to(target)?;
        Ok(Self { source, target, scale, offset })
    }
    /// Camera represented by the retained source image, including its integer sampling phase.
    pub const fn source(self) -> Viewport {
        self.source
    }
    /// Requested physical camera; projected pixels remain an approximation of this view.
    pub const fn target(self) -> Viewport {
        self.target
    }
    /// Physical output pixels per physical source pixel.
    pub const fn scale(self) -> f64 {
        self.scale
    }
    /// Physical output position of the source image's top-left corner.
    pub const fn offset(self) -> [f64; 2] {
        self.offset
    }
    /// Source column beneath an output pixel center, or uncovered/out-of-output coverage.
    pub fn column(self, output: u32) -> Option<u32> {
        self.sample(output, self.target.surface().width(), self.source.surface().width(), 0)
    }
    /// Source row beneath an output pixel center, or uncovered/out-of-output coverage.
    pub fn row(self, output: u32) -> Option<u32> {
        self.sample(output, self.target.surface().height(), self.source.surface().height(), 1)
    }
    fn sample(self, output: u32, bound: u32, source: u32, axis: usize) -> Option<u32> {
        if output >= bound {
            return None;
        }
        let position = (f64::from(output) + 0.5 - self.offset[axis]) / self.scale;
        (position >= 0.0 && position < f64::from(source)).then(|| position.floor() as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sugarsubstitute_image_ops::Extent;
    type TestResult = Result<(), Box<dyn std::error::Error>>;
    #[test]
    fn integer_pan_preserves_sampling_phase_at_distant_document_coordinates() -> TestResult {
        let mut source = Viewport::new(Extent::new(31, 19)?)?;
        source.reframe([1e12, -1e12], 0.75)?;
        source.pan_by([7.0, -3.0])?;
        let mut target = source;
        target.pan_by([13.0, -7.0])?;
        let projection = ViewProjection::new(source, target)?;
        assert_eq!(projection.scale(), 1.0);
        assert_eq!(projection.offset(), [13.0, -7.0]);
        for x in 0..31 {
            assert_eq!(projection.column(x), x.checked_sub(13));
        }
        for y in 0..19 {
            assert_eq!(projection.row(y), (y + 7 < 19).then_some(y + 7));
        }
        Ok(())
    }
    #[test]
    fn pixel_center_mapping_matches_document_coordinates_for_zoom_and_resize() -> TestResult {
        let mut source = Viewport::new(Extent::new(29, 23)?)?;
        source.zoom_at(0.75, [3.25, 8.5])?;
        source.pan_by([3.0, -5.0])?;
        let mut target = source;
        target.zoom_at(1.25, [17.25, 8.75])?;
        target.resize(Extent::new(37, 31)?)?;
        let projection = ViewProjection::new(source, target)?;
        for y in 0..31 {
            for x in 0..37 {
                let point = source
                    .device_at(target.document_at([f64::from(x) + 0.5, f64::from(y) + 0.5])?)?;
                let expected = |value: f64, bound: u32| {
                    (value >= 0.0 && value < f64::from(bound)).then(|| value.floor() as u32)
                };
                assert_eq!(projection.column(x), expected(point[0], 29));
                assert_eq!(projection.row(y), expected(point[1], 23));
            }
        }
        assert_eq!(projection.column(37), None);
        assert_eq!(projection.row(31), None);
        Ok(())
    }
    #[test]
    fn extreme_scale_and_disjoint_views_have_bounded_sampling_without_stretch_rectangles()
    -> TestResult {
        let source = Viewport::new(Extent::new(23, 17)?)?;
        let mut distant = source;
        distant.pan_by([1000.0, 1000.0])?;
        let empty = ViewProjection::new(source, distant)?;
        assert!((0..23).all(|x| empty.column(x).is_none()));
        assert!((0..17).all(|y| empty.row(y).is_none()));
        let mut tiny = source;
        tiny.reframe([0.0; 2], 1e-9)?;
        let large = ViewProjection::new(tiny, source)?;
        assert_eq!(large.column(22), Some(0));
        assert_eq!(large.row(16), Some(0));
        Ok(())
    }
}
