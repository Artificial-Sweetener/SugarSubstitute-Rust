// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain bounded preview intent independently from the currently accepted document.

use super::*;
use sugarsubstitute_image_ops::Extent;

/// Unpublished projection replacement never cancels the ordinary display domain.
pub(super) const PREVIEW_PROJECTION_SURFACE: u64 = 3;
/// Resize allocation has its own bounded replacement identity within the same execution scope.
pub(super) const PREVIEW_RESIZE_SURFACE: u64 = 4;

/// Identity of one offered preview, independent from its eventual native frame generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreviewToken(pub(super) u64);

/// Native acceptance is the only transition that reports a presented preview.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreviewStatus {
    /// Original import, private evaluation or exact native presentation is still pending.
    Preparing,
    /// This exact candidate was acknowledged by the native CPU presentation adapter.
    Presented(FrameStamp),
    /// The still-current candidate retained an unsuccessful worker outcome.
    Failed,
    /// A newer intent or explicit withdrawal fenced this candidate.
    Cancelled,
}

/// Outcome of one preview offer; every refusal leaves the original in the caller's slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreviewOffer {
    /// The session took the original and will report this candidate's status.
    Offered(PreviewToken),
    /// Current custody cannot admit a candidate yet; retry with the same original and deadline.
    Busy,
    /// The original nonrenewable deadline has passed, read against the session execution clock.
    Expired,
}

#[cfg(test)]
impl PreviewOffer {
    /// Return the admitted candidate identity, treating either refusal as absent.
    pub(crate) fn token(self) -> Option<PreviewToken> {
        match self {
            Self::Offered(token) => Some(token),
            Self::Busy | Self::Expired => None,
        }
    }
}

/// Original custody and a fixed application deadline travel together through admission retries.
pub(super) struct Input {
    pub(super) token: PreviewToken,
    pub(super) source: ImageSource,
    pub(super) revision: u64,
    pub(super) deadline: Moment,
    pub(super) owner_selection: u64,
    pub(super) cancelled: bool,
}

/// Exactly one worker or one retained candidate owns the active preparation.
pub(super) enum Stage {
    Input(Option<Input>),
    Loading(Option<OwnedJob<Scene, WorkError>>),
    Scene(Option<Scene>),
    Preparing(Option<OwnedJob<work::preview::ReadyPreview, WorkError>>),
    Ready(Option<Box<work::preview::ReadyPreview>>),
    Offered(Box<document::DocumentState>, Option<Resources>),
}

/// Bind one unpublished lifetime and retained native source custody to its original deadline.
pub(super) struct Candidate {
    pub(super) token: PreviewToken,
    pub(super) deadline: Moment,
    pub(super) cancelled: bool,
    pub(super) expired: bool,
    pub(super) owner_selection: u64,
    pub(super) stage: Stage,
    pub(super) custody: Option<SourceCustody>,
}

/// Keep native originals and archive readers alive until a storage worker retires their last clone.
pub(super) struct SourceCustody {
    pub(super) _original: Option<ImageSource>,
    pub(super) _source: Option<work::Source>,
}

/// Disposal uses the session's existing infrastructure lane and original memory claims.
#[derive(Default)]
pub(in crate::native_canvas::session) struct PreviewRetired {
    pub(super) _input: Option<Input>,
    pub(super) _ready: Option<Box<work::preview::ReadyPreview>>,
    pub(super) _document: Option<Box<document::DocumentState>>,
    pub(super) _resources: Option<Resources>,
    pub(super) _custody: Option<SourceCustody>,
    pub(super) _original: Option<ImageSource>,
    pub(super) _bytes: Option<(Vec<u8>, MemoryClaim)>,
}

/// One current preparation, one latest waiting original and one parked final document.
#[derive(Default)]
pub(in crate::native_canvas::session) struct PreviewState {
    pub(super) candidate: Option<Candidate>,
    pub(super) pending: Option<Input>,
    pub(super) parked: Option<RetainedDocument>,
    pub(super) shown: Option<(PreviewToken, FrameStamp)>,
    pub(super) last: Option<(PreviewToken, PreviewStatus)>,
    pub(super) restoring: bool,
    pub(super) resources: Option<Resources>,
    pub(super) resize_job:
        Option<OwnedJob<sugarsubstitute_canvas_render::ProjectionBuffers, WorkError>>,
    pub(super) resize_deadline: Option<Moment>,
    pub(super) resize_extent: Option<Extent>,
    pub(super) resize_job_intent: Option<(Extent, Moment)>,
    pub(super) retired: Option<PreviewRetired>,
    pub(super) poll_requested: bool,
}

impl Session {
    /// Offer one preview without changing source selection or navigation of the displayed final.
    /// Busy and expired results preserve the identical original in the caller's slot. The caller
    /// supplies the original nonrenewable deadline in the session execution clock's epoch; expiry
    /// is decided here so a caller's earlier clock read cannot race it into a fatal error.
    pub(crate) fn offer_preview(
        &mut self,
        source: &mut Option<ImageSource>,
        deadline: Moment,
    ) -> Result<PreviewOffer, Box<dyn Error>> {
        if deadline <= self.clock.now() {
            return Ok(PreviewOffer::Expired);
        }
        if self.closing
            || !self.presents_previews()
            || self.resources.is_none()
            || self.document_transaction.is_some()
            || self.preview.restoring
            || self.deferred_retirement.is_some()
            || self.preview.resize_job.is_some()
            || self.preview.retired.is_some()
            || self.pending_open.is_some()
            || self.loading.is_some()
            || self.pending_restore.is_some()
        {
            return Ok(PreviewOffer::Busy);
        }
        if source.is_none() {
            return Err("Preview original is unavailable".into());
        }
        let token = PreviewToken(
            self.document_sequence.checked_add(1).ok_or("Document identity exhausted")?,
        );
        let revision =
            self.source_sequence.checked_add(1).ok_or("Canvas source revision exhausted")?;
        if self.preview.pending.is_some() {
            let old = self.preview.pending.take();
            self.retire(Retired {
                _preview: Some(PreviewRetired { _input: old, ..Default::default() }),
                ..Default::default()
            })?;
        }
        self.cancel_preview_candidate()?;
        if let Some(job) = &self.preview.resize_job {
            job.cancel();
        }
        self.document_sequence = token.0;
        self.source_sequence = revision;
        self.preview.pending = Some(Input {
            token,
            source: source.take().ok_or("Preview original disappeared")?,
            revision,
            deadline,
            owner_selection: self.selection,
            cancelled: false,
        });
        self.preview.last = Some((token, PreviewStatus::Preparing));
        self.preview.poll_requested = true;
        Ok(PreviewOffer::Offered(token))
    }

    /// Observe bounded status; superseded identities always resolve to cancellation.
    pub(crate) fn preview_status(&self, token: PreviewToken) -> PreviewStatus {
        if let Some((current, target)) = self.preview.shown
            && current == token
        {
            return if self.preview.restoring {
                PreviewStatus::Cancelled
            } else {
                PreviewStatus::Presented(target)
            };
        }
        if self
            .preview
            .pending
            .as_ref()
            .is_some_and(|input| input.token == token && !input.cancelled)
            || self
                .preview
                .candidate
                .as_ref()
                .is_some_and(|candidate| candidate.token == token && !candidate.cancelled)
        {
            return PreviewStatus::Preparing;
        }
        self.preview
            .last
            .filter(|(current, _)| *current == token)
            .map_or(PreviewStatus::Cancelled, |(_, status)| status)
    }

    /// Cancel unpublished intent and restore the parked final after actual worker settlement.
    pub(crate) fn withdraw_preview(&mut self) -> Result<(), Box<dyn Error>> {
        let changed = self.preview.pending.as_ref().is_some_and(|input| !input.cancelled)
            || self.preview.candidate.as_ref().is_some_and(|candidate| !candidate.cancelled)
            || (self.preview.parked.is_some() && !self.preview.restoring);
        self.cancel_preview_candidate()?;
        if let Some(job) = &self.preview.resize_job {
            job.cancel();
        }
        self.preview.restoring |= self.preview.parked.is_some();
        if let Some((token, _)) = self.preview.shown {
            self.preview.last = Some((token, PreviewStatus::Cancelled));
        }
        self.preview.poll_requested |= changed;
        Ok(())
    }

    /// Accepted previews retain immutable whole-document pixels and deny navigation through tools.
    pub(in crate::native_canvas::session) fn preview_shown(&self) -> bool {
        self.preview.shown.is_some()
    }

    /// Fence new final-load admission until accepted-preview custody has restored its predecessor.
    pub(in crate::native_canvas::session) fn preview_restoring(&self) -> bool {
        self.preview.restoring
    }

    /// Closing/final replacement waits for observed preview jobs and worker-owned disposal.
    pub(in crate::native_canvas::session) fn preview_pending(&self) -> bool {
        self.preview.candidate.is_some()
            || self.preview.pending.is_some()
            || self.preview.parked.is_some()
            || self.preview.shown.is_some()
            || self.preview.resize_job.is_some()
            || self.preview.retired.is_some()
    }

    /// Fence active preparation without confusing cancellation with completion or disposal.
    fn cancel_preview_candidate(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(candidate) = &mut self.preview.candidate {
            candidate.cancelled = true;
            match &candidate.stage {
                Stage::Loading(Some(job)) => job.cancel(),
                Stage::Preparing(Some(job)) => job.cancel(),
                _ => {}
            }
            if let Stage::Offered(document, _) = &mut candidate.stage
                && let Some(target) = document.cpu_pending.take()
            {
                self.pacing.abandoned(target, Duration::from_nanos(self.clock.now().as_nanos()))?;
            }
        }
        if let Some(mut input) = self.preview.pending.take() {
            input.cancelled = true;
            self.preview.last = Some((input.token, PreviewStatus::Cancelled));
            if self.deferred_retirement.is_some() {
                self.preview.pending = Some(input);
                return Ok(());
            }
            self.retire(Retired {
                _preview: Some(PreviewRetired { _input: Some(input), ..Default::default() }),
                ..Default::default()
            })?;
        }
        Ok(())
    }
}
