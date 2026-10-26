// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Measure first-source latency only at matching native presentation acknowledgement.

use super::{
    display::{CpuDisplayBoundary, cpu_display_products},
    refinement::Wake,
    *,
};
use sugarsubstitute_canvas::ViewProjection;

#[test]
fn first_image_latency_requires_matching_ack_and_is_recorded_only_once()
-> Result<(), Box<dyn Error>> {
    let (mut session, clock, directory) = session()?;
    let wake = Arc::new(Wake::default());
    let source = FrameStamp { surface: SurfaceId::new(1)?, generation: Generation::new(1)? };
    let first = FrameStamp { generation: Generation::new(2)?, ..source };
    let second = FrameStamp { generation: Generation::new(3)?, ..source };
    let source_view = session.tools.navigation().viewport();
    let mut output_view = source_view;
    output_view.pan_by([11.0, -7.0])?;
    let products = cpu_display_products(
        &mut session,
        wake,
        CpuDisplayBoundary {
            source,
            target: first,
            second_target: second,
            source_view,
            projected_view: output_view,
            replacement_extent: output_view.surface(),
        },
    )?;
    let document =
        (products.document.snapshot().identity(), products.document.snapshot().revision());
    session.scene = Some(Scene {
        document: products.document,
        source: None,
        source_revision: 7,
        original: None,
    });
    session.selection = 8;
    session.pending_first_image = Some((8, Duration::from_millis(10)));
    session.front = Some(products.surface);
    session.cpu_projection = Some(products.projection);
    session.display_buffers = [Some(products.spare), None];
    session.display_buffer_extent = Some(output_view.surface());
    session.projection_damage = Some(DamagePlan::full(output_view.surface()));
    session.cpu_pending = Some(first);
    let coverage = BackingCoverage::new(source_view, [0; 2])?;
    session.pending_cpu_evidence = Some(DisplayEvidence {
        tier: BackingTier::Detail,
        target: first,
        source,
        view: output_view,
        coverage,
        projection: ViewProjection::new(source_view, output_view)?,
        document,
        source_revision: 7,
    });
    session.pacing.submitted(first, PresentationMode::Projected, Duration::ZERO, Duration::ZERO)?;
    session.pacing.prepared(first, Duration::ZERO)?;

    clock.0.store(20_000_000, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(session.first_image_queue_latency, None);
    clock.0.store(30_000_000, std::sync::atomic::Ordering::SeqCst);
    session.did_present_cpu(first)?;
    assert_eq!(session.first_image_queue_latency, None);
    assert_eq!(session.pending_first_image, Some((8, Duration::from_millis(10))));
    session.scene.as_mut().ok_or("Scene missing")?.source_revision = 8;
    let first_projection = session.cpu_projection.replace(products.second_projection);
    session.cpu_pending = Some(second);
    session.pending_cpu_evidence = Some(DisplayEvidence {
        tier: BackingTier::Detail,
        target: second,
        source,
        view: output_view,
        coverage,
        projection: ViewProjection::new(source_view, output_view)?,
        document,
        source_revision: 8,
    });
    clock.0.store(100_000_000, std::sync::atomic::Ordering::SeqCst);
    session.pacing.submitted(
        second,
        PresentationMode::Projected,
        Duration::from_millis(100),
        Duration::from_millis(100),
    )?;
    session.pacing.prepared(second, Duration::from_millis(100))?;
    clock.0.store(120_000_000, std::sync::atomic::Ordering::SeqCst);
    session.did_present_cpu(second)?;
    assert_eq!(session.first_image_queue_latency, Some(Duration::from_millis(110)));
    assert_eq!(session.pending_first_image, None);
    session.open(
        directory.path().join("later.png"),
        sugarsubstitute_image_io::CacheReuse::Rebuild,
        None,
    )?;
    assert_eq!(session.pending_first_image, None);
    assert_eq!(session.first_image_queue_latency, Some(Duration::from_millis(110)));
    session.pending_open = None;
    drop(first_projection);
    Ok(())
}
