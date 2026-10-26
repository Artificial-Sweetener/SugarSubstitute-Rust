// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify native navigation intent and exact preview/refinement decisions with an injected clock.
use super::*;
mod display;
mod refinement;
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
    assert_eq!(session.filter(), Filter::Nearest);
    assert!(session.next_deadline().is_some());
    clock.0.store(40_000_000, Ordering::SeqCst);
    session.poll(Duration::ZERO)?;
    let view = session.tools.navigation().viewport();
    assert_eq!(view.zoom(), 0.59375);
    assert_eq!(view.document_at(pointer)?, anchor);
    clock.0.store(119_999_999, Ordering::SeqCst);
    session.poll(Duration::ZERO)?;
    assert_eq!(session.tools.navigation().viewport().zoom(), 0.625);
    assert_eq!(session.filter(), Filter::Nearest);
    clock.0.store(120_000_000, Ordering::SeqCst);
    session.poll(Duration::ZERO)?;
    assert_eq!(session.filter(), Filter::Lanczos3);
    assert!(session.next_deadline().is_none());
    session.input(Input::Tool(ToolInput::Wheel { steps: 0.5, phase: ScrollPhase::End }))?;
    clock.0.store(180_000_000, Ordering::SeqCst);
    session.poll(Duration::ZERO)?;
    assert_eq!(session.tools.navigation().viewport().zoom(), 0.625 * 1.25_f64.sqrt());
    assert_eq!(session.filter(), Filter::Lanczos3);
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
    assert_eq!(session.filter(), Filter::Lanczos3);
    assert!(session.next_deadline().is_none());
    session.input(Input::Unfocused)?;
    session.input(Input::Tool(ToolInput::Pointer([100.0, 100.0])))?;
    assert_eq!(session.tools.navigation().viewport().origin(), [-7.0, 11.0]);
    assert!(!session.feedback().capture);
    assert!(!session.take_picker_request());
    Ok(())
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
