// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Content moved beyond the extent is kept, restored when moved back, and never read inside.
use super::*;

const FULL: u16 = FULL_COVERAGE;

fn pattern(extent: Extent, region: Region, context: &JobContext) -> Result<Coverage, ImageError> {
    let mut coverage = Coverage::new(extent, 0, context.memory())?;
    coverage
        .update(region, context.memory(), |x, y, _| ((x * 977 + y * 613) % 65_521 + 1) as u16)?;
    Ok(coverage)
}

#[test]
fn moving_content_off_the_extent_and_back_restores_it_exactly() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(300, 200)?;
        let coverage = pattern(extent, Region::new(150, 40, 130, 100)?, context)?;
        let away = coverage.translate([120, -70], context)?;
        // Reads see only what is left inside the extent; the rest is kept beyond it.
        assert_eq!(away.bounds(), Some(Region::new(270, 0, 30, 70)?));
        let (outside, origin) = away.outside().ok_or(ImageError::Invalid("Nothing kept"))?;
        assert_eq!(outside.bounds(), Some(outside.extent().region()));
        assert!(origin[0] >= 270 || origin[1] < 0);
        assert_eq!(away.content_bounds(), Some([270, -30, 400, 70]));
        assert_ne!(away, coverage.translate([120, -70], context)?.without_outside());
        let back = away.translate([-120, 70], context)?;
        assert_eq!(back, coverage);
        assert!(back.outside().is_none());
        // Moving everything off the extent still keeps it.
        let gone = coverage.translate([400, 0], context)?;
        assert_eq!(gone.bounds(), None);
        assert_eq!(gone.translate([-400, 0], context)?, coverage);
        Ok(())
    })
}

#[test]
fn transforms_placement_and_reframing_keep_content_beyond_the_extent() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(200, 200)?;
        let coverage = pattern(extent, Region::new(120, 50, 70, 40)?, context)?;
        // A flip about x = 200 lands everything beyond the right edge; flipping back restores it.
        let flip = CoverageAffine::new([-1.0, 0.0, 0.0, 1.0, 400.0, 0.0])?.aligned();
        let flipped = coverage.transform(flip, context)?;
        assert_eq!(flipped.bounds(), None);
        assert_eq!(flipped.content_bounds(), Some([210, 50, 280, 90]));
        assert_eq!(flipped.transform(flip, context)?, coverage);
        // A fragment placed half beyond the edge lands back whole.
        let mask = Coverage::new(extent, 0, context.memory())?;
        let fragment = coverage.translate([60, 0], context)?;
        let placed = mask.over(&fragment, &fragment, context)?;
        assert_eq!(placed.content_bounds(), Some([180, 50, 250, 90]));
        assert_eq!(placed.translate([-60, 0], context)?, coverage);
        // A smaller canvas keeps what it no longer shows; growing it again shows it.
        let smaller =
            coverage.reframe(Extent::new(150, 150)?, [0, 0], context.memory(), context)?;
        assert_eq!(smaller.bounds(), Some(Region::new(120, 50, 30, 40)?));
        let again = smaller.reframe(extent, [0, 0], context.memory(), context)?;
        assert_eq!(again, coverage);
        // Combining keeps content beyond; intersecting with in-extent coverage drops it.
        let full = Coverage::new(extent, FULL, context.memory())?;
        assert_eq!(
            flipped.combine(&full, CombineMode::Add, context)?.content_bounds(),
            Some([0, 0, 280, 200])
        );
        assert!(flipped.combine(&full, CombineMode::Intersect, context)?.outside().is_none());
        // Expanding a mask partly beyond the edge grows it as one raster.
        let grown = coverage.translate([50, 0], context)?.expand(5, context)?;
        assert_eq!(grown.content_bounds(), Some([165, 45, 245, 95]));
        Ok(())
    })
}

#[test]
fn content_beyond_is_only_attached_in_canonical_form() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(100, 100)?;
        let away =
            pattern(extent, Region::new(60, 10, 30, 30)?, context)?.translate([30, 0], context)?;
        let (outside, origin) = away.outside().ok_or(ImageError::Invalid("Nothing kept"))?;
        let rebuilt = away.without_outside().with_outside(outside.clone(), origin)?;
        assert_eq!(rebuilt, away);
        // Overlapping the extent, repeating an attachment or padding the frame is refused.
        assert!(away.without_outside().with_outside(outside.clone(), [50, 10]).is_err());
        assert!(away.clone().with_outside(outside.clone(), origin).is_err());
        let padded = outside.reframe(Extent::new(30, 40)?, [0, 0], context.memory(), context)?;
        assert!(away.without_outside().with_outside(padded, origin).is_err());
        Ok(())
    })
}
