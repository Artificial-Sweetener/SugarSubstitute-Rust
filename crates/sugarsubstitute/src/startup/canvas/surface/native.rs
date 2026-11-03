// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Dispatch native canvas protocol requests to one entry point per request family.
//!
//! Each family entry point receives the decoded request, its ordinary-lane credit and any
//! bulk-upload storage that covers it. It answers the request exactly once, immediately
//! through `NativeSurface::answer` or later from a retained wait; cancelling a retained wait
//! reaches the owner through `NativeSurface::cancel`. Sources, geometry and workflow surfaces
//! are served from the session's input documents by their own modules; the editing entry point
//! answers `unsupported`, and `ADVERTISED` keeps that capability off.

use super::*;
use sugarsubstitute_contracts::canvas_native::{
    CanvasEditRequest, CanvasFailure, CanvasFailureStatus, CanvasNativeCapabilities,
    CanvasNativeReplyPayload, CanvasNativeRequest,
};
use sugarsubstitute_desktop::CanvasNativeRequestStorage;

/// The capabilities `context.reply` advertises while the canvas can display output.
///
/// A capability turns on only together with the entry point that services it.
pub(super) const ADVERTISED: CanvasNativeCapabilities = CanvasNativeCapabilities {
    display_output: true,
    geometry: true,
    editing: false,
    workflow_surfaces: true,
};

impl NativeSurface {
    /// Route one native request to the entry point that owns its family.
    pub(in crate::startup::canvas) fn native(
        &mut self,
        identity: MainIdentity,
        request: CanvasNativeRequest,
        claim: MemoryClaim,
        storage: CanvasNativeRequestStorage,
        session: Option<&mut Session>,
    ) -> Result<(), Box<dyn Error>> {
        if self.closed {
            return Ok(());
        }
        match request {
            CanvasNativeRequest::Context(request) => {
                self.context(identity, request, claim, session)
            }
            CanvasNativeRequest::Source(request) => {
                self.receive_source(identity, request, claim, storage, session)
            }
            CanvasNativeRequest::Geometry(request) => {
                self.receive_geometry(identity, request, claim, session)
            }
            CanvasNativeRequest::Edit(request) => {
                self.receive_edit(identity, request, claim, storage)
            }
            CanvasNativeRequest::Workflow(request) => {
                self.receive_workflow(identity, request, claim, storage, session)
            }
        }
    }

    /// Serve `edit.request`: tools, options, history, sessions and coverage modification.
    fn receive_edit(
        &mut self,
        identity: MainIdentity,
        request: CanvasEditRequest,
        claim: MemoryClaim,
        _storage: CanvasNativeRequestStorage,
    ) -> Result<(), Box<dyn Error>> {
        self.answer(identity, request.failure_reply(&unsupported())?, claim)
    }

    /// Answer a request the current page has no canvas authority for: an empty display-only
    /// context, or an `unavailable` failure for every other family.
    pub(in crate::startup::canvas) fn refuse_native(
        &mut self,
        identity: MainIdentity,
        request: &CanvasNativeRequest,
        claim: MemoryClaim,
    ) -> Result<(), Box<dyn Error>> {
        if self.closed {
            return Ok(());
        }
        let failure = CanvasFailure::new(CanvasFailureStatus::Unavailable);
        self.answer(identity, request.refusal_reply(&failure)?, claim)
    }

    /// Queue one small encoded reply with the request's lane credit.
    pub(super) fn answer(
        &mut self,
        identity: MainIdentity,
        payload: CanvasNativeReplyPayload,
        claim: MemoryClaim,
    ) -> Result<(), Box<dyn Error>> {
        let reply = CanvasNativeReply::new(identity, payload, claim)?;
        self.enqueue(DesktopStartupCommand::SendCanvasNative(reply))
    }
}

fn unsupported() -> CanvasFailure {
    CanvasFailure::new(CanvasFailureStatus::Unsupported)
}
