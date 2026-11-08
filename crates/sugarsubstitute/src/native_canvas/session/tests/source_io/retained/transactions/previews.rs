// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise preview publication, locked Fit resizing and complete final restoration on real workers.

use super::*;
use sugarsubstitute_canvas::{NavigationCommand, ZoomMode};
mod backdrops;
mod deadlines;
mod large;
mod wakes;

impl Fixture {
    /// Offer actual encoded storage while retaining the current final's document and presentation.
    fn preview(&mut self, color: [u8; 3]) -> Result<PreviewToken, Box<dyn Error>> {
        let mut input = Some(self.encoded(color)?);
        let deadline = self.session.preview_now().checked_add(Duration::from_secs(30))?;
        self.session
            .offer_preview(&mut input, deadline)?
            .token()
            .ok_or("Preview admission refused".into())
    }

    /// Advance injected time only to exercise the existing display cadence, observing real worker wakes.
    fn preview_step(&mut self) -> Result<(), Box<dyn Error>> {
        self.clock.0.fetch_add(20_000_000, Ordering::SeqCst);
        let observed = self.wake.revision();
        self.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        self.session.poll(Duration::ZERO)?;
        let snapshot = self.session.runtime.as_ref().ok_or("Runtime absent")?.scope.snapshot();
        if snapshot.running + snapshot.queued > 0 && snapshot.terminal_unobserved == 0 {
            self.wake.wait_after(observed).map_err(|cause| {
                let state = self.session.runtime.as_ref().map(|runtime| runtime.scope.snapshot());
                format!("Preview progress scope={state:?}, load={}, render={}, display={}, retirement={}, queued_retirement={}, deferred_retirement={}, preview={}, source={}: {cause}",
                    self.session.loading.is_some(), self.session.rendering.is_some(), self.session.displaying.is_some(),
                    self.session.retirement.is_some(), self.session.pending_retirement.is_some(),
                    self.session.deferred_retirement.is_some(), self.session.preview_pending(), self.session.selection)
            })?;
        }
        Ok(())
    }

    /// Observe real terminal jobs and existing worker disposal without requiring native publication.
    fn preview_idle(&mut self) -> Result<(), Box<dyn Error>> {
        for _ in 0..500 {
            self.preview_step()?;
            let snapshot = self.session.runtime.as_ref().ok_or("Runtime absent")?.scope.snapshot();
            if snapshot.running + snapshot.queued + snapshot.terminal_unobserved == 0
                && self.session.loading.is_none()
                && self.session.rendering.is_none()
                && self.session.displaying.is_none()
                && self.session.pending_retirement.is_none()
                && self.session.deferred_retirement.is_none()
                && self.session.retirement.is_none()
            {
                return Ok(());
            }
        }
        Err("Preview jobs did not settle".into())
    }

    /// Keep ordinary final acknowledgements live until the distinct candidate owns native custody.
    fn offered_preview(&mut self, token: PreviewToken) -> Result<FrameStamp, Box<dyn Error>> {
        for _ in 0..500 {
            if self.session.preview_status(token) != PreviewStatus::Preparing {
                return Err(
                    format!("Preview stopped: {:?}", self.session.preview_status(token)).into()
                );
            }
            self.preview_step()?;
            if let Some(target) =
                self.session.frame().filter(|frame| frame.pending).map(|frame| frame.target)
            {
                if self.session.document.cpu_pending != Some(target) {
                    return Ok(target);
                }
                self.session.did_present_cpu(target)?;
            }
        }
        Err("Preview never reached native custody".into())
    }

    /// Wait for actual cancellation, foreign-worker observation and parked-document restoration.
    fn withdraw_preview_and_wait(&mut self) -> Result<(), Box<dyn Error>> {
        self.session.withdraw_preview()?;
        for _ in 0..500 {
            self.preview_step()?;
            if !self.session.preview_pending() {
                return Ok(());
            }
        }
        Err("Preview restoration did not settle".into())
    }
}

#[test]
fn preview_acceptance_freezes_the_latest_actual_final_camera_and_withdraw_restores_pixels()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    let red = fixture.colored_frame([255, 0, 0])?;
    fixture.session.did_present_cpu(red)?;
    let final_identity = fixture.session.accepted_camera().ok_or("Final camera absent")?.document;
    let token = fixture.preview([0, 0, 255])?;
    assert!(!fixture.session.preview_shown());
    assert!(matches!(
        fixture.session.navigate(NavigationCommand::OneToOne)?,
        CameraCommandResult::Accepted(_)
    ));
    let target = fixture.offered_preview(token)?;
    let accepted = fixture.session.accepted_camera().ok_or("Final acceptance absent")?;
    let final_pixels = fixture
        .session
        .document
        .cpu_projection
        .as_ref()
        .ok_or("Final pixels absent")?
        .pixels()
        .as_ptr();
    assert!(matches!(
        fixture.session.navigate(NavigationCommand::Zoom(1.25))?,
        CameraCommandResult::Accepted(_)
    ));
    fixture.session.did_present_cpu(target)?;
    assert_eq!(fixture.session.preview_status(token), PreviewStatus::Presented(target));
    assert!(fixture.session.preview_shown());
    assert_eq!(
        fixture.session.navigate(NavigationCommand::Zoom(1.25))?,
        CameraCommandResult::Denied
    );
    let camera = fixture.session.document.tools.navigation().viewport();
    fixture.session.input(Input::Tool(ToolInput::OneToOne))?;
    assert_eq!(fixture.session.document.tools.navigation().viewport(), camera);
    assert_eq!(fixture.session.document.tools.navigation().zoom_mode(), ZoomMode::Fit);
    fixture.withdraw_preview_and_wait()?;
    let restored = fixture.session.selected_cpu_frame().ok_or("Restored final absent")?.0;
    assert!(restored.target.generation > target.generation);
    assert_eq!(restored.pixels.as_ptr(), final_pixels);
    assert_eq!(fixture.session.document.tools.navigation().viewport(), accepted.viewport);
    assert_eq!(
        fixture
            .session
            .document
            .scene
            .as_ref()
            .ok_or("Final scene absent")?
            .document
            .snapshot()
            .identity(),
        final_identity
    );
    fixture.session.did_present_cpu(restored.target)?;
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn successive_previews_preserve_the_final_and_resize_refits_the_locked_preview()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    let red = fixture.colored_frame([255, 0, 0])?;
    fixture.session.did_present_cpu(red)?;
    let final_view = fixture.session.accepted_camera().ok_or("Final camera absent")?.viewport;
    let first = fixture.preview([0, 255, 0])?;
    let first_target = fixture.offered_preview(first)?;
    fixture.session.did_present_cpu(first_target)?;
    let second = fixture.preview([0, 0, 255])?;
    let second_target = fixture.offered_preview(second)?;
    fixture.session.did_present_cpu(second_target)?;
    let backing =
        fixture.session.document.fallback_front.as_ref().ok_or("Preview backing absent")?.clone();
    fixture.session.resize(Extent::new(12, 8)?)?;
    fixture.clock.0.fetch_add(1_000_000_000, Ordering::SeqCst);
    let (resized, extent, _) = fixture.pending_frame()?;
    assert_eq!(extent, Extent::new(12, 8)?);
    fixture.session.did_present_cpu(resized)?;
    assert!(Arc::ptr_eq(
        &backing,
        fixture.session.document.fallback_front.as_ref().ok_or("Preview backing lost")?
    ));
    assert_eq!(fixture.session.document.tools.navigation().zoom_mode(), ZoomMode::Fit);
    assert_eq!(fixture.session.navigate(NavigationCommand::OneToOne)?, CameraCommandResult::Denied);
    drop(backing);
    fixture.session.resize(final_view.surface())?;
    fixture.withdraw_preview_and_wait()?;
    assert_eq!(fixture.session.document.tools.navigation().viewport(), final_view);
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn failed_or_cancelled_candidates_preserve_final_and_empty_preview_closes_with_owned_custody()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    fixture.session.resize(Extent::new(8, 8)?)?;
    fixture.session.input(Input::Drawable(true))?;
    let token = fixture.preview([0, 0, 255])?;
    let target = fixture.offered_preview(token)?;
    fixture.session.did_present_cpu(target)?;
    fixture.withdraw_preview_and_wait()?;
    assert!(fixture.session.accepted_camera().is_none());
    assert_eq!(fixture.session.camera_intent(), CameraCommandResult::NoContent);
    let red = fixture.colored_frame([255, 0, 0])?;
    fixture.session.did_present_cpu(red)?;
    let mut invalid = Some(ImageSource::file(fixture._directory.path().join("absent.png")));
    let token = fixture
        .session
        .offer_preview(
            &mut invalid,
            fixture.session.preview_now().checked_add(Duration::from_secs(30))?,
        )?
        .token()
        .ok_or("Preview refused")?;
    for _ in 0..500 {
        fixture.preview_step()?;
        if fixture.session.preview_status(token) == PreviewStatus::Failed {
            break;
        }
    }
    assert_eq!(fixture.session.preview_status(token), PreviewStatus::Failed);
    assert!(!fixture.session.source_failed());
    assert_eq!(fixture.session.document.native_presented, Some(red));
    let token = fixture.preview([0, 255, 0])?;
    fixture.session.withdraw_preview()?;
    fixture.withdraw_preview_and_wait()?;
    assert_eq!(fixture.session.preview_status(token), PreviewStatus::Cancelled);
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn resize_withdraws_an_offered_target_and_final_preparation_observes_preview_retirement()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    let red = fixture.colored_frame([255, 0, 0])?;
    fixture.session.did_present_cpu(red)?;
    let token = fixture.preview([0, 0, 255])?;
    let obsolete = fixture.offered_preview(token)?;
    fixture.session.resize(Extent::new(12, 8)?)?;
    assert!(fixture.session.did_present_cpu(obsolete).is_err());
    let replacement = fixture.offered_preview(token)?;
    assert!(replacement.generation > obsolete.generation);
    fixture.session.did_present_cpu(replacement)?;
    assert_eq!(
        fixture.session.document.tools.navigation().viewport().surface(),
        Extent::new(12, 8)?
    );
    let empty = fixture.stage_document()?;
    fixture.session.promote_empty_document(empty)?;
    assert!(fixture.session.promote_empty_document(empty).is_err());
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn close_observes_unpublished_and_accepted_preview_custody_without_a_final()
-> Result<(), Box<dyn Error>> {
    let mut unpublished = Fixture::start(true)?;
    unpublished.session.input(Input::Drawable(true))?;
    let _token = unpublished.preview([0, 0, 255])?;
    unpublished.session.poll(Duration::ZERO)?;
    let _directory = unpublished.finish()?;
    let mut accepted = Fixture::start(true)?;
    accepted.session.resize(Extent::new(8, 8)?)?;
    accepted.session.input(Input::Drawable(true))?;
    let token = accepted.preview([0, 255, 0])?;
    let target = accepted.offered_preview(token)?;
    accepted.session.did_present_cpu(target)?;
    let _directory = accepted.finish()?;
    Ok(())
}
