// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify completed backing coverage for image-free and image-visible inspection cameras.

use super::*;

#[test]
fn in_flight_matte_coverage_survives_only_the_matching_physical_extent()
-> Result<(), Box<dyn Error>> {
    for matching_extent in [true, false] {
        let Fixture { mut session, wake, boundary, _directory } =
            preparing(Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3))?;
        let now = session.navigation_time();
        let admitted = session.document.tools.navigate(
            sugarsubstitute_canvas::NavigationCommand::Frame {
                center: [-3.25, 2.75],
                span: [1.0 / 4096.0; 2],
            },
            now,
        )?;
        assert!(matches!(
            admitted,
            sugarsubstitute_canvas::NavigationCommandOutcome::Applied { .. }
        ));
        let view = session.document.tools.navigation().viewport();
        let coverage = coverage::detail(view)?;
        assert!(!coverage.overlaps(view, Extent::new(4096, 4096)?)?);
        let rendering = session.rendering.as_mut().ok_or("Missing admitted fine work")?;
        rendering.coverage = coverage;
        rendering.display_extent =
            if matching_extent { view.surface() } else { Extent::new(1, 1)? };
        session.cancel_obsolete_refinement()?;
        let rendering = session.rendering.as_mut().ok_or("Fine work lost its owned lifetime")?;
        assert_eq!(rendering.superseded, !matching_extent);
        rendering.superseded = true;
        rendering.job.cancel();
        observe_terminal(&mut session, &wake, &boundary)?;
        assert!(session.failed.is_none());
    }
    Ok(())
}

#[test]
fn completed_fine_coverage_is_current_when_the_camera_contains_only_matte()
-> Result<(), Box<dyn Error>> {
    let document = Extent::new(1, 1)?;
    let mut target = Viewport::new(Extent::new(8, 8)?)?;
    target.reframe([-3.65, 2.35], 10.0)?;
    let coverage = coverage::detail(target)?;
    assert!(!coverage.overlaps(target, document)?);
    assert!(coverage.crop(target, document)?.is_some());
    for tier in [BackingTier::Detail, BackingTier::Navigation] {
        assert!(coverage::is_useful(tier, coverage, target, document)?);
    }
    Ok(())
}

#[test]
fn completed_fine_coverage_retains_partial_image_overlap_and_rejects_disjoint_visible_image()
-> Result<(), Box<dyn Error>> {
    let document = Extent::new(8192, 8192)?;
    let mut target = Viewport::new(Extent::new(320, 240)?)?;
    target.reframe([4096.0, 4096.0], 1.0)?;
    let coverage = coverage::detail(target)?;
    target.pan_by([200.0, 0.0])?;
    assert!(coverage.overlaps(target, document)?);
    assert!(coverage.crop(target, document)?.is_none());
    for tier in [BackingTier::Detail, BackingTier::Navigation] {
        assert!(coverage::is_useful(tier, coverage, target, document)?);
    }
    target.pan_by([2000.0, 0.0])?;
    assert!(!coverage.overlaps(target, document)?);
    assert!(coverage.crop(target, document)?.is_none());
    for tier in [BackingTier::Detail, BackingTier::Navigation] {
        assert!(!coverage::is_useful(tier, coverage, target, document)?);
    }
    Ok(())
}
