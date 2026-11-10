// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the application running when the composition acknowledges a frame that cannot be accepted.
//!
//! An acknowledgement the session ignores changes nothing. A rejected frame is dropped: the last
//! accepted frame stays on screen, the session demands a fresh frame, and the request that waited
//! on the dropped frame stays open: it completes with the next accepted frame, or ends by
//! cancellation or release, never by a clock. An attachment whose frames are rejected repeatedly
//! presents again from fresh authority. Unclassified errors stay fatal.

use super::*;
use crate::native_canvas::session::{Ignored, Rejection, Unaccepted};

/// Consecutive rejected frames after which an attachment presents again from fresh authority.
const REPRESENT_AFTER: u8 = 3;

impl NativeSurface {
    /// Resolve one native presentation acknowledgement of `target`.
    ///
    /// # Errors
    /// Returns only acknowledgement failures that are not classified as ignored or rejected:
    /// custody, ownership and accounting corruption.
    pub(in crate::startup::canvas) fn presented(
        &mut self,
        target: FrameStamp,
        session: &mut Session,
    ) -> Result<(), Box<dyn Error>> {
        let cause = match self.accept_presented(target, session) {
            Ok(()) => {
                if self.rejected != Some(target) {
                    self.rejected = None;
                }
                if let Some(binding) = self.frame_binding_mut() {
                    binding.rejected_frames = 0;
                }
                return Ok(());
            }
            Err(cause) => cause,
        };
        match cause.downcast_ref::<Unaccepted>() {
            Some(Unaccepted::Ignored(ignored)) => {
                tracing::debug!(
                    operation = "canvas_acknowledgement_ignored",
                    reason = ignored_name(*ignored),
                    surface = target.surface.get(),
                    generation = target.generation.get(),
                    "Ignored a native canvas acknowledgement that matches no awaited frame"
                );
                Ok(())
            }
            Some(Unaccepted::Rejected(rejected)) => {
                self.reject_frame(target, rejected.reason(), cause.as_ref(), session)
            }
            None => Err(cause),
        }
    }

    /// Resolve a hide acknowledgement that no longer matches the binding as ignored.
    pub(super) fn ignore_stale_hide(&self, identity: MainIdentity) {
        tracing::debug!(
            operation = "canvas_acknowledgement_ignored",
            reason = ignored_name(Ignored::Stale),
            attachment = %self.attachment_label(),
            matches_binding = self.binding.as_ref().is_some_and(|binding| binding.identity == identity),
            "Ignored a native canvas hide acknowledgement that matches no awaited hide"
        );
    }

    /// The binding the frame being acknowledged belongs to: the retained one while preparing.
    fn frame_binding_mut(&mut self) -> Option<&mut Binding> {
        let preparing = self
            .pending
            .as_ref()
            .is_some_and(|pending| matches!(pending.kind, PendingKind::Prepare));
        if preparing { self.retained.as_mut() } else { self.binding.as_mut() }
    }

    /// The leading hexadecimal characters of the attachment identity, enough to correlate records
    /// without recording page-chosen text.
    fn attachment_label(&self) -> String {
        self.binding
            .as_ref()
            .and_then(|binding| binding.address.attachment_id().strip_prefix("attachment:"))
            .map(|identity| {
                identity.chars().take(8).filter(|ch| matches!(ch, '0'..='9' | 'a'..='f')).collect()
            })
            .unwrap_or_default()
    }

    /// Drop a frame that cannot be accepted, keeping the last accepted frame on screen and any
    /// request that waited on it pending.
    fn reject_frame(
        &mut self,
        target: FrameStamp,
        reason: Rejection,
        cause: &(dyn Error + 'static),
        session: &mut Session,
    ) -> Result<(), Box<dyn Error>> {
        session.release_rejected_frame(target)?;
        self.rejected = Some(target);
        let attachment = self.attachment_label();
        let consecutive = self.frame_binding_mut().map_or(0, |binding| {
            binding.rejected_frames = binding.rejected_frames.saturating_add(1);
            binding.rejected_frames
        });
        let (revision, request_id) = self.binding.as_ref().map_or((0, 0), |binding| {
            (
                binding.revision,
                self.pending.as_ref().map_or(0, |pending| pending.request.request_id()),
            )
        });
        tracing::warn!(
            operation = "canvas_frame_rejected",
            reason = reason.name(),
            attachment = %attachment,
            revision,
            request_id,
            surface = target.surface.get(),
            generation = target.generation.get(),
            consecutive,
            cause = %cause,
            "Dropped a rejected canvas frame; the last accepted frame stays shown"
        );
        if consecutive >= REPRESENT_AFTER {
            self.represent(session)?;
        }
        Ok(())
    }

    /// Release the attachment's presentation authority and prepare it again.
    ///
    /// The session abandons frame custody and withdraws its backdrop generation; the binding's
    /// committed placement and backdrops then take a new generation and the session draws again
    /// once ordinary readiness allows it. Nothing waits on a clock, and the last accepted frame
    /// stays on screen until a fresh one is accepted; the rejected frame is never shown.
    fn represent(&mut self, session: &mut Session) -> Result<(), Box<dyn Error>> {
        self.fence_previews(session)?;
        session.set_backdrops(Vec::new())?;
        session.revoke_presentation()?;
        if let Some(binding) = self.frame_binding_mut() {
            binding.rejected_frames = 0;
        }
        tracing::info!(
            operation = "canvas_attachment_represented",
            attachment = %self.attachment_label(),
            "Presenting the attachment again after repeated rejected frames"
        );
        self.resume_document(session)
    }
}

const fn ignored_name(ignored: Ignored) -> &'static str {
    match ignored {
        Ignored::Stale => "stale",
        Ignored::Foreign => "foreign",
        Ignored::HidePending => "hide_pending",
    }
}
