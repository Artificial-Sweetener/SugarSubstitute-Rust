// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Serve `edit.request` and report `edit.changed` for the shown input document.
//!
//! Every successful command replies with the resulting state. The navigation that last sent a
//! request subscribes to changes: whenever the state differs from what it last received, it
//! gets one `edit.changed`, at most one per frame, carrying a new revision. Selection bounds
//! are converted from physical viewport pixels to CSS pixels relative to the surface geometry
//! origin, so they follow pan and zoom.

use super::input_protocol::{document_id, layer_key};
use super::*;
use crate::native_canvas::session::editing::{EditRejection, EditState};
use sugarsubstitute_canvas::{BrushOptions, SessionKind};
use sugarsubstitute_contracts::{
    canvas_native::{
        CanvasBrushOptions, CanvasEditMode, CanvasEditRequest, CanvasEditSelectionBounds,
        CanvasEditSession, CanvasEditSessionKind, CanvasEditState, CanvasEditTool, CanvasFailure,
        CanvasFailureMessage, CanvasFailureStatus,
    },
    execution::Moment,
    panel::PanelSession,
};
use sugarsubstitute_desktop::CanvasNativeNotification;

/// Shortest interval between two `edit.changed` notifications: one 60 Hz frame.
const NOTIFICATION_INTERVAL: Duration = Duration::from_nanos(16_666_667);
/// Longest failure detail a reply carries, in characters.
const FAILURE_DETAIL: usize = 200;

/// The editing subscriber and the state it last received.
#[derive(Default)]
pub(super) struct Editing {
    subscriber: Option<MainIdentity>,
    reported: Option<CanvasEditState>,
    notified: Option<Moment>,
    /// Whether the state changed since the subscriber last received it.
    pending: bool,
    pub(super) notification: Option<DesktopStartupCommand>,
}

impl Editing {
    /// The clock reading in nanoseconds before which no further notification is sent.
    fn due(&self) -> Option<u64> {
        let interval = u64::try_from(NOTIFICATION_INTERVAL.as_nanos()).unwrap_or(u64::MAX);
        self.notified.map(|at| at.as_nanos().saturating_add(interval))
    }

    /// Forget a retired navigation's subscription and its undelivered notification.
    pub(super) fn retire_navigation(&mut self, identity: MainIdentity) {
        if self.subscriber == Some(identity) {
            *self = Self::default();
        }
    }
}

/// A failure with developer-facing detail.
fn failure(status: CanvasFailureStatus, detail: &str) -> CanvasFailure {
    let detail: String = detail.chars().take(FAILURE_DETAIL).collect();
    CanvasFailure {
        status,
        message: CanvasFailureMessage::new(detail)
            .unwrap_or_else(|_| CanvasFailure::new(status).message),
    }
}

/// Whole percent of a `[0, 1]` fraction.
fn percent(fraction: f64) -> u8 {
    (fraction * 100.0).round().clamp(0.0, 100.0) as u8
}

fn brush(options: BrushOptions) -> Result<CanvasBrushOptions, Box<dyn Error>> {
    Ok(CanvasBrushOptions::new(
        u16::try_from(options.size())?,
        percent(options.hardness()),
        percent(options.opacity()),
    )?)
}

impl NativeSurface {
    /// Serve `edit.request`: carry out the command on the shown input document and reply with
    /// the resulting state, or a typed failure that changed nothing.
    pub(super) fn receive_edit(
        &mut self,
        identity: MainIdentity,
        request: CanvasEditRequest,
        claim: MemoryClaim,
        session: Option<&mut Session>,
    ) -> Result<(), Box<dyn Error>> {
        let Some(session) = session.filter(|session| session.initialized()) else {
            let failure = CanvasFailure::new(CanvasFailureStatus::Unavailable);
            return self.answer(identity, request.failure_reply(&failure)?, claim);
        };
        if !self.available(identity) {
            let failure = CanvasFailure::new(CanvasFailureStatus::Unavailable);
            return self.answer(identity, request.failure_reply(&failure)?, claim);
        }
        if self.editing.subscriber != Some(identity) {
            self.editing = Editing { subscriber: Some(identity), ..Editing::default() };
        }
        if let Err(cause) = session.edit(request.command()) {
            let failure = match cause.downcast_ref::<EditRejection>() {
                Some(EditRejection::Unsupported) => {
                    CanvasFailure::new(CanvasFailureStatus::Unsupported)
                }
                Some(EditRejection::Unavailable(detail)) => {
                    failure(CanvasFailureStatus::Unavailable, detail)
                }
                None => {
                    tracing::warn!(operation = "canvas_edit_command", cause_chain = %crate::diagnostics::CauseChain(cause.as_ref()), "Edit command failed");
                    failure(CanvasFailureStatus::Error, &cause.to_string())
                }
            };
            return self.answer(identity, request.failure_reply(&failure)?, claim);
        }
        let state = self.report_edit_state(session.edit_state()?)?;
        self.answer(identity, request.state_reply(&state)?, claim)
    }

    /// Notify the subscriber once the editing state differs from what it last received, at
    /// most once per frame.
    pub(super) fn poll_editing(&mut self, session: &Session) -> Result<(), Box<dyn Error>> {
        let Some(identity) = self.editing.subscriber.filter(|identity| self.available(*identity))
        else {
            return Ok(());
        };
        let candidate = self.protocol_edit_state(session.edit_state()?, 0)?;
        self.editing.pending = !self.editing.reported.as_ref().is_some_and(|reported| {
            CanvasEditState { revision: reported.revision, ..candidate.clone() } == *reported
        });
        let now = session.preview_now();
        if !self.editing.pending || self.editing.due().is_some_and(|due| now.as_nanos() < due) {
            return Ok(());
        }
        let Some(memory) = session.preview_resident() else { return Ok(()) };
        let claim = match memory.claim(CanvasNativeNotification::STORAGE_BYTES) {
            Ok(claim) => claim,
            Err(cause) => {
                tracing::warn!(operation = "canvas_edit_notification", cause_chain = %crate::diagnostics::CauseChain(&cause), "Edit state notification admission is unavailable");
                return Ok(());
            }
        };
        let state = self.report_edit_state_protocol(candidate)?;
        let payload = state.changed(&PanelSession::for_main(identity)?)?;
        self.editing.notification = Some(DesktopStartupCommand::SendCanvasNotification(
            CanvasNativeNotification::new(identity, payload, claim)?,
        ));
        self.editing.notified = Some(now);
        self.editing.pending = false;
        Ok(())
    }

    /// When a changed state held back by the frame interval may be sent.
    pub(in crate::startup::canvas) fn editing_deadline(&self, now: Moment) -> Option<Instant> {
        let due = self.editing.due().filter(|_| self.editing.pending)?;
        Instant::now().checked_add(Duration::from_nanos(due.saturating_sub(now.as_nanos())))
    }

    /// Record `state` as reported, with a new revision when it changed.
    fn report_edit_state(&mut self, state: EditState) -> Result<CanvasEditState, Box<dyn Error>> {
        let state = self.protocol_edit_state(state, 0)?;
        self.report_edit_state_protocol(state)
    }

    fn report_edit_state_protocol(
        &mut self,
        mut state: CanvasEditState,
    ) -> Result<CanvasEditState, Box<dyn Error>> {
        let previous = self.editing.reported.as_ref();
        let revision = previous.map_or(0, |reported| reported.revision);
        state.revision = if previous
            .is_some_and(|reported| CanvasEditState { revision, ..state.clone() } == *reported)
        {
            revision
        } else {
            revision.checked_add(1).ok_or("Edit state revision exhausted")?
        };
        self.editing.reported = Some(state.clone());
        Ok(state)
    }

    /// The protocol form of `state`: protocol identities, whole-percent options and selection
    /// bounds in CSS pixels of the current allocation.
    fn protocol_edit_state(
        &self,
        state: EditState,
        revision: u64,
    ) -> Result<CanvasEditState, Box<dyn Error>> {
        let scale = self
            .binding
            .as_ref()
            .and_then(|binding| binding.allocation)
            .map(|allocation| allocation.physical_pixels_per_css_pixel)
            .filter(|scale| scale.is_finite() && *scale > 0.0);
        let selection = state.selection.zip(scale).and_then(|(bounds, scale)| {
            let [left, top, width, height] = bounds.map(|value| value / scale);
            CanvasEditSelectionBounds::new(left, top, width, height).ok()
        });
        Ok(CanvasEditState {
            revision,
            document: state.document.map(document_id).transpose()?,
            active_mask: state.active_mask.map(layer_key).transpose()?,
            tool: state.tool,
            tools: state
                .tools
                .iter()
                .map(|&(id, available)| CanvasEditTool { id, available })
                .collect(),
            brush: brush(state.brush)?,
            eraser: brush(state.eraser)?,
            selection,
            undo: state.history[0],
            redo: state.history[1],
            session: state.session.map(|session| CanvasEditSession {
                kind: match session.kind {
                    SessionKind::Move => CanvasEditSessionKind::Move,
                    SessionKind::Transform => CanvasEditSessionKind::Transform,
                    SessionKind::SharedEdges => CanvasEditSessionKind::SharedEdges,
                    SessionKind::Modify => CanvasEditSessionKind::Modify,
                },
                undo: session.undo,
                redo: session.redo,
            }),
            mode: if state.coverage_mode { CanvasEditMode::Coverage } else { CanvasEditMode::None },
        })
    }
}

#[cfg(test)]
mod tests;
