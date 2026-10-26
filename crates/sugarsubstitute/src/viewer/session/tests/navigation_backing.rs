// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Navigation prefers ready canonical coverage and preserves useful finer overlap while settling.
use super::*;
use sugarsubstitute_image_ops::Filter;

fn shown(
    tier: BackingTier,
    view: Viewport,
    generation: u64,
) -> Result<(Presented, FrameStamp), Box<dyn Error>> {
    Ok((
        Presented {
            tier,
            coverage: coverage::detail(view)?,
            view: coverage::detail(view)?.viewport(),
            reconstruction: Reconstruction::Finished(if tier == BackingTier::Navigation {
                Filter::Bilinear
            } else {
                Filter::Lanczos3
            }),
            document: (1, 1),
            source_revision: 7,
        },
        FrameStamp { surface: SurfaceId::new(1)?, generation: Generation::new(generation)? },
    ))
}

#[test]
fn zoom_out_prepares_the_destination_before_the_visible_camera_reaches_it()
-> Result<(), Box<dyn Error>> {
    let (mut session, _, _directory) = session()?;
    let current = session.tools.navigation().viewport();
    session.input(Input::Tool(ToolInput::Wheel { steps: -2.0, phase: ScrollPhase::Unphased }))?;
    assert_eq!(session.tools.navigation().viewport(), current);
    assert!(session.navigation_view().zoom() < current.zoom());
    assert_eq!(session.navigation_view(), session.tools.navigation().target_viewport());
    assert_eq!(session.reconstruction()?.filter(), Filter::Lanczos3);
    Ok(())
}

#[test]
fn ready_navigation_fills_newly_revealed_edges_beneath_retained_fine_detail()
-> Result<(), Box<dyn Error>> {
    let document = Extent::new(4096, 4096)?;
    let mut original = Viewport::new(Extent::new(1920, 1080)?)?;
    original.reframe([2000.0, 2000.0], 1.0)?;
    let detail = shown(BackingTier::Detail, original, 2)?;
    let mut target = original;
    target.zoom_at(0.5, [960.0, 540.0])?;
    let navigation = shown(BackingTier::Navigation, target, 4)?;
    let bootstrap = shown(BackingTier::Fallback, coverage::fallback(document)?.viewport(), 1)?;
    assert!(detail.0.coverage.crop(target, document)?.is_none());
    let selected =
        backing::choose_backing(Some(detail), Some(navigation), Some(bootstrap), target, document)?
            .ok_or("Missing navigation selection")?;
    assert_eq!(selected.primary.source, detail.1);
    let underlay = selected.underlay.ok_or("Fine overlap lost its navigation underlay")?;
    assert_eq!(underlay.source, navigation.1);
    assert_eq!(underlay.shown.tier, BackingTier::Navigation);
    Ok(())
}

#[test]
fn navigation_does_not_mask_finer_ready_pixels_or_claim_missing_coverage()
-> Result<(), Box<dyn Error>> {
    let document = Extent::new(4096, 4096)?;
    let mut view = Viewport::new(Extent::new(320, 240)?)?;
    view.reframe([2000.0, 2000.0], 0.5)?;
    let old_detail = shown(BackingTier::Detail, view, 2)?;
    let mut target = view;
    target.zoom_at(2.0, [160.0, 120.0])?;
    let navigation = shown(BackingTier::Navigation, target, 4)?;
    let selected =
        backing::choose_backing(Some(old_detail), Some(navigation), None, target, document)?
            .ok_or("Missing ready navigation")?;
    assert_eq!(selected.primary.source, navigation.1);
    assert!(selected.underlay.is_none());
    let detail = shown(BackingTier::Detail, target, 5)?;
    let selected = backing::choose_backing(Some(detail), Some(navigation), None, target, document)?
        .ok_or("Missing fine selection")?;
    assert_eq!(selected.primary.source, detail.1);
    let mut disjoint = target;
    disjoint.pan_by([1000.0, 1000.0])?;
    assert!(backing::choose_backing(None, Some(navigation), None, disjoint, document)?.is_none());
    Ok(())
}
