// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify native navigation intent and exact preview/refinement decisions with an injected clock.
use super::*;
mod display;
mod edit_damage;
mod latency;
mod navigation_backing;
mod refinement;
mod runtime;
mod source_io;
use std::sync::atomic::{AtomicU64, Ordering};
use sugarsubstitute_canvas::{PointerSample, ScrollPhase, ToolButton, ToolInput, ToolModifiers};
use sugarsubstitute_desktop::canvas::{Input, Lifetime};
use sugarsubstitute_image_ops::Extent;

/// Retained-document allowance that keeps the default test execution capacity's headroom for
/// fixtures that saturate it; retention tests fund their own allowance.
const RETAINED_DOCUMENT_BYTES: u64 = 512 * 1024;
struct TestClock(AtomicU64);
impl Clock for TestClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::SeqCst))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
fn session() -> Result<(Session, Arc<TestClock>, tempfile::TempDir), Box<dyn Error>> {
    session_with_sequence(JobSequence::new())
}
fn session_with_sequence(
    sequence: JobSequence,
) -> Result<(Session, Arc<TestClock>, tempfile::TempDir), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let config = Config {
        gpu: None,
        capture: false,
        verify_native: false,
        cache: directory.path().to_path_buf(),
        initial: InitialSource::PickFile,
        locale: "en".into(),
        extent: Extent::new(320, 240)?,
        maximum_extent: Extent::new(64, 64)?,
        retained_document_bytes: RETAINED_DOCUMENT_BYTES,
    };
    let clock = Arc::new(TestClock(AtomicU64::new(0)));
    let mut session = Session::new(config, DiagnosticHealth::fixture(), clock.clone(), sequence)?;
    session.document.tools.set_content(Some(Extent::new(640, 480)?), None, Duration::ZERO)?;
    Ok((session, clock, directory))
}
#[test]
fn canvas_work_uses_the_composing_clock_and_shared_job_sequence() -> Result<(), Box<dyn Error>> {
    let sequence = JobSequence::new();
    assert_eq!(sequence.allocate()?.get(), 1);
    let (session, clock, _directory) = session_with_sequence(sequence.clone())?;
    clock.0.store(42, Ordering::SeqCst);

    let spec = session.spec(
        Operation::Infrastructure,
        JobClass::InfrastructureOrdinary,
        1024,
        Duration::from_secs(1),
        None,
    )?;
    assert_eq!(spec.id.get(), 2);
    assert_eq!(spec.deadline, Some(Moment::from_nanos(1_000_000_042)));
    assert_eq!(sequence.allocate()?.get(), 3);
    Ok(())
}
#[test]
fn normalized_tool_zoom_uses_the_execution_clock_and_keeps_finished_work_during_motion()
-> Result<(), Box<dyn Error>> {
    let (mut session, clock, _directory) = session()?;
    let pointer = [71.25, 53.5];
    session.input(Input::Tool(ToolInput::Pointer(PointerSample::mouse(
        pointer,
        ToolModifiers::NONE,
    ))))?;
    let anchor = session.document.tools.navigation().viewport().document_at(pointer)?;
    session.input(Input::Tool(ToolInput::Wheel { steps: 1.0, phase: ScrollPhase::Unphased }))?;
    assert_eq!(session.document.tools.navigation().target_viewport().zoom(), 0.625);
    assert_eq!(
        session.reconstruction()?,
        Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3)
    );
    assert!(session.next_deadline().is_some());
    clock.0.store(40_000_000, Ordering::SeqCst);
    session.poll(Duration::ZERO)?;
    let view = session.document.tools.navigation().viewport();
    assert_eq!(view.zoom(), 0.59375);
    assert_eq!(view.document_at(pointer)?, anchor);
    clock.0.store(119_999_999, Ordering::SeqCst);
    session.poll(Duration::ZERO)?;
    assert_eq!(session.document.tools.navigation().viewport().zoom(), 0.625);
    assert_eq!(
        session.reconstruction()?,
        Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3)
    );
    clock.0.store(120_000_000, Ordering::SeqCst);
    session.poll(Duration::ZERO)?;
    assert_eq!(
        session.reconstruction()?,
        Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3)
    );
    assert!(session.next_deadline().is_none());
    session.input(Input::Tool(ToolInput::Wheel { steps: 0.5, phase: ScrollPhase::End }))?;
    clock.0.store(180_000_000, Ordering::SeqCst);
    session.poll(Duration::ZERO)?;
    assert_eq!(session.document.tools.navigation().viewport().zoom(), 0.625 * 1.25_f64.sqrt());
    assert_eq!(
        session.reconstruction()?,
        Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3)
    );
    Ok(())
}
#[test]
fn panning_keeps_finished_quality_and_focus_loss_ends_tool_capture() -> Result<(), Box<dyn Error>> {
    let (mut session, clock, _directory) = session()?;
    session.input(Input::Tool(ToolInput::OneToOne))?;
    clock.0.store(80_000_000, Ordering::SeqCst);
    session.poll(Duration::ZERO)?;
    session.input(Input::Tool(ToolInput::Pointer(PointerSample::mouse(
        [20.0, 30.0],
        ToolModifiers::NONE,
    ))))?;
    session.input(Input::Tool(ToolInput::Button {
        button: ToolButton::Primary,
        pressed: true,
        modifiers: ToolModifiers::NONE,
    }))?;
    session.input(Input::Tool(ToolInput::Pointer(PointerSample::mouse(
        [27.0, 19.0],
        ToolModifiers::NONE,
    ))))?;
    assert_eq!(session.document.tools.navigation().viewport().origin(), [-7.0, 11.0]);
    assert!(session.feedback().capture);
    assert_eq!(
        session.reconstruction()?,
        Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3)
    );
    assert!(session.next_deadline().is_none());
    session.input(Input::Unfocused)?;
    session.input(Input::Tool(ToolInput::Pointer(PointerSample::mouse(
        [100.0, 100.0],
        ToolModifiers::NONE,
    ))))?;
    assert_eq!(session.document.tools.navigation().viewport().origin(), [-7.0, 11.0]);
    assert!(!session.feedback().capture);
    assert_eq!(
        session.reconstruction()?,
        Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3)
    );
    assert!(!session.take_picker_request());
    Ok(())
}

#[test]
fn fractional_pan_repairs_finished_detail_on_the_retained_grid_until_release()
-> Result<(), Box<dyn Error>> {
    let (mut session, clock, _directory) = session()?;
    session.input(Input::Tool(ToolInput::OneToOne))?;
    clock.0.store(80_000_000, Ordering::SeqCst);
    session.poll(Duration::ZERO)?;
    let target = session.document.tools.navigation().viewport();
    let retained = coverage::detail(target)?;
    let finished = Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3);
    session.document.presented = Some(Presented {
        tier: BackingTier::Detail,
        coverage: retained,
        view: retained.viewport(),
        sampling_scale: retained.viewport().zoom(),
        reconstruction: finished,
        document: (1, 1, 0),
        source_revision: 0,
        comparison: None,
    });

    session.input(Input::Tool(ToolInput::Pointer(PointerSample::mouse(
        [20.0, 30.0],
        ToolModifiers::NONE,
    ))))?;
    session.input(Input::Tool(ToolInput::Button {
        button: ToolButton::Primary,
        pressed: true,
        modifiers: ToolModifiers::NONE,
    }))?;
    session.input(Input::Tool(ToolInput::Pointer(PointerSample::mouse(
        [27.0, 19.0],
        ToolModifiers::NONE,
    ))))?;
    assert_eq!(session.reconstruction()?, finished);
    session.input(Input::Tool(ToolInput::Pointer(PointerSample::mouse(
        [27.5, 19.25],
        ToolModifiers::NONE,
    ))))?;
    assert_eq!(session.reconstruction()?, finished);
    let actual = session.document.tools.navigation().viewport();
    let repair_view = session.detail_view(actual, (1, 1, 0), 0, finished)?;
    assert!(repair_view.shares_sampling_grid(retained.viewport()));
    assert_eq!(session.document.tools.navigation().viewport(), actual);
    let repair = coverage::detail(repair_view)?.viewport().repair_from(retained.viewport())?;
    assert!(repair.blit().is_some());
    assert!(repair.dirty().pixels() < repair.dirty().surface().pixels());
    session.input(Input::Tool(ToolInput::Button {
        button: ToolButton::Primary,
        pressed: false,
        modifiers: ToolModifiers::NONE,
    }))?;
    assert_eq!(session.reconstruction()?, finished);
    assert_eq!(session.detail_view(actual, (1, 1, 0), 0, finished)?, actual);
    assert!(!actual.shares_sampling_grid(retained.viewport()));
    Ok(())
}

#[test]
fn pan_detail_grid_requires_the_same_document_source_and_finished_quality()
-> Result<(), Box<dyn Error>> {
    let (mut session, clock, _directory) = session()?;
    session.input(Input::Tool(ToolInput::OneToOne))?;
    clock.0.store(80_000_000, Ordering::SeqCst);
    session.poll(Duration::ZERO)?;
    let retained = coverage::detail(session.document.tools.navigation().viewport())?;
    let finished = Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3);
    session.document.presented = Some(Presented {
        tier: BackingTier::Detail,
        coverage: retained,
        view: retained.viewport(),
        sampling_scale: retained.viewport().zoom(),
        reconstruction: finished,
        document: (1, 1, 0),
        source_revision: 7,
        comparison: None,
    });
    session.input(Input::Tool(ToolInput::Button {
        button: ToolButton::Primary,
        pressed: true,
        modifiers: ToolModifiers::NONE,
    }))?;
    session.input(Input::Tool(ToolInput::Pointer(PointerSample::mouse(
        [0.5, -0.25],
        ToolModifiers::NONE,
    ))))?;
    let actual = session.document.tools.navigation().viewport();
    assert!(!actual.shares_sampling_grid(retained.viewport()));
    assert!(
        session
            .detail_view(actual, (1, 1, 0), 7, finished)?
            .shares_sampling_grid(retained.viewport())
    );
    for (document, source, quality) in [
        ((2, 1, 0), 7, finished),
        ((1, 2, 0), 7, finished),
        ((1, 1, 0), 8, finished),
        ((1, 1, 0), 7, Reconstruction::Preview),
    ] {
        assert_eq!(session.detail_view(actual, document, source, quality)?, actual);
    }
    Ok(())
}

#[test]
fn finished_reconstruction_threshold_does_not_turn_nearest_into_a_preview_loop()
-> Result<(), Box<dyn Error>> {
    let (mut session, _, _directory) = session()?;
    let mut exact = session.document.tools.navigation().viewport();
    exact.reframe(exact.origin(), 2.0)?;
    session.document.tools = ToolManager::new(Navigation::new(exact));
    assert_eq!(
        session.reconstruction()?,
        Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3)
    );

    let above = f64::from_bits(2.0_f64.to_bits() + 1);
    let mut magnified = exact;
    magnified.reframe(magnified.origin(), above)?;
    session.document.tools = ToolManager::new(Navigation::new(magnified));
    let finished = Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Nearest);
    assert_eq!(session.reconstruction()?, finished);
    let retained = coverage::detail(magnified)?;
    session.document.presented = Some(Presented {
        tier: BackingTier::Detail,
        coverage: retained,
        view: retained.viewport(),
        sampling_scale: retained.viewport().zoom(),
        reconstruction: finished,
        document: (1, 1, 0),
        source_revision: 0,
        comparison: None,
    });
    assert_eq!(session.reconstruction()?, finished);
    session.input(Input::Tool(ToolInput::Button {
        button: ToolButton::Primary,
        pressed: true,
        modifiers: ToolModifiers::NONE,
    }))?;
    assert_eq!(session.reconstruction()?, finished);
    assert!(!finished.is_preview());
    Ok(())
}

#[test]
fn empty_viewer_releases_picker_before_any_frame_or_gpu_completion() -> Result<(), Box<dyn Error>> {
    let (mut session, _, _directory) = session()?;
    session.config.gpu =
        Some(crate::native_canvas::gpu_policy::GpuRequest::Required("vulkan:8086:a780".parse()?));
    session.device_pending = true;
    session.gpu_ready = false;
    let wake = Arc::new(refinement::Wake::default());
    session.start(wake.clone())?;
    while session.resources.is_none() {
        let observed = wake.revision();
        session.poll(Duration::ZERO)?;
        if session.resources.is_none() {
            wake.wait_after(observed);
        }
    }
    assert!(session.document.front.is_none());
    assert!(session.document.fallback_front.is_none());
    assert!(session.document.gpu_front.is_none());
    assert!(session.document.fallback_gpu_front.is_none());
    assert!(session.document.navigation_front.is_none());
    assert!(session.document.navigation_gpu_front.is_none());
    assert!(session.device_job.is_none());
    assert!(session.device_pending);
    assert!(session.take_picker_request());
    assert!(!session.take_picker_request());
    session.close()?;
    while !session.shutdown_started {
        let observed = wake.revision();
        session.poll(Duration::ZERO)?;
        if !session.shutdown_started {
            wake.wait_after(observed);
        }
    }
    drop(session.runtime.take().ok_or("Execution missing")?);
    session.finish()
}
#[test]
fn invalid_native_input_preserves_camera_and_closed_lifetimes_ignore_new_intent()
-> Result<(), Box<dyn Error>> {
    let (mut session, _, _directory) = session()?;
    let previous = session.document.tools.navigation().viewport();
    assert!(
        session
            .input(Input::Tool(ToolInput::Wheel { steps: f64::NAN, phase: ScrollPhase::Unphased }))
            .is_err()
    );
    assert!(
        session
            .input(Input::Tool(ToolInput::Pointer(PointerSample::mouse(
                [f64::INFINITY, 2.0],
                ToolModifiers::NONE
            ))))
            .is_err()
    );
    assert_eq!(session.document.tools.navigation().viewport(), previous);
    assert!(!session.is_closing());
    session.close()?;
    assert!(session.is_closing());
    session.input(Input::Tool(ToolInput::Wheel { steps: 4.0, phase: ScrollPhase::Unphased }))?;
    assert_eq!(session.document.tools.navigation().viewport(), previous);
    Ok(())
}

/// The display's refresh rate belongs to the session: a document replaced after it was
/// reported keeps navigation and presentation paced to the display, not the 60 Hz fallback.
#[test]
fn the_display_refresh_rate_paces_every_later_document() -> Result<(), Box<dyn Error>> {
    let (mut session, _clock, _directory) = session()?;
    session.input(Input::DisplayRefresh(Some(144.0)))?;
    let period = Duration::try_from_secs_f64(1.0 / 144.0)?;
    assert_eq!(session.document.tools.navigation().frame_period(), period);
    assert_eq!(session.pacing.period(), period);
    drop(session.withdraw_document()?);
    assert_eq!(session.document.tools.navigation().frame_period(), period);
    assert_eq!(session.pacing.period(), period);
    Ok(())
}
