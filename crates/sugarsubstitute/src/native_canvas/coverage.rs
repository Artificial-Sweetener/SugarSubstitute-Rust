// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound retained whole-document and detailed camera coverage independently of execution.

use sugarsubstitute_canvas::{BackingCoverage, Viewport};
use sugarsubstitute_image_ops::Extent;

/// Symmetric retained-detail overscan in physical output pixels.
pub(super) const DETAIL_MARGIN: [u32; 2] = [128; 2];
/// Inner safety band that triggers repair before visible coverage is exhausted.
const DETAIL_REFRESH_MARGIN: [u32; 2] = [64; 2];
/// Maximum edge of the low-resolution whole-document fallback.
const FALLBACK_MAX_EDGE: u32 = 1024;
/// Interior fit cap leaves one physical guard pixel on every fallback edge.
const FALLBACK_INTERIOR_MAX_EDGE: u32 = FALLBACK_MAX_EDGE - 2;

/// Independently retained backing selected for one coherent whole-output projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BackingTier {
    /// Bounded whole-document coverage used when detail cannot cover the target.
    Fallback,
    /// Canonical pyramid-backed navigation coverage, sampled bilinearly before fine settling.
    Navigation,
    /// Full-quality overscanned coverage near the current camera.
    Detail,
}

/// Construct bounded whole-document coverage while preserving document aspect ratio.
pub(super) fn fallback(document: Extent) -> Result<BackingCoverage, Box<dyn std::error::Error>> {
    let longest = document.width().max(document.height());
    let edge = longest.min(FALLBACK_INTERIOR_MAX_EDGE);
    let scaled = |value: u32| {
        u32::try_from((u64::from(value) * u64::from(edge)).div_ceil(u64::from(longest)).max(1))
    };
    let surface = Extent::new(scaled(document.width())?, scaled(document.height())?)?;
    let mut view = Viewport::new(surface)?;
    view.fit(document)?;
    Ok(BackingCoverage::new(view, [1; 2])?)
}

/// Construct overscanned detail coverage centered on the requested camera.
pub(super) fn detail(target: Viewport) -> Result<BackingCoverage, Box<dyn std::error::Error>> {
    Ok(BackingCoverage::new(target, DETAIL_MARGIN)?)
}

/// Refresh when sampling phase changes or before the target consumes half the detail overscan.
pub(super) fn detail_needs_refresh(
    backing: BackingCoverage,
    target: Viewport,
    document: Extent,
) -> Result<bool, Box<dyn std::error::Error>> {
    if !backing.viewport().shares_sampling_grid(target) {
        return Ok(true);
    }
    let guarded = BackingCoverage::new(target, DETAIL_REFRESH_MARGIN)?;
    if guarded.margin() != DETAIL_REFRESH_MARGIN {
        return Ok(false);
    }
    Ok(backing.crop(guarded.viewport(), document)?.is_none())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_fallback_covers_whole_document_after_pan_and_zoom_out()
    -> Result<(), Box<dyn std::error::Error>> {
        let document = Extent::new(16_384, 8_192)?;
        let backing = fallback(document)?;
        assert_eq!(backing.viewport().surface(), Extent::new(1024, 513)?);
        assert_eq!(backing.margin(), [1; 2]);

        let mut target = Viewport::new(Extent::new(640, 480)?)?;
        target.fit(document)?;
        target.zoom_at(0.5, [320.0, 240.0])?;
        target.pan_by([173.0, -91.0])?;
        assert!(backing.crop(target, document)?.is_some());
        Ok(())
    }

    #[test]
    fn guarded_fallback_covers_known_portrait_fractional_fit_regression()
    -> Result<(), Box<dyn std::error::Error>> {
        let document = Extent::new(1920, 3024)?;
        let backing = fallback(document)?;
        assert_eq!(backing.viewport().surface(), Extent::new(651, 1024)?);
        assert_eq!(backing.margin(), [1; 2]);
        let mut target = Viewport::new(Extent::new(3840, 2160)?)?;
        target.fit(document)?;
        target.zoom_at(0.8, [1920.0, 1080.0])?;
        assert!(backing.crop(target, document)?.is_some());
        Ok(())
    }

    #[test]
    fn guarded_fallback_covers_transpose_coprime_extreme_and_resized_views()
    -> Result<(), Box<dyn std::error::Error>> {
        let rows = [
            (
                [3024, 1920],
                [2160, 3840],
                Some([3840, 2160]),
                0.8,
                [0.5, 0.5],
                [0.0, 0.0],
                [1024, 651],
            ),
            (
                [1001, 997],
                [1001, 777],
                Some([777, 1001]),
                0.8,
                [0.371, 0.613],
                [0.25, -0.5],
                [1003, 999],
            ),
            ([16_383, 8191], [3840, 2160], None, 1.25, [0.371, 0.613], [173.0, -91.0], [1024, 513]),
            ([65_535, 17], [16_384, 257], None, 8.0, [0.5, 0.5], [-173.0, 91.0], [1024, 3]),
            (
                [4093, 6143],
                [853, 1279],
                Some([1279, 853]),
                0.125,
                [0.371, 0.613],
                [173.0, -91.0],
                [683, 1024],
            ),
            ([1, 1], [16_384, 16_384], None, 0.125, [0.5, 0.5], [173.0, -91.0], [3, 3]),
        ];
        for (document, initial, resized, factor, anchor, pan, expected) in rows {
            let document = Extent::new(document[0], document[1])?;
            let backing = fallback(document)?;
            assert_eq!(backing.viewport().surface(), Extent::new(expected[0], expected[1])?);
            assert_eq!(backing.margin(), [1; 2]);
            assert!(backing.viewport().surface().width() <= FALLBACK_MAX_EDGE);
            assert!(backing.viewport().surface().height() <= FALLBACK_MAX_EDGE);
            let mut target = Viewport::new(Extent::new(initial[0], initial[1])?)?;
            target.fit(document)?;
            if let Some(resized) = resized {
                target.resize(Extent::new(resized[0], resized[1])?)?;
            }
            let surface = target.surface();
            target.zoom_at(
                factor,
                [f64::from(surface.width()) * anchor[0], f64::from(surface.height()) * anchor[1]],
            )?;
            target.pan_by(pan)?;
            assert!(backing.crop(target, document)?.is_some(), "{document:?}");
        }
        Ok(())
    }

    #[test]
    fn detail_guard_covers_small_motion_but_hands_large_motion_to_fallback()
    -> Result<(), Box<dyn std::error::Error>> {
        let document = Extent::new(4096, 4096)?;
        let mut target = Viewport::new(Extent::new(640, 480)?)?;
        target.reframe([1000.0, 1000.0], 1.0)?;
        let backing = detail(target)?;

        let mut inside = target;
        inside.pan_by([128.0, -128.0])?;
        assert!(backing.crop(inside, document)?.is_some());
        let mut outside = target;
        outside.pan_by([129.0, 0.0])?;
        assert!(backing.crop(outside, document)?.is_none());
        assert!(fallback(document)?.crop(outside, document)?.is_some());
        Ok(())
    }

    #[test]
    fn detail_refreshes_at_inner_guard_while_visible_coverage_remains_complete()
    -> Result<(), Box<dyn std::error::Error>> {
        let document = Extent::new(4096, 4096)?;
        let mut target = Viewport::new(Extent::new(640, 480)?)?;
        target.reframe([1000.0, 1000.0], 1.0)?;
        let backing = detail(target)?;

        target.pan_by([65.0, 0.0])?;
        assert!(backing.crop(target, document)?.is_some());
        assert!(detail_needs_refresh(backing, target, document)?);
        Ok(())
    }

    #[test]
    fn detail_refresh_preserves_integer_pan_but_rejects_fractional_pan_and_odd_resize()
    -> Result<(), Box<dyn std::error::Error>> {
        let document = Extent::new(4096, 4096)?;
        let mut source = Viewport::new(Extent::new(640, 480)?)?;
        source.reframe([1000.0, 1000.0], 1.0)?;
        let backing = detail(source)?;

        let mut integer_pan = source;
        integer_pan.pan_by([17.0, -11.0])?;
        assert!(!detail_needs_refresh(backing, integer_pan, document)?);

        let mut fractional_pan = source;
        fractional_pan.pan_by([0.5, 0.25])?;
        assert!(backing.crop(fractional_pan, document)?.is_some());
        assert!(detail_needs_refresh(backing, fractional_pan, document)?);

        let mut odd_resize = source;
        odd_resize.resize(Extent::new(641, 481)?)?;
        assert!(backing.crop(odd_resize, document)?.is_some());
        assert!(detail_needs_refresh(backing, odd_resize, document)?);
        Ok(())
    }
}
