// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Request one application poll for new intent, then use actual admission and presentation wakes.

use super::{preview::*, *};

impl Session {
    /// Consume an external intent wake once; refusal retries retain the original deadline.
    pub(in crate::native_canvas::session) fn consume_preview_poll_request(&mut self) {
        self.preview.poll_requested = false;
    }

    /// Require complete private pixels and available native custody before spending a pacing slot.
    pub(super) fn preview_can_offer(&self, candidate: &Candidate) -> bool {
        let extent = self.document.tools.navigation().viewport().surface();
        if candidate.cancelled
            || candidate.owner_selection != self.selection
            || self.closing
            || !self.drawable
            || self.displaying.is_some()
            || self.document.cpu_pending.is_some()
            || self.gpu_presentation_pending()
            || self.pending_retirement.is_some()
            || self.deferred_retirement.is_some()
            || self.preview.retired.is_some()
            || !matches!(&candidate.stage, Stage::Ready(Some(ready)) if ready.drawing.view.surface() == extent)
        {
            return false;
        }
        self.preview_shown()
            || self.document.scene.as_ref().is_none_or(|scene| scene.source.is_none())
            || (self.document.native_presented.is_some()
                && self.document.display_evidence.is_some_and(|evidence| {
                    evidence.view.surface() == extent && evidence.source_revision == self.selection
                }))
    }

    /// Preserve newer source selection while restoring the parked document's custody.
    pub(super) fn preview_replacing_final(&self) -> bool {
        self.pending_open.as_ref().is_some_and(|open| open.intent == self.selection)
            || self.loading.as_ref().is_some_and(|loading| loading.intent == self.selection)
    }

    /// Restoration waits for actual foreign job outcomes and owned retirement capacity.
    pub(super) fn preview_can_restore(&self) -> bool {
        self.preview.restoring
            && self.preview.candidate.is_none()
            && self.preview.pending.is_none()
            && self.rendering.is_none()
            && self.displaying.is_none()
            && self.pending_retirement.is_none()
            && self.deferred_retirement.is_none()
            && self.retirement.is_none()
            && self.preview.resize_job.is_none()
            && self.preview.retired.is_none()
    }

    /// Wake once for newly recorded intent; ongoing work and refused admission own their real wakes.
    pub(in crate::native_canvas::session) fn preview_deadline(
        &self,
        now: Duration,
    ) -> Result<Option<Duration>, Box<dyn Error>> {
        let offer = self
            .preview
            .candidate
            .as_ref()
            .is_some_and(|candidate| self.preview_can_offer(candidate));
        let restore = self.preview_can_restore()
            && !self.closing
            && !self.document_suspended
            && self.drawable
            && !self.preview_replacing_final();
        let pacing = if offer || restore { self.pacing.next_deadline(now)? } else { None };
        let candidate = self
            .preview
            .candidate
            .as_ref()
            .filter(|candidate| !candidate.cancelled)
            .map(|candidate| Duration::from_nanos(candidate.deadline.as_nanos()));
        let pending = self
            .preview
            .pending
            .as_ref()
            .filter(|input| !input.cancelled)
            .map(|input| Duration::from_nanos(input.deadline.as_nanos()));
        let resize = self
            .preview
            .resize_deadline
            .filter(|deadline| {
                self.preview_shown()
                    && !self.preview.restoring
                    && self.preview.resize_job.is_none()
                    && *deadline > self.clock.now()
            })
            .map(|deadline| Duration::from_nanos(deadline.as_nanos()));
        Ok([self.preview.poll_requested.then_some(now), pacing, candidate, pending, resize]
            .into_iter()
            .flatten()
            .min())
    }
}
