// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise candidate rollback and exact native promotion with real image workers.

use super::*;
mod previews;

impl Fixture {
    /// Observe shared worker settlement until the session transfers the preceding document.
    fn stage_document(&mut self) -> Result<DocumentToken, Box<dyn Error>> {
        self.session.input(Input::Drawable(false))?;
        loop {
            let revision = self.wake.revision();
            if let Some(token) = self.session.prepare_document()? {
                return Ok(token);
            }
            self.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
            self.session.poll(Duration::ZERO)?;
            if let Some(token) = self.session.prepare_document()? {
                return Ok(token);
            }
            self.wake.wait_after(revision);
        }
    }

    /// Keep cancellation distinct from actual restoration and worker-owned disposal.
    fn restore_document(&mut self, token: DocumentToken) -> Result<(), Box<dyn Error>> {
        self.session.restore_document(token)?;
        while self.session.document_transaction_pending(token) {
            let revision = self.wake.revision();
            self.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
            self.session.poll(Duration::ZERO)?;
            if self.session.document_transaction_pending(token) {
                self.wake.wait_after(revision);
            }
        }
        Ok(())
    }

    /// Prepare actual encoded pixels through the application's shared executor.
    fn colored_frame(&mut self, color: [u8; 3]) -> Result<FrameStamp, Box<dyn Error>> {
        let mut source = Some(self.encoded(color)?);
        assert!(self.session.offer_source(&mut source, CacheReuse::Compatible, None)?);
        self.session.resize(Extent::new(8, 8)?)?;
        self.session.input(Input::Drawable(true))?;
        self.clock.0.fetch_add(1_000_000_000, Ordering::SeqCst);
        let prepared = self.pending_frame().map_err(|cause| {
            let state = self.session.runtime.as_ref().map(|runtime| runtime.scope.snapshot());
            format!("Final frame preparation scope={state:?}, pending_open={}, load={}, render={}, display={}, retirement={}, source={}: {cause}",
                self.session.pending_open.is_some(), self.session.loading.is_some(),
                self.session.rendering.is_some(), self.session.displaying.is_some(),
                self.session.retirement.is_some(), self.session.selection)
        })?;
        Ok(prepared.0)
    }
}

#[test]
fn cancelled_candidate_restores_exact_pixels_camera_and_source_without_reusing_intents()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    let red = fixture.colored_frame([255, 0, 0])?;
    fixture.session.did_present_cpu(red)?;
    let pixels = fixture.session.frame().ok_or("Red projection absent")?.pixels.as_ptr();
    let camera = fixture.session.document.tools.navigation().viewport();
    let token = fixture.stage_document()?;
    let blue = fixture.colored_frame([0, 0, 255])?;
    let candidate_intent = fixture.session.selection;
    assert!(fixture.session.promote_document(token, blue).is_err());
    assert!(fixture.session.promote_document(token, red).is_err());
    fixture.restore_document(token)?;
    assert!(fixture.session.did_present_cpu(blue).is_err());
    let restored = fixture.session.selected_cpu_frame().ok_or("Restored red frame absent")?.0;
    assert_eq!(restored.target, red);
    assert_eq!(restored.pixels.as_ptr(), pixels);
    assert_eq!(fixture.session.document.tools.navigation().viewport(), camera);
    assert!(fixture.session.promote_document(token, red).is_err());
    let next = fixture.colored_frame([0, 255, 0])?;
    assert!(fixture.session.selection > candidate_intent);
    assert!(next.generation > blue.generation);
    fixture.session.did_present_cpu(next)?;
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn released_presentation_does_not_block_the_next_document_on_an_unacknowledged_frame()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    let pending = fixture.colored_frame([255, 0, 0])?;
    assert_eq!(fixture.session.document.cpu_pending, Some(pending));
    fixture.session.revoke_presentation()?;
    assert!(fixture.session.document.cpu_pending.is_none());
    assert!(fixture.session.did_present_cpu(pending).is_err());
    let token = fixture.stage_document()?;
    let blue = fixture.colored_frame([0, 0, 255])?;
    fixture.session.did_present_cpu(blue)?;
    fixture.session.promote_document(token, blue)?;
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn cancelling_a_fully_loaded_unpresented_selection_allows_the_next_document_to_prepare()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    let pending = fixture.colored_frame([255, 0, 0])?;
    let selection = fixture.session.selection;
    assert!(fixture.session.loading.is_none());
    assert!(fixture.session.pending_open.is_none());
    assert_eq!(fixture.session.document.cpu_pending, Some(pending));

    fixture.session.cancel_selection()?;

    assert!(fixture.session.selection > selection);
    assert!(fixture.session.document.cpu_pending.is_none());
    assert!(fixture.session.document.pending_cpu_evidence.is_none());
    assert!(fixture.session.selected_cpu_frame().is_none());
    assert!(fixture.session.did_present_cpu(pending).is_err());
    let token = fixture.stage_document()?;
    let blue = fixture.colored_frame([0, 0, 255])?;
    fixture.session.did_present_cpu(blue)?;
    fixture.session.promote_document(token, blue)?;
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn acknowledged_candidate_promotes_once_and_retires_both_documents_on_close()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    let red = fixture.colored_frame([255, 0, 0])?;
    fixture.session.did_present_cpu(red)?;
    let token = fixture.stage_document()?;
    let blue = fixture.colored_frame([0, 0, 255])?;
    fixture.session.did_present_cpu(blue)?;
    assert!(fixture.session.promote_document(token, red).is_err());
    fixture.session.promote_document(token, blue)?;
    assert!(fixture.session.promote_document(token, blue).is_err());
    assert!(fixture.session.restore_document(token).is_err());
    let _directory = fixture.finish()?;
    Ok(())
}
