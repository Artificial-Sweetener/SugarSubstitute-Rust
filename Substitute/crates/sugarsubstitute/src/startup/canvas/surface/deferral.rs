// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hold a candidate's preparation behind the work that owns native mutation, instead of refusing
//! it as unavailable.
//!
//! One native surface serves one staging attachment and one waiting request at a time. A
//! candidate that arrives while another owns that slot is not unavailable, only early: refusing it
//! made a host that was merely busy look failed. It waits here, bounded by count to one, and is
//! served when the slot frees. A newer candidate supersedes the held one, which is answered
//! `stale`; a page that withdraws the held candidate gets `cancelled`. No elapsed time decides
//! any of this.

use super::*;

/// A candidate's `surface.prepare`, with the response custody its terminal reply will use.
pub(super) struct Deferred {
    pub(super) identity: MainIdentity,
    pub(super) request: Box<CanvasSurfaceRequest>,
    claim: MemoryClaim,
}

impl NativeSurface {
    /// Whether staging, a waiting request or a document restoration still owns native mutation,
    /// so a new candidate must wait for it.
    pub(super) fn prepare_blocked(&self) -> bool {
        self.restoring
            || self.pending.is_some()
            || self.retained.is_some()
            || self.binding.as_ref().is_some_and(|binding| binding.document.is_some())
    }

    /// Hold `request` until native mutation frees. A candidate already held was superseded by
    /// this newer one and is answered `stale`, so at most one request is ever held.
    ///
    /// # Errors
    /// Preserves response-queue overload from the superseded request's answer.
    pub(super) fn defer_prepare(
        &mut self,
        identity: MainIdentity,
        request: Box<CanvasSurfaceRequest>,
        claim: MemoryClaim,
    ) -> Result<(), Box<dyn Error>> {
        super::lifecycle::deferred(&request);
        let held = Deferred { identity, request, claim };
        if let Some(older) = self.deferred.replace(held) {
            self.reply(
                older.identity,
                &older.request,
                CanvasSurfaceReplyStatus::Stale,
                older.claim,
            )?;
        }
        Ok(())
    }

    /// Answer `cancelled` for the held candidate the page withdrew; false when `request_id` does
    /// not name it.
    ///
    /// # Errors
    /// Preserves response-queue overload.
    pub(super) fn cancel_deferred(
        &mut self,
        identity: MainIdentity,
        request_id: u64,
    ) -> Result<bool, Box<dyn Error>> {
        let Some(held) = self
            .deferred
            .take_if(|held| held.identity == identity && held.request.request_id() == request_id)
        else {
            return Ok(false);
        };
        self.reply(held.identity, &held.request, CanvasSurfaceReplyStatus::Cancelled, held.claim)?;
        Ok(true)
    }

    /// Serve the held candidate once nothing owns native mutation. A held candidate whose backend
    /// grant ended is answered `unavailable`: its attachment can never prepare.
    ///
    /// # Errors
    /// Preserves response-queue overload and the candidate's preparation failure.
    pub(super) fn admit_deferred(&mut self, session: &mut Session) -> Result<(), Box<dyn Error>> {
        let Some(held) = &self.deferred else { return Ok(()) };
        if !self.available(held.identity) {
            return self.answer_deferred(CanvasSurfaceReplyStatus::Unavailable);
        }
        if self.prepare_blocked() {
            return Ok(());
        }
        let held = self.deferred.take().ok_or("Held canvas candidate disappeared")?;
        super::lifecycle::admitted(&held.request);
        self.prepare(held.identity, held.request, held.claim, session)
    }

    /// Answer the held candidate with `status`, when one is held.
    ///
    /// # Errors
    /// Preserves response-queue overload.
    pub(super) fn answer_deferred(
        &mut self,
        status: CanvasSurfaceReplyStatus,
    ) -> Result<(), Box<dyn Error>> {
        match self.deferred.take() {
            Some(held) => self.reply(held.identity, &held.request, status, held.claim),
            None => Ok(()),
        }
    }

    /// Drop the held candidate of a retired navigation; no page remains to receive its reply.
    pub(super) fn retire_deferred(&mut self, identity: MainIdentity) {
        if let Some(held) = self.deferred.take_if(|held| held.identity == identity) {
            super::lifecycle::retired(&held.request, PendingKind::Prepare);
        }
    }
}
