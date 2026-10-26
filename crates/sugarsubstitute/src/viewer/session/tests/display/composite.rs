// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve detailed overlap and complete current-source evidence in composite displays.
use super::*;

#[test]
fn partial_detail_selects_shared_composite_contract_before_cpu_or_gpu_projection()
-> Result<(), Box<dyn Error>> {
    let document = Extent::new(4096, 4096)?;
    let mut source_view = Viewport::new(Extent::new(320, 240)?)?;
    source_view.reframe([1000.0, 1000.0], 1.0)?;
    let detail = coverage::detail(source_view)?;
    let fallback = coverage::fallback(document)?;
    let mut target = source_view;
    target.pan_by([200.0, 0.0])?;
    assert!(detail.crop(target, document)?.is_none());
    assert!(detail.overlaps(target, document)?);
    let reconstruction = Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3);
    let detail_stamp = FrameStamp { surface: SurfaceId::new(1)?, generation: Generation::new(2)? };
    let fallback_stamp = FrameStamp { generation: Generation::new(1)?, ..detail_stamp };
    let shown = |tier, coverage| Presented {
        tier,
        coverage,
        view: coverage.viewport(),
        reconstruction,
        document: (1, 1),
        source_revision: 7,
    };

    let selected = backing::choose_backing(
        Some((shown(BackingTier::Detail, detail), detail_stamp)),
        Some((shown(BackingTier::Fallback, fallback), fallback_stamp)),
        target,
        document,
    )?
    .ok_or("Composite backing was not selected")?;
    assert_eq!(selected.primary.shown.tier, BackingTier::Detail);
    assert_eq!(selected.primary.source, detail_stamp);
    assert_eq!(selected.primary.projection.target(), target);
    let underlay = selected.underlay.ok_or("Fallback underlay was not selected")?;
    assert_eq!(underlay.shown.tier, BackingTier::Fallback);
    assert_eq!(underlay.source, fallback_stamp);
    assert_eq!(underlay.projection.target(), target);
    Ok(())
}

#[test]
fn composite_evidence_requires_complete_underlay_and_matching_output_camera()
-> Result<(), Box<dyn Error>> {
    let document = Extent::new(4096, 4096)?;
    let mut source_view = Viewport::new(Extent::new(320, 240)?)?;
    source_view.reframe([1000.0, 1000.0], 1.0)?;
    let detail = coverage::detail(source_view)?;
    let fallback = coverage::fallback(document)?;
    let mut target_view = source_view;
    target_view.pan_by([200.0, 0.0])?;
    let source = FrameStamp { surface: SurfaceId::new(1)?, generation: Generation::new(2)? };
    let target = FrameStamp { generation: Generation::new(3)?, ..source };
    let underlay_source = FrameStamp { generation: Generation::new(1)?, ..source };
    let primary = ViewProjection::new(detail.viewport(), target_view)?;
    let underlay = work::UnderlayEvidence {
        source: underlay_source,
        coverage: fallback,
        projection: fallback
            .crop(target_view, document)?
            .ok_or("Fallback did not cover composite camera")?,
    };
    let evidence = DisplayEvidence {
        underlay: Some(underlay),
        tier: BackingTier::Detail,
        target,
        source,
        view: target_view,
        coverage: detail,
        projection: primary,
        document: (1, 1),
        source_revision: 7,
    };
    assert!(evidence.proves_complete(document)?);
    assert!(!DisplayEvidence { underlay: None, ..evidence }.proves_complete(document)?);

    let mut foreign_view = target_view;
    foreign_view.pan_by([1.0, 0.0])?;
    let foreign = work::UnderlayEvidence {
        projection: ViewProjection::new(fallback.viewport(), foreign_view)?,
        ..underlay
    };
    assert!(!DisplayEvidence { underlay: Some(foreign), ..evidence }.proves_complete(document)?);
    let future_underlay = work::UnderlayEvidence {
        source: FrameStamp { generation: Generation::new(4)?, ..underlay_source },
        ..underlay
    };
    assert!(
        !DisplayEvidence { underlay: Some(future_underlay), ..evidence }
            .proves_complete(document)?
    );
    assert!(
        !DisplayEvidence { tier: BackingTier::Fallback, ..evidence }.proves_complete(document)?
    );
    Ok(())
}
