// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify native navigation intent and exact preview/refinement decisions with an injected clock.
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
use sugarsubstitute_desktop::canvas::{Input, Lifetime, WheelDelta};

struct TestClock(AtomicU64);
impl Clock for TestClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::SeqCst))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
fn session() -> Result<(Session, Arc<TestClock>), Box<dyn Error>> {
    let options = Options::parse([
        "--cache".into(),
        "E:\\viewer-navigation-fixture".into(),
        "--width".into(),
        "320".into(),
        "--height".into(),
        "240".into(),
    ])?;
    let clock = Arc::new(TestClock(AtomicU64::new(0)));
    let mut session = Session::new(options, DiagnosticHealth::fixture())?;
    session.clock = clock.clone();
    Ok((session, clock))
}
#[test]
fn cursor_zoom_previews_then_refines_at_the_original_quiet_cutoff() -> Result<(), Box<dyn Error>> {
    let (mut session, clock) = session()?;
    let pointer = [71.25, 53.5];
    session.input(Input::Pointer(pointer))?;
    let anchor = session.view.document_at(pointer)?;
    session.input(Input::Wheel(WheelDelta::Lines([0.0, 4.0])))?;
    assert_eq!(session.view.zoom(), 2.0);
    assert_eq!(session.view.document_at(pointer)?, anchor);
    assert_eq!(session.filter(), Filter::Nearest);
    assert!(session.next_deadline().is_some());
    clock.0.store(119_999_999, Ordering::SeqCst);
    assert_eq!(session.filter(), Filter::Nearest);
    clock.0.store(120_000_000, Ordering::SeqCst);
    assert_eq!(session.filter(), Filter::Lanczos3);
    assert!(session.next_deadline().is_none());
    session.input(Input::Wheel(WheelDelta::Pixels([0.0, 480.0])))?;
    assert_eq!(session.view.zoom(), 4.0);
    assert_eq!(session.view.document_at(pointer)?, anchor);
    assert_eq!(session.navigation_until, Some(Moment::from_nanos(240_000_000)));
    Ok(())
}
#[test]
fn panning_keeps_finished_quality_and_focus_loss_ends_the_drag() -> Result<(), Box<dyn Error>> {
    let (mut session, _) = session()?;
    session.input(Input::Pointer([20.0, 30.0]))?;
    session.input(Input::Pan(true))?;
    session.input(Input::Pointer([27.0, 19.0]))?;
    assert_eq!(session.view.origin(), [-7.0, 11.0]);
    assert_eq!(session.filter(), Filter::Lanczos3);
    assert_eq!(session.navigation_until, None);
    session.input(Input::Unfocused)?;
    session.input(Input::Pointer([100.0, 100.0]))?;
    assert_eq!(session.view.origin(), [-7.0, 11.0]);
    assert!(!session.take_picker_request());
    Ok(())
}
#[test]
fn invalid_native_input_preserves_camera_and_closed_lifetimes_ignore_new_intent()
-> Result<(), Box<dyn Error>> {
    let (mut session, _) = session()?;
    let previous = session.view;
    assert!(session.input(Input::Wheel(WheelDelta::Lines([0.0, f64::NAN]))).is_err());
    assert!(session.input(Input::Pointer([f64::INFINITY, 2.0])).is_err());
    assert_eq!(session.view, previous);
    assert!(!session.is_closing());
    session.close()?;
    assert!(session.is_closing());
    session.input(Input::Wheel(WheelDelta::Lines([0.0, 4.0])))?;
    assert_eq!(session.view, previous);
    Ok(())
}
