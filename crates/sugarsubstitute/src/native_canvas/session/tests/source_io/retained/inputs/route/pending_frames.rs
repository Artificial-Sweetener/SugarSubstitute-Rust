// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise pending native frames and stale display storage across Input/Output route changes.

use super::*;

impl Fixture {
    /// Move injected time only to the presentation owner's actual next eligible slot.
    fn pending_route_cadence(&mut self) -> Result<(), Box<dyn Error>> {
        let now = self.session.navigation_time();
        if let PresentationReadiness::WaitingUntil(deadline) = self.session.pacing.readiness(now)? {
            let remaining =
                u64::try_from(deadline.checked_sub(now).ok_or("Cadence regressed")?.as_nanos())?;
            self.clock.0.fetch_add(remaining, Ordering::SeqCst);
        }
        Ok(())
    }

    /// Follow actual worker wakes and the existing presentation cadence without acknowledging pixels.
    fn pending_route_step(&mut self) -> Result<(), Box<dyn Error>> {
        self.pending_route_cadence()?;
        let observed = self.wake.revision();
        self.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        self.session.poll(Duration::ZERO)?;
        if let Some(cause) = &self.session.failed {
            return Err(format!("Pending route failed: {cause}").into());
        }
        let scope = self.session.runtime.as_ref().ok_or("Runtime absent")?.scope.snapshot();
        if scope.running + scope.queued > 0 {
            self.wake.wait_after(observed).map_err(|cause| {
                format!(
                    "Pending route worker snapshot={:?}: {cause}",
                    self.session.native_preparation_snapshot()
                )
            })?;
        }
        Ok(())
    }

    /// Finish acquisition at its own boundary without waiting for a deliberately unaccepted frame.
    fn pending_route_source(&mut self) -> Result<(), Box<dyn Error>> {
        for _ in 0..500 {
            self.pending_route_step()?;
            if !self.session.source_loading() {
                return Ok(());
            }
        }
        Err(format!(
            "The route source did not settle: {:?}",
            self.session.native_preparation_snapshot()
        )
        .into())
    }

    /// A route may settle while its newly prepared frame legitimately awaits native acceptance.
    fn pending_route(&mut self, document: Option<InputDocumentId>) -> Result<(), Box<dyn Error>> {
        for _ in 0..500 {
            self.pending_route_step()?;
            if !self.session.input_route_pending() && self.session.document.input == document {
                return Ok(());
            }
        }
        Err(format!(
            "The requested document route did not settle: {:?}",
            self.session.native_preparation_snapshot()
        )
        .into())
    }

    /// Await only the current selected frame, retaining its ACK for the test's next transition.
    fn pending_route_frame(&mut self) -> Result<(FrameStamp, Extent, u32), Box<dyn Error>> {
        for _ in 0..500 {
            self.pending_route_step()?;
            if let Some((frame, _)) =
                self.session.selected_cpu_frame().filter(|(frame, _)| frame.pending)
            {
                let extent = frame.damage.dirty().surface();
                let center = u64::from(extent.height() / 2) * u64::from(extent.width())
                    + u64::from(extent.width() / 2);
                let pixel = *frame.pixels.get(usize::try_from(center)?).ok_or("Center absent")?;
                return Ok((frame.target, extent, pixel));
            }
        }
        Err(format!(
            "The current route did not offer a selected CPU frame: {:?}",
            self.session.native_preparation_snapshot()
        )
        .into())
    }
}

#[test]
fn route_swaps_with_unacknowledged_input_frames_keep_output_presentation_live()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    {
        fixture.session.resize(Extent::new(8, 8)?)?;
        let directory = fixture._directory.path().to_path_buf();
        let blue = image(&directory, "blue.png", Extent::new(4, 3)?, |_, _| [0, 0, 255, 255])?;
        let reply = fixture.sync(vec![
            red_surface(&directory)?,
            surface("blue", SurfaceBase::File(file("input/blue.png", &blue).0), Vec::new()),
        ])?;
        let first = synced(&reply, "image")?.document;
        let second = synced(&reply, "blue")?.document;
        fixture.session.activate_input(WORKFLOW, "image", None)?;
        fixture.settle_inputs()?;
        fixture.session.input(Input::Drawable(true))?;
        let (first_pending, _, _) = fixture.pending_route_frame()?;
        assert_eq!(fixture.session.document.input, Some(first));
        assert!(matches!(
            fixture.session.pacing.readiness(fixture.session.navigation_time())?,
            PresentationReadiness::Busy { target, .. } if target == first_pending
        ));

        // A generated final arrives while the Input frame still awaits its native ACK.
        let mut output = Some(fixture.encoded([0, 255, 0])?);
        assert!(fixture.session.offer_source(&mut output, CacheReuse::Compatible, None)?);
        assert!(fixture.session.did_present_cpu(first_pending).is_err());
        fixture.pending_route_source()?;
        let output_asset = parked_asset(&fixture.session).ok_or("The final was not parked")?;
        let (rebound_pending, _, _) = fixture.pending_route_frame()?;
        assert_eq!(fixture.session.document.input, Some(first));
        assert_ne!(rebound_pending, first_pending);

        // Switch Input documents without acknowledging the replacement Input frame either.
        fixture.session.activate_input(WORKFLOW, "blue", None)?;
        fixture.pending_route(Some(second))?;
        assert_eq!(fixture.session.document.input, Some(second));
        assert!(fixture.session.did_present_cpu(rebound_pending).is_err());
        let (second_pending, _, _) = fixture.pending_route_frame()?;
        assert!(matches!(
            fixture.session.pacing.readiness(fixture.session.navigation_time())?,
            PresentationReadiness::Busy { target, .. } if target == second_pending
        ));

        assert!(fixture.session.show_output());
        fixture.pending_route(None)?;
        assert!(!fixture.session.input_shown());
        assert_eq!(shown_asset(&fixture.session), Some(output_asset));
        let (output_pending, _, pixel) = fixture.pending_route_frame()?;
        assert!((pixel >> 16) & 0xff < 16 && (pixel >> 8) & 0xff > 224 && pixel & 0xff < 16);
        assert!(matches!(
            fixture.session.pacing.readiness(fixture.session.navigation_time())?,
            PresentationReadiness::Busy { target, .. } if target == output_pending
        ));
        for obsolete in [first_pending, rebound_pending, second_pending] {
            assert_ne!(output_pending, obsolete);
            assert!(fixture.session.did_present_cpu(obsolete).is_err());
            assert_eq!(
                fixture.session.selected_cpu_frame().ok_or("Current final frame absent")?.0.target,
                output_pending
            );
        }
        fixture.session.did_present_cpu(output_pending)?;
        let frame = fixture.session.selected_cpu_frame().ok_or("Accepted final frame absent")?.0;
        assert_eq!(frame.target, output_pending);
        assert!(!frame.pending);
    }
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn stale_completed_input_draw_keeps_storage_for_the_next_output() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    {
        fixture.session.resize(Extent::new(8, 8)?)?;
        let directory = fixture._directory.path().to_path_buf();
        fixture.sync(vec![red_surface(&directory)?])?;
        fixture.session.activate_input(WORKFLOW, "image", None)?;
        fixture.settle_inputs()?;
        fixture.session.input(Input::Drawable(true))?;
        let (accepted, _, _) = fixture.pending_route_frame()?;
        fixture.session.did_present_cpu(accepted)?;

        fixture.pending_route_cadence()?;
        fixture.session.input(Input::Drawable(true))?;
        fixture.session.set_backdrops(vec![sugarsubstitute_canvas_render::BackdropRegion::new(
            [0.0, 0.0, 8.0, 4.0],
            1.0,
            1.0,
        )?])?;
        fixture.session.submit_display()?;
        assert!(fixture.session.displaying.is_some(), "a real display job owns the spare");
        assert!(fixture.session.document.display_buffers.iter().all(Option::is_none));
        assert!(
            !fixture.session.needs_display_buffers(Extent::new(8, 8)?),
            "an in-flight display borrow must not allocate another pair"
        );
        loop {
            let observed = fixture.wake.revision();
            fixture.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
            if fixture
                .session
                .displaying
                .as_ref()
                .is_some_and(|display| matches!(display.job.status().phase, JobPhase::Terminal(_)))
            {
                break;
            }
            fixture.wake.wait_after(observed)?;
        }

        // The worker really finished, but source replacement precedes coordinator publication.
        let mut output = Some(fixture.encoded([0, 255, 0])?);
        assert!(fixture.session.offer_source(&mut output, CacheReuse::Compatible, None)?);
        fixture.session.consume_display()?;
        assert!(fixture.session.displaying.is_none());
        assert!(fixture.session.document.cpu_projection.is_some(), "accepted pixels stay owned");
        assert!(
            fixture.session.document.display_buffers.iter().any(Option::is_some)
                || fixture.session.needs_display_buffers(Extent::new(8, 8)?),
            "a stale completed draw must return its storage or request replacement buffers"
        );

        fixture.pending_route_source()?;
        assert!(fixture.session.show_output());
        fixture.pending_route(None)?;
        let (target, _, pixel) = fixture.pending_route_frame()?;
        assert!((pixel >> 16) & 0xff < 16 && (pixel >> 8) & 0xff > 224 && pixel & 0xff < 16);
        assert_eq!(
            fixture.session.selected_cpu_frame().ok_or("New output frame absent")?.0.target,
            target
        );
        fixture.session.did_present_cpu(target)?;
    }
    let _directory = fixture.finish()?;
    Ok(())
}
