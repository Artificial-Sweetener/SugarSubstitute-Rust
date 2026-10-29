// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify semantic camera completion with real CPU preparation and native acknowledgement.

use super::*;
use sugarsubstitute_canvas::NavigationCommand;

impl Fixture {
    /// Prepare and acknowledge a real encoded source through the embedded CPU execution path.
    fn camera_source(&mut self) -> Result<AcceptedCamera, Box<dyn Error>> {
        let mut source = Some(self.encoded([255, 0, 0])?);
        assert!(self.session.offer_source(&mut source, CacheReuse::Compatible, None)?);
        self.session.resize(Extent::new(8, 8)?)?;
        self.session.input(Input::Drawable(true))?;
        self.clock.0.store(1_000_000_000, Ordering::SeqCst);
        let (target, _, _) = self.pending_frame()?;
        assert!(self.session.accepted_camera().is_none());
        self.session.did_present_cpu(target)?;
        self.session.accepted_camera().ok_or("Accepted camera evidence absent".into())
    }

    /// Observe actual prepared frames until native acceptance proves the final destination.
    fn accept_camera(&mut self, intent: CameraIntent) -> Result<AcceptedCamera, Box<dyn Error>> {
        loop {
            match self.session.settled_camera(intent) {
                CameraSettlement::Settled(camera) => return Ok(camera),
                CameraSettlement::Superseded => return Err("Camera destination superseded".into()),
                CameraSettlement::Failed => return Err("Camera preparation failed".into()),
                CameraSettlement::Pending => {}
            }
            self.clock.0.fetch_add(100_000_000, Ordering::SeqCst);
            let observed = self.wake.revision();
            self.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
            self.session.poll(Duration::ZERO)?;
            match self.session.settled_camera(intent) {
                CameraSettlement::Settled(camera) => return Ok(camera),
                CameraSettlement::Superseded => return Err("Camera destination superseded".into()),
                CameraSettlement::Failed => return Err("Camera preparation failed".into()),
                CameraSettlement::Pending => {}
            }
            if let Some(target) =
                self.session.frame().filter(|frame| frame.pending).map(|frame| frame.target)
            {
                self.session.did_present_cpu(target)?;
            } else {
                self.wake.wait_after(observed)?;
            }
        }
    }
}

fn admitted(result: CameraCommandResult) -> Result<CameraIntent, Box<dyn Error>> {
    match result {
        CameraCommandResult::Accepted(intent) => Ok(intent),
        other => Err(format!("Camera command was not admitted: {other:?}").into()),
    }
}

#[test]
fn unchanged_fit_resolves_the_existing_accepted_frame_and_empty_content_is_explicit()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    assert_eq!(fixture.session.navigate(NavigationCommand::Fit)?, CameraCommandResult::NoContent);
    let original = fixture.camera_source()?;
    let intent = admitted(fixture.session.navigate(NavigationCommand::Fit)?)?;
    assert_eq!(fixture.session.settled_camera(intent), CameraSettlement::Settled(original));
    assert_eq!(original.viewport.zoom(), 8.0);
    assert_eq!(original.extent, Extent::new(1, 1)?);
    assert_eq!(fixture.session.document.native_presented, Some(original.frame));
    assert!(fixture.session.document.cpu_pending.is_none());
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn camera_state_keeps_actual_acceptance_through_motion_and_unaccepted_preparation()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    let original = fixture.camera_source()?;
    let intent = admitted(fixture.session.navigate(NavigationCommand::OneToOne)?)?;
    assert_eq!(fixture.session.settled_camera(intent), CameraSettlement::Pending);
    assert_eq!(fixture.session.accepted_camera(), Some(original));
    fixture.clock.0.store(2_000_000_000, Ordering::SeqCst);
    let (prepared, _, _) = fixture.pending_frame()?;
    assert_ne!(prepared, original.frame);
    assert_eq!(fixture.session.accepted_camera(), Some(original));
    assert_eq!(fixture.session.settled_camera(intent), CameraSettlement::Pending);
    fixture.session.did_present_cpu(prepared)?;
    let final_camera = fixture.accept_camera(intent)?;
    assert_eq!(final_camera.viewport.zoom(), 1.0);
    assert_eq!(final_camera.viewport.origin(), [-3.5; 2]);
    assert_eq!(final_camera.document, original.document);
    assert_eq!(final_camera.revision, original.revision);
    assert_eq!(final_camera.source_revision, original.source_revision);
    assert_ne!(final_camera.frame, original.frame);
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn newer_camera_resize_and_source_retirement_fence_waiters_without_releasing_pixels()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    let original = fixture.camera_source()?;
    let abandoned = admitted(fixture.session.navigate(NavigationCommand::OneToOne)?)?;
    let current = admitted(fixture.session.navigate(NavigationCommand::Fit)?)?;
    assert_eq!(fixture.session.settled_camera(abandoned), CameraSettlement::Superseded);
    assert_eq!(fixture.session.settled_camera(current), CameraSettlement::Settled(original));
    assert_eq!(fixture.session.accepted_camera(), Some(original));
    let pixels = fixture.session.frame().ok_or("Accepted pixels absent")?.pixels.as_ptr();
    fixture.session.resize(Extent::new(10, 8)?)?;
    assert_eq!(fixture.session.settled_camera(current), CameraSettlement::Superseded);
    assert_eq!(fixture.session.accepted_camera(), Some(original));
    assert_eq!(fixture.session.frame().ok_or("Retained pixels absent")?.pixels.as_ptr(), pixels);
    let resized = admitted(fixture.session.navigate(NavigationCommand::Fit)?)?;
    fixture.session.cancel_selection()?;
    assert_eq!(fixture.session.settled_camera(resized), CameraSettlement::Superseded);
    assert!(fixture.session.accepted_camera().is_none());
    assert_eq!(
        fixture.session.frame().ok_or("Cancelled source pixels lost")?.pixels.as_ptr(),
        pixels
    );
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn cancellation_stops_only_matching_motion_and_keeps_actual_accepted_pixels()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    let original = fixture.camera_source()?;
    let first = admitted(fixture.session.navigate(NavigationCommand::OneToOne)?)?;
    fixture.clock.0.store(1_040_000_000, Ordering::SeqCst);
    assert!(fixture.session.cancel_camera(first)?);
    let stopped = fixture.session.document.tools.navigation().viewport();
    assert!(stopped.zoom() > 1.0 && stopped.zoom() < 8.0);
    assert_eq!(
        fixture.session.document.tools.navigation().zoom_mode(),
        sugarsubstitute_canvas::ZoomMode::Custom
    );
    assert!(!fixture.session.document.tools.navigation().interacting());
    assert_eq!(fixture.session.accepted_camera(), Some(original));
    assert_eq!(fixture.session.settled_camera(first), CameraSettlement::Superseded);
    let current = admitted(fixture.session.navigate(NavigationCommand::Fit)?)?;
    assert!(!fixture.session.cancel_camera(first)?);
    assert_eq!(fixture.session.camera_intent(), CameraCommandResult::Accepted(current));
    fixture.clock.0.store(2_000_000_000, Ordering::SeqCst);
    let settled = fixture.accept_camera(current)?;
    assert_eq!(settled.viewport, original.viewport);
    assert_eq!(settled.document, original.document);
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn current_camera_failure_is_terminal_and_retains_its_original_native_boundary_cause()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    let original = fixture.camera_source()?;
    let intent = admitted(fixture.session.navigate(NavigationCommand::OneToOne)?)?;
    fixture.session.selected(Err(Box::new(std::io::Error::from_raw_os_error(87))))?;
    assert_eq!(fixture.session.settled_camera(intent), CameraSettlement::Failed);
    assert!(fixture.session.source_failed());
    let cause = fixture.session.failed.as_ref().ok_or("Original native failure was lost")?;
    assert_eq!(
        cause.downcast_ref::<std::io::Error>().and_then(std::io::Error::raw_os_error),
        Some(87)
    );
    assert_eq!(fixture.session.document.native_presented, Some(original.frame));
    assert!(fixture.session.accepted_camera().is_none());
    fixture.session.cancel_selection()?;
    assert_eq!(fixture.session.settled_camera(intent), CameraSettlement::Superseded);
    assert!(fixture.session.failed.is_some());
    let mut recovery = Some(fixture.encoded([0, 0, 255])?);
    assert!(fixture.session.offer_source(&mut recovery, CacheReuse::Compatible, None)?);
    let _directory = fixture.finish()?;
    Ok(())
}
