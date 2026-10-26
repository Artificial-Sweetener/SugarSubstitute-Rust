// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify native navigation intent and exact preview/refinement decisions with an injected clock.
use super::*;
mod display;
mod latency;
mod refinement;
mod source_io;
use std::sync::atomic::{AtomicU64, Ordering};
use sugarsubstitute_canvas::{ScrollPhase, ToolButton, ToolInput};
use sugarsubstitute_desktop::canvas::{Input, Lifetime};
use sugarsubstitute_image_ops::Extent;

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
    let directory = tempfile::tempdir()?;
    let options = Options::parse([
        "--cache".into(),
        directory.path().as_os_str().to_owned(),
        "--width".into(),
        "320".into(),
        "--height".into(),
        "240".into(),
    ])?;
    let clock = Arc::new(TestClock(AtomicU64::new(0)));
    let mut session = Session::new(options, DiagnosticHealth::fixture())?;
    session.clock = clock.clone();
    session.tools.set_content(Some(Extent::new(640, 480)?), None, Duration::ZERO)?;
    Ok((session, clock, directory))
}
#[test]
fn normalized_tool_zoom_uses_the_execution_clock_and_refines_after_wheel_idle()
-> Result<(), Box<dyn Error>> {
    let (mut session, clock, _directory) = session()?;
    let pointer = [71.25, 53.5];
    session.input(Input::Tool(ToolInput::Pointer(pointer)))?;
    let anchor = session.tools.navigation().viewport().document_at(pointer)?;
    session.input(Input::Tool(ToolInput::Wheel { steps: 1.0, phase: ScrollPhase::Unphased }))?;
    assert_eq!(session.tools.navigation().target_viewport().zoom(), 0.625);
    assert_eq!(session.reconstruction()?, Reconstruction::Preview);
    assert!(session.next_deadline().is_some());
    clock.0.store(40_000_000, Ordering::SeqCst);
    session.poll(Duration::ZERO)?;
    let view = session.tools.navigation().viewport();
    assert_eq!(view.zoom(), 0.59375);
    assert_eq!(view.document_at(pointer)?, anchor);
    clock.0.store(119_999_999, Ordering::SeqCst);
    session.poll(Duration::ZERO)?;
    assert_eq!(session.tools.navigation().viewport().zoom(), 0.625);
    assert_eq!(session.reconstruction()?, Reconstruction::Preview);
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
    assert_eq!(session.tools.navigation().viewport().zoom(), 0.625 * 1.25_f64.sqrt());
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
    session.input(Input::Tool(ToolInput::Pointer([20.0, 30.0])))?;
    session.input(Input::Tool(ToolInput::Button { button: ToolButton::Primary, pressed: true }))?;
    session.input(Input::Tool(ToolInput::Pointer([27.0, 19.0])))?;
    assert_eq!(session.tools.navigation().viewport().origin(), [-7.0, 11.0]);
    assert!(session.feedback().capture);
    assert_eq!(
        session.reconstruction()?,
        Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3)
    );
    assert!(session.next_deadline().is_none());
    session.input(Input::Unfocused)?;
    session.input(Input::Tool(ToolInput::Pointer([100.0, 100.0])))?;
    assert_eq!(session.tools.navigation().viewport().origin(), [-7.0, 11.0]);
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
    let target = session.tools.navigation().viewport();
    let retained = coverage::detail(target)?;
    let finished = Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3);
    session.presented = Some(Presented {
        tier: BackingTier::Detail,
        coverage: retained,
        view: retained.viewport(),
        reconstruction: finished,
        document: (1, 1),
        source_revision: 0,
    });

    session.input(Input::Tool(ToolInput::Pointer([20.0, 30.0])))?;
    session.input(Input::Tool(ToolInput::Button { button: ToolButton::Primary, pressed: true }))?;
    session.input(Input::Tool(ToolInput::Pointer([27.0, 19.0])))?;
    assert_eq!(session.reconstruction()?, finished);
    session.input(Input::Tool(ToolInput::Pointer([27.5, 19.25])))?;
    assert_eq!(session.reconstruction()?, finished);
    let actual = session.tools.navigation().viewport();
    let repair_view = session.detail_view(actual, (1, 1), 0, finished)?;
    assert!(repair_view.shares_sampling_grid(retained.viewport()));
    assert_eq!(session.tools.navigation().viewport(), actual);
    let repair = coverage::detail(repair_view)?.viewport().repair_from(retained.viewport())?;
    assert!(repair.blit().is_some());
    assert!(repair.dirty().pixels() < repair.dirty().surface().pixels());
    session
        .input(Input::Tool(ToolInput::Button { button: ToolButton::Primary, pressed: false }))?;
    assert_eq!(session.reconstruction()?, finished);
    assert_eq!(session.detail_view(actual, (1, 1), 0, finished)?, actual);
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
    let retained = coverage::detail(session.tools.navigation().viewport())?;
    let finished = Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3);
    session.presented = Some(Presented {
        tier: BackingTier::Detail,
        coverage: retained,
        view: retained.viewport(),
        reconstruction: finished,
        document: (1, 1),
        source_revision: 7,
    });
    session.input(Input::Tool(ToolInput::Button { button: ToolButton::Primary, pressed: true }))?;
    session.input(Input::Tool(ToolInput::Pointer([0.5, -0.25])))?;
    let actual = session.tools.navigation().viewport();
    assert!(!actual.shares_sampling_grid(retained.viewport()));
    assert!(
        session.detail_view(actual, (1, 1), 7, finished)?.shares_sampling_grid(retained.viewport())
    );
    for (document, source, quality) in [
        ((2, 1), 7, finished),
        ((1, 2), 7, finished),
        ((1, 1), 8, finished),
        ((1, 1), 7, Reconstruction::Preview),
    ] {
        assert_eq!(session.detail_view(actual, document, source, quality)?, actual);
    }
    Ok(())
}

#[test]
fn finished_reconstruction_threshold_does_not_turn_nearest_into_a_preview_loop()
-> Result<(), Box<dyn Error>> {
    let (mut session, _, _directory) = session()?;
    let mut exact = session.tools.navigation().viewport();
    exact.reframe(exact.origin(), 2.0)?;
    session.tools = ToolManager::new(Navigation::new(exact));
    assert_eq!(
        session.reconstruction()?,
        Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3)
    );

    let above = f64::from_bits(2.0_f64.to_bits() + 1);
    let mut magnified = exact;
    magnified.reframe(magnified.origin(), above)?;
    session.tools = ToolManager::new(Navigation::new(magnified));
    let finished = Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Nearest);
    assert_eq!(session.reconstruction()?, finished);
    let retained = coverage::detail(magnified)?;
    session.presented = Some(Presented {
        tier: BackingTier::Detail,
        coverage: retained,
        view: retained.viewport(),
        reconstruction: finished,
        document: (1, 1),
        source_revision: 0,
    });
    assert_eq!(session.reconstruction()?, finished);
    session.input(Input::Tool(ToolInput::Button { button: ToolButton::Primary, pressed: true }))?;
    assert_eq!(session.reconstruction()?, finished);
    assert!(!finished.is_preview());
    Ok(())
}

#[test]
fn empty_viewer_releases_picker_before_any_frame_or_gpu_completion() -> Result<(), Box<dyn Error>> {
    let (mut session, _, _directory) = session()?;
    session.options.gpu = Some("vulkan:8086:a780".parse()?);
    session.device_pending = true;
    session.gpu_ready = false;
    let wake = Arc::new(refinement::Wake::default());
    session.start(wake.clone())?;
    while session.resources.is_none() {
        let observed = wake.revision();
        session.poll(Duration::ZERO)?;
        if session.resources.is_none() {
            wake.wait_after(observed)?;
        }
    }
    assert!(session.front.is_none());
    assert!(session.fallback_front.is_none());
    assert!(session.gpu_front.is_none());
    assert!(session.fallback_gpu_front.is_none());
    assert!(session.device_job.is_none());
    assert!(session.device_pending);
    assert!(session.take_picker_request());
    assert!(!session.take_picker_request());
    session.close()?;
    while !session.shutdown_started {
        let observed = wake.revision();
        session.poll(Duration::ZERO)?;
        if !session.shutdown_started {
            wake.wait_after(observed)?;
        }
    }
    let Runtime { owner, client, scope } = session.runtime.take().ok_or("Execution missing")?;
    drop((client, scope));
    drop(owner);
    session.finish()
}
#[test]
fn invalid_native_input_preserves_camera_and_closed_lifetimes_ignore_new_intent()
-> Result<(), Box<dyn Error>> {
    let (mut session, _, _directory) = session()?;
    let previous = session.tools.navigation().viewport();
    assert!(
        session
            .input(Input::Tool(ToolInput::Wheel { steps: f64::NAN, phase: ScrollPhase::Unphased }))
            .is_err()
    );
    assert!(session.input(Input::Tool(ToolInput::Pointer([f64::INFINITY, 2.0]))).is_err());
    assert_eq!(session.tools.navigation().viewport(), previous);
    assert!(!session.is_closing());
    session.close()?;
    assert!(session.is_closing());
    session.input(Input::Tool(ToolInput::Wheel { steps: 4.0, phase: ScrollPhase::Unphased }))?;
    assert_eq!(session.tools.navigation().viewport(), previous);
    Ok(())
}
