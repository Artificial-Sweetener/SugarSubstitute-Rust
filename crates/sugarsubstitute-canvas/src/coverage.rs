// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Plan a phase-preserving portable backing viewport independently of presentation handles.
use crate::{CanvasError, ViewProjection, Viewport, viewport::MAX_SURFACE_EDGE};
use sugarsubstitute_image_ops::Extent;

/// A retained canvas viewport with symmetric physical overscan around its construction target.
/// Requested margins are host policy; actual margins are bounded only by the canvas surface limit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BackingCoverage {
    viewport: Viewport,
    margin: [u32; 2],
}
impl BackingCoverage {
    /// Enlarge a target camera by bounded symmetric physical margins without recentering its phase.
    /// # Errors
    /// Preserves checked extent and integer-translation failures without returning partial geometry.
    pub fn new(target: Viewport, requested_margin: [u32; 2]) -> Result<Self, CanvasError> {
        let target_surface = target.surface();
        let margin = [
            requested_margin[0].min((MAX_SURFACE_EDGE - target_surface.width()) / 2),
            requested_margin[1].min((MAX_SURFACE_EDGE - target_surface.height()) / 2),
        ];
        let width = u64::from(target_surface.width()) + 2 * u64::from(margin[0]);
        let height = u64::from(target_surface.height()) + 2 * u64::from(margin[1]);
        let surface = Extent::new(
            u32::try_from(width)
                .map_err(|_| CanvasError::Invalid("Backing width exceeds canvas bounds"))?,
            u32::try_from(height)
                .map_err(|_| CanvasError::Invalid("Backing height exceeds canvas bounds"))?,
        )?;
        let mut viewport = target;
        viewport.expand_surface(surface, margin)?;
        Ok(Self { viewport, margin })
    }
    /// Canonical camera and physical dimensions represented by the retained backing pixels.
    pub const fn viewport(self) -> Viewport {
        self.viewport
    }
    /// Actual symmetric horizontal and vertical physical margins after canvas-domain clamping.
    pub const fn margin(self) -> [u32; 2] {
        self.margin
    }
    /// Build a disposable crop when retained coverage contains all target-visible document area.
    /// Target pixels outside the document may remain uncovered and receive presentation matte.
    /// The returned transform grants no exact-frame identity and may represent a different scale.
    /// # Errors
    /// Preserves unrepresentable camera projection or document-footprint failures.
    pub fn crop(
        self,
        target: Viewport,
        document: Extent,
    ) -> Result<Option<ViewProjection>, CanvasError> {
        let projection = ViewProjection::new(self.viewport, target)?;
        let document_start = target.device_at([0.0; 2])?;
        let document_end =
            target.device_at([f64::from(document.width()), f64::from(document.height())])?;
        let target_surface = target.surface();
        let visible = [
            intersection(document_start[0], document_end[0], f64::from(target_surface.width())),
            intersection(document_start[1], document_end[1], f64::from(target_surface.height())),
        ];
        if visible.iter().any(Option::is_none) {
            return Ok(Some(projection));
        }
        let source_surface = self.viewport.surface();
        let covered = [
            (
                projection.offset()[0],
                projection.offset()[0] + f64::from(source_surface.width()) * projection.scale(),
            ),
            (
                projection.offset()[1],
                projection.offset()[1] + f64::from(source_surface.height()) * projection.scale(),
            ),
        ];
        if covered.iter().any(|(start, end)| !start.is_finite() || !end.is_finite()) {
            return Err(CanvasError::Invalid("Backing crop coverage is unrepresentable"));
        }
        Ok(visible
            .into_iter()
            .zip(covered)
            .all(|(visible, covered)| {
                visible.is_some_and(|visible| covered.0 <= visible.0 && covered.1 >= visible.1)
            })
            .then_some(projection))
    }
}

fn intersection(start: f64, end: f64, bound: f64) -> Option<(f64, f64)> {
    let start = start.max(0.0);
    let end = end.min(bound);
    (start < end).then_some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;
    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn symmetric_backing_offsets_crop_pixels_without_changing_integer_phase() -> TestResult {
        let mut target = Viewport::new(Extent::new(640, 480)?)?;
        target.reframe([10_000.25, 20_000.5], 1.25)?;
        target.pan_by([17.0, -9.0])?;
        let coverage = BackingCoverage::new(target, [96, 64])?;
        assert_eq!(coverage.margin(), [96, 64]);
        assert_eq!(coverage.viewport().surface(), Extent::new(832, 608)?);
        assert_eq!(coverage.viewport().zoom(), target.zoom());
        assert_eq!(coverage.viewport().pixel_translation(), [113, 55]);
        assert_eq!(coverage.viewport().document_at([96.0, 64.0])?, target.document_at([0.0; 2])?);
        let crop = coverage
            .crop(target, Extent::new(1_000_000, 1_000_000)?)?
            .ok_or("Construction target lost backing coverage")?;
        assert_eq!(crop.scale(), 1.0);
        assert_eq!(crop.offset(), [-96.0, -64.0]);
        for x in 0..640 {
            assert_eq!(crop.column(x), Some(x + 96));
        }
        for y in 0..480 {
            assert_eq!(crop.row(y), Some(y + 64));
        }
        Ok(())
    }

    #[test]
    fn repeated_pan_reversals_preserve_backing_blit_phase_without_drift() -> TestResult {
        let mut target = Viewport::new(Extent::new(320, 200)?)?;
        target.reframe([1000.125, 2000.25], 0.37)?;
        let initial = BackingCoverage::new(target, [48, 32])?;
        let mut previous = initial;
        for delta in
            std::iter::repeat_n([7.0, -3.0], 200).chain(std::iter::repeat_n([-7.0, 3.0], 200))
        {
            target.pan_by(delta)?;
            let next = BackingCoverage::new(target, [48, 32])?;
            let damage = next.viewport().repair_from(previous.viewport())?;
            assert!(damage.blit().is_some());
            assert!(damage.dirty().pixels() < next.viewport().surface().pixels());
            previous = next;
        }
        assert_eq!(previous, initial);
        Ok(())
    }

    #[test]
    fn crop_accepts_contained_pan_zoom_and_resize_but_rejects_missing_document_pixels() -> TestResult
    {
        let document = Extent::new(10_000, 10_000)?;
        let mut target = Viewport::new(Extent::new(400, 300)?)?;
        target.reframe([1000.0, 1000.0], 1.0)?;
        let coverage = BackingCoverage::new(target, [80, 60])?;

        let mut moved = target;
        moved.pan_by([80.0, -60.0])?;
        assert!(coverage.crop(moved, document)?.is_some());
        moved.pan_by([-161.0, 0.0])?;
        assert!(coverage.crop(moved, document)?.is_none());

        let mut zoomed = target;
        zoomed.zoom_at(2.0, [200.0, 150.0])?;
        assert!(coverage.crop(zoomed, document)?.is_some());
        zoomed = target;
        zoomed.zoom_at(0.5, [200.0, 150.0])?;
        assert!(coverage.crop(zoomed, document)?.is_none());

        let mut resized = target;
        resized.resize(Extent::new(200, 150)?)?;
        assert!(coverage.crop(resized, document)?.is_some());
        resized = target;
        resized.resize(Extent::new(800, 600)?)?;
        assert!(coverage.crop(resized, document)?.is_none());
        Ok(())
    }

    #[test]
    fn crop_requires_only_document_intersection_and_leaves_unbacked_matte_safe() -> TestResult {
        let target = Viewport::new(Extent::new(100, 100)?)?;
        let coverage = BackingCoverage::new(target, [10, 10])?;
        let mut moved = target;
        moved.pan_by([-50.0, -50.0])?;
        let crop = coverage
            .crop(moved, Extent::new(70, 70)?)?
            .ok_or("Visible document pixels were not retained")?;
        assert_eq!(crop.column(0), Some(60));
        assert_eq!(crop.row(0), Some(60));
        assert_eq!(crop.column(99), None);
        assert_eq!(crop.row(99), None);
        assert!(coverage.crop(moved, Extent::new(1000, 1000)?)?.is_none());
        moved.pan_by([-10_000.0, -10_000.0])?;
        assert!(coverage.crop(moved, Extent::new(70, 70)?)?.is_some());
        Ok(())
    }

    #[test]
    fn maximum_surfaces_clamp_symmetrically_and_translation_exhaustion_is_atomic() -> TestResult {
        let target = Viewport::new(Extent::new(16_380, 10)?)?;
        let coverage = BackingCoverage::new(target, [u32::MAX; 2])?;
        assert_eq!(coverage.margin(), [2, 8187]);
        assert_eq!(coverage.viewport().surface(), Extent::new(16_384, 16_384)?);
        let full = Viewport::new(Extent::new(16_384, 16_384)?)?;
        assert_eq!(BackingCoverage::new(full, [u32::MAX; 2])?.margin(), [0; 2]);

        let mut translated = Viewport::new(Extent::new(10, 10)?)?;
        translated.pan_by([f64::from(i32::MAX), 0.0])?;
        let before = translated;
        assert!(translated.expand_surface(Extent::new(12, 12)?, [1, 1]).is_err());
        assert_eq!(translated, before);
        assert!(translated.expand_surface(Extent::new(9, 12)?, [0, 0]).is_err());
        assert_eq!(translated, before);
        Ok(())
    }
}
