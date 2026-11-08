// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hold pending surface requests and record each request's lifecycle as closed diagnostics.
//!
//! Every admitted request ends in exactly one terminal reply; these records show where a
//! request is between admission and that reply. Records are emitted per lifecycle step, and a
//! pending selection or viewport request records its unmet reply condition only when the reason
//! changes, so frame cadence never multiplies them.

use super::*;
use crate::canvas_surface_diagnostic::{
    Attachment, Operation, Pending as Waiting, Record, Stage, Status, Wait,
};
use crate::native_canvas::session::CameraWait;

impl NativeSurface {
    /// Hold `request` until a later native or worker effect resolves it with its terminal reply.
    pub(super) fn hold(
        &mut self,
        identity: MainIdentity,
        request: Box<CanvasSurfaceRequest>,
        claim: MemoryClaim,
        kind: PendingKind,
    ) {
        Record { pending: waiting(kind), ..record(Stage::Pending, &request) }.emit();
        self.pending =
            Some(Pending { identity, request, claim, kind, wait: Wait::None, preparation: None });
    }

    /// Record why the pending selection still has no reply, once per change of reason.
    pub(super) fn trace_selection_wait(&mut self, session: &Session) {
        let (Some(pending), Some(binding)) = (&mut self.pending, &self.binding) else { return };
        if !matches!(pending.kind, PendingKind::Select) {
            return;
        }
        let wait = selection_wait(binding, session);
        let preparation = (wait == Wait::Preparing).then(|| session.native_preparation_snapshot());
        if let Some(snapshot) = preparation
            && pending.preparation != preparation
        {
            crate::canvas_preparation_diagnostic::Record {
                request_id: pending.request.request_id(),
                intent: binding.source_intent_revision.unwrap_or(0),
                current: session.source_intent_revision(),
                snapshot,
            }
            .emit();
        }
        pending.preparation = preparation;
        if wait == pending.wait {
            return;
        }
        pending.wait = wait;
        Record {
            pending: Waiting::Select,
            wait,
            intent: binding.source_intent_revision.unwrap_or(0),
            current: session.source_intent_revision(),
            ..record(Stage::Waiting, &pending.request)
        }
        .emit();
    }

    /// Record why the pending viewport request's camera has not settled, once per change of
    /// reason, and return that reason.
    pub(super) fn trace_viewport_wait(&mut self, session: &Session) -> Wait {
        let wait = camera_wait(session.camera_wait());
        let Some(pending) = &mut self.pending else { return wait };
        if wait != pending.wait {
            pending.wait = wait;
            Record {
                pending: Waiting::Viewport,
                wait,
                current: session.source_intent_revision(),
                ..record(Stage::Waiting, &pending.request)
            }
            .emit();
        }
        wait
    }
}

/// Record a viewport request that outlived its settle budget while waiting for `wait`.
pub(super) fn expired(request: &CanvasSurfaceRequest, wait: Wait) {
    Record { pending: Waiting::Viewport, wait, ..record(Stage::Expired, request) }.emit();
}

/// Record admission past attachment, ordering and availability checks.
pub(super) fn admitted(request: &CanvasSurfaceRequest) {
    record(Stage::Admitted, request).emit();
}

/// Record the source intent a selection offered to the session.
pub(super) fn offered(request: &CanvasSurfaceRequest, intent: u64) {
    Record { intent, current: intent, ..record(Stage::Offered, request) }.emit();
}

/// Record a pending selection re-bound from its offered intent to the session's rebuild of it.
pub(super) fn rebound(request: &CanvasSurfaceRequest, offered: u64, rebuilt: u64) {
    Record {
        pending: Waiting::Select,
        intent: offered,
        current: rebuilt,
        ..record(Stage::Rebound, request)
    }
    .emit();
}

/// Record a queued terminal reply. Preview transfers are excluded: each chunk has its own reply.
pub(super) fn replied(request: &CanvasSurfaceRequest, status: CanvasSurfaceReplyStatus) {
    if matches!(operation(request.operation()), Operation::Preview) {
        return;
    }
    Record { status: reply_status(status), ..record(Stage::Replied, request) }.emit();
}

/// Record a request whose navigation retired before its reply; no page can receive one.
pub(super) fn retired(request: &CanvasSurfaceRequest, kind: PendingKind) {
    Record { pending: waiting(kind), ..record(Stage::Retired, request) }.emit();
}

/// Record a cancellation that matched no pending request; an earlier reply already ended it.
pub(super) fn unmatched(request_id: u64) {
    Record {
        stage: Stage::Unmatched,
        operation: Operation::Cancel,
        request_id,
        revision: 0,
        generation: 0,
        attachment: None,
        pending: Waiting::None,
        wait: Wait::None,
        intent: 0,
        current: 0,
        status: Status::None,
    }
    .emit();
}

/// Classify the first unmet condition of a pending selection's reply, in the order it resolves.
fn selection_wait(binding: &Binding, session: &Session) -> Wait {
    if binding.source_intent_revision != Some(session.source_intent_revision()) {
        return Wait::Superseded;
    }
    if session.source_loading() {
        return Wait::Acquiring;
    }
    if binding.committed && binding.placement.is_none() {
        return Wait::Hidden;
    }
    match session.selected_frame() {
        None => Wait::Preparing,
        Some(_) if session.host_graphics().is_none() => Wait::Graphics,
        Some((content, _)) if content.pending() => Wait::Presenting,
        Some(_) => Wait::Accepting,
    }
}

const fn camera_wait(wait: CameraWait) -> Wait {
    match wait {
        CameraWait::Moving => Wait::Moving,
        CameraWait::Hidden => Wait::Hidden,
        CameraWait::Preparing => Wait::Preparing,
        CameraWait::Presenting => Wait::Presenting,
        CameraWait::Accepting => Wait::Accepting,
    }
}

fn record(stage: Stage, request: &CanvasSurfaceRequest) -> Record {
    Record {
        stage,
        operation: operation(request.operation()),
        request_id: request.request_id(),
        revision: request.revision(),
        generation: request.address().generation(),
        attachment: Attachment::of(request.address().attachment_id()),
        pending: Waiting::None,
        wait: Wait::None,
        intent: 0,
        current: 0,
        status: Status::None,
    }
}

fn operation(operation: &CanvasSurfaceOperation) -> Operation {
    match operation {
        CanvasSurfaceOperation::Prepare => Operation::Prepare,
        CanvasSurfaceOperation::Geometry(_) => Operation::Geometry,
        CanvasSurfaceOperation::Select { comparison: None, .. } => Operation::Select,
        CanvasSurfaceOperation::Select { comparison: Some(_), .. } => Operation::Compare,
        CanvasSurfaceOperation::Overview(_) => Operation::Overview,
        CanvasSurfaceOperation::Viewport(_) => Operation::Viewport,
        CanvasSurfaceOperation::PreviewBegin(_)
        | CanvasSurfaceOperation::PreviewChunk(_)
        | CanvasSurfaceOperation::PreviewEnd(_)
        | CanvasSurfaceOperation::PreviewWithdraw(_) => Operation::Preview,
        CanvasSurfaceOperation::Commit => Operation::Commit,
        CanvasSurfaceOperation::Release => Operation::Release,
    }
}

const fn waiting(kind: PendingKind) -> Waiting {
    match kind {
        PendingKind::Prepare => Waiting::Prepare,
        PendingKind::Restore => Waiting::Restore,
        PendingKind::Select => Waiting::Select,
        PendingKind::Geometry => Waiting::Geometry,
        PendingKind::Hide => Waiting::Hide,
        PendingKind::Commit => Waiting::Commit,
        PendingKind::EmptyCommit => Waiting::EmptyCommit,
        PendingKind::Viewport { .. } => Waiting::Viewport,
    }
}

const fn reply_status(status: CanvasSurfaceReplyStatus) -> Status {
    match status {
        CanvasSurfaceReplyStatus::Ready => Status::Ready,
        CanvasSurfaceReplyStatus::Applied => Status::Applied,
        CanvasSurfaceReplyStatus::Stale => Status::Stale,
        CanvasSurfaceReplyStatus::Cancelled => Status::Cancelled,
        CanvasSurfaceReplyStatus::Unavailable => Status::Unavailable,
        CanvasSurfaceReplyStatus::Error => Status::Error,
    }
}
