// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise publication through a desktop-owned CPU surface and the headless path.

use super::*;

/// Admit real CPU projection pixels and deliver their draw through the session's execution port.
pub(super) fn cpu_draw(attached: bool) -> Result<(Fixture, FrameStamp), Box<dyn Error>> {
    let (mut session, clock, directory) = session()?;
    if attached {
        session.attach_cpu_surface()?;
    }
    let wake = Arc::new(Wake::default());
    let source = FrameStamp { surface: SurfaceId::new(1)?, generation: Generation::new(1)? };
    let target = FrameStamp { generation: Generation::new(2)?, ..source };
    let second_target = FrameStamp { generation: Generation::new(3)?, ..source };
    let view = session.document.tools.navigation().viewport();
    let products = cpu_display_products(
        &mut session,
        wake.clone(),
        CpuDisplayBoundary {
            source,
            target,
            second_target,
            source_view: view,
            projected_view: view,
            replacement_extent: view.surface(),
        },
    )?;
    let identity =
        (products.document.snapshot().identity(), products.document.snapshot().revision(), 0);
    session.document.scene = Some(Scene {
        document: products.document,
        source: None,
        source_revision: 7,
        original: None,
        compared: None,
        placed: Vec::new(),
    });
    session.selection = 7;
    session.source_sequence = 7;
    session.document.front = Some(products.surface);
    let mut drawing = Some(work::Draw {
        underlay: None,
        tier: BackingTier::Fallback,
        target,
        source,
        view,
        coverage: BackingCoverage::new(view, [0; 2])?,
        projection: ViewProjection::new(view, view)?,
        pixels: work::DrawPixels::Cpu(products.projection),
        document: identity,
        source_revision: 7,
        overlay_generation: 0,
        backdrop_generation: 0,
        backdrop_active: false,
        verified: true,
    });
    let spec = session.spec(
        Operation::Infrastructure,
        JobClass::InfrastructureOrdinary,
        65536,
        Duration::from_secs(30),
        None,
    )?;
    let runtime = session.runtime.as_ref().ok_or("CPU fixture execution is unavailable")?;
    let job = runtime.client.submit_blocking_owned_input(
        &runtime.scope,
        spec,
        &mut drawing,
        |drawing, _| Ok::<_, WorkError>(drawing),
    )?;
    session.pacing.submitted(
        target,
        PresentationMode::Projected,
        Duration::ZERO,
        Duration::ZERO,
    )?;
    session.displaying = Some(super::super::super::display::Displaying {
        target,
        document: identity,
        source_revision: 7,
        backdrop_generation: 0,
        job,
    });
    Ok((Fixture { session, clock, wake, view, source, _directory: directory }, target))
}

#[test]
fn embedded_cpu_surface_publishes_only_after_matching_native_acknowledgement()
-> Result<(), Box<dyn Error>> {
    let (mut fixture, target) = cpu_draw(true)?;
    assert!(fixture.session.attach_cpu_surface().is_err());
    consume(&mut fixture.session, &fixture.wake)?;
    let frame = fixture.session.frame().ok_or("CPU projection was not published")?;
    assert_eq!(frame.target, target);
    assert!(frame.pending);
    assert!(fixture.session.document.display_evidence.is_none());
    assert!(fixture.session.document.native_presented.is_none());
    let foreign = FrameStamp { generation: Generation::new(4)?, ..target };
    assert!(fixture.session.did_present_cpu(foreign).is_err());
    assert!(fixture.session.frame().ok_or("CPU projection was lost")?.pending);
    fixture.session.did_present_cpu(target)?;
    assert!(!fixture.session.frame().ok_or("CPU projection was lost")?.pending);
    assert_eq!(fixture.session.document.native_presented, Some(target));
    assert_eq!(fixture.session.document.display_source, Some(fixture.source));
    assert!(fixture.session.displayed_coverage_is_valid()?);
    assert!(fixture.session.did_present_cpu(target).is_err());
    Ok(())
}

#[test]
fn replacing_the_selected_output_releases_an_unpresented_cpu_frame() -> Result<(), Box<dyn Error>> {
    let (mut fixture, obsolete) = cpu_draw(true)?;
    consume(&mut fixture.session, &fixture.wake)?;
    assert!(fixture.session.frame().ok_or("Missing pending frame")?.pending);

    let replacement = fixture._directory.path().join("next-output.png");
    fixture.session.open(replacement, CacheReuse::Compatible, None)?;

    assert!(fixture.session.frame().is_none());
    assert!(fixture.session.did_present_cpu(obsolete).is_err());
    assert!(fixture.session.pending_open.is_some());
    assert!(!matches!(
        fixture.session.pacing.readiness(Duration::ZERO)?,
        PresentationReadiness::Busy { .. }
    ));
    Ok(())
}

#[cfg(feature = "native-composition")]
#[test]
fn selection_cancel_rejects_an_already_completed_display_before_native_publication()
-> Result<(), Box<dyn Error>> {
    let (mut fixture, target) = cpu_draw(true)?;
    loop {
        let observed = fixture.wake.revision();
        let runtime = fixture.session.runtime.as_ref().ok_or("Draw execution absent")?;
        runtime.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        let drawing = fixture.session.displaying.as_ref().ok_or("Draw identity absent")?;
        if matches!(drawing.job.status().phase, JobPhase::Terminal(TerminalKind::Success)) {
            break;
        }
        fixture.wake.wait_after(observed);
    }
    fixture.session.cancel_selection()?;
    consume(&mut fixture.session, &fixture.wake)?;
    assert!(fixture.session.document.cpu_pending.is_none());
    assert!(fixture.session.document.pending_cpu_evidence.is_none());
    assert!(fixture.session.document.native_presented.is_none());
    assert!(fixture.session.did_present_cpu(target).is_err());
    assert!(fixture.session.failed.is_none());
    assert!(!matches!(
        fixture.session.pacing.readiness(Duration::ZERO)?,
        PresentationReadiness::Busy { .. }
    ));
    Ok(())
}

#[test]
fn resizing_releases_only_an_obsolete_unpresented_cpu_frame() -> Result<(), Box<dyn Error>> {
    let (mut fixture, obsolete) = cpu_draw(true)?;
    consume(&mut fixture.session, &fixture.wake)?;
    let original = fixture.view.surface();

    fixture.session.resize(original)?;
    assert!(fixture.session.frame().ok_or("Unchanged frame was lost")?.pending);
    assert!(fixture.session.resize(Extent::new(16_385, 1)?).is_err());
    assert!(fixture.session.frame().ok_or("Rejected resize lost the frame")?.pending);

    fixture.session.resize(Extent::new(256, 192)?)?;
    assert!(fixture.session.frame().is_none());
    assert!(fixture.session.did_present_cpu(obsolete).is_err());
    assert!(!matches!(
        fixture.session.pacing.readiness(Duration::ZERO)?,
        PresentationReadiness::Busy { .. }
    ));
    Ok(())
}

#[test]
fn failed_frame_withdrawal_preserves_the_offered_source_and_pending_frame()
-> Result<(), Box<dyn Error>> {
    let (mut fixture, target) = cpu_draw(true)?;
    fixture.clock.0.store(20_000_000, Ordering::SeqCst);
    consume(&mut fixture.session, &fixture.wake)?;
    fixture.clock.0.store(0, Ordering::SeqCst);
    let previous = fixture.session.selection;
    let mut replacement = Some(ImageSource::file(fixture._directory.path().join("next.png")));

    assert!(fixture.session.offer_source(&mut replacement, CacheReuse::Compatible, None).is_err());
    assert!(replacement.is_some());
    assert_eq!(fixture.session.selection, previous);
    assert!(fixture.session.pending_open.is_none());
    let frame = fixture.session.frame().ok_or("Pending frame was lost")?;
    assert_eq!(frame.target, target);
    assert!(frame.pending);
    Ok(())
}

#[test]
fn failed_resize_withdrawal_preserves_viewport_and_pending_frame() -> Result<(), Box<dyn Error>> {
    let (mut fixture, target) = cpu_draw(true)?;
    fixture.clock.0.store(20_000_000, Ordering::SeqCst);
    consume(&mut fixture.session, &fixture.wake)?;
    fixture.clock.0.store(0, Ordering::SeqCst);
    let original = fixture.session.document.tools.navigation().viewport().surface();

    assert!(fixture.session.resize(Extent::new(256, 192)?).is_err());
    assert_eq!(fixture.session.document.tools.navigation().viewport().surface(), original);
    let frame = fixture.session.frame().ok_or("Pending frame was lost")?;
    assert_eq!(frame.target, target);
    assert!(frame.pending);
    Ok(())
}
#[test]
fn headless_cpu_output_cannot_gain_a_native_presenter_after_work_starts()
-> Result<(), Box<dyn Error>> {
    let (mut fixture, target) = cpu_draw(false)?;
    assert!(fixture.session.attach_cpu_surface().is_err());
    consume(&mut fixture.session, &fixture.wake)?;
    let frame = fixture.session.frame().ok_or("Headless projection was not published")?;
    assert_eq!(frame.target, target);
    assert!(!frame.pending);
    assert!(fixture.session.did_present_cpu(target).is_err());
    assert!(fixture.session.displayed_coverage_is_valid()?);
    assert!(fixture.session.attach_cpu_surface().is_err());
    Ok(())
}

#[test]
fn cpu_surface_rejects_gpu_and_closed_sessions() -> Result<(), Box<dyn Error>> {
    let (mut gpu, _clock, _directory) = session()?;
    gpu.config.gpu =
        Some(crate::native_canvas::gpu_policy::GpuRequest::Required("vulkan:8086:a780".parse()?));
    assert!(gpu.attach_cpu_surface().is_err());
    let (mut closed, _clock, _directory) = session()?;
    closed.close()?;
    assert!(closed.attach_cpu_surface().is_err());
    Ok(())
}
