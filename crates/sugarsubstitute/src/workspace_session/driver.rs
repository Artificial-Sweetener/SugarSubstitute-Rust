// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive the real session protocol through typed worker and native-delivery effects.
mod ingress;
mod native;
mod publication;
mod restoration;
#[cfg(test)]
mod tests;
use super::*;
use identity::Identities;
use outbound::Outbound;
use std::{collections::VecDeque, sync::Arc, time::Duration};
use sugarsubstitute_application::workspace_session::{
    CaptureTicket, DurableCheckpoint, WorkspaceSession,
};
use sugarsubstitute_contracts::{
    desktop_session::{CheckpointId, validate_message, wire::*},
    execution::{MemoryLease, Moment},
};

/// One required worker action. The composing root retains it on admission refusal and submits
/// only through its existing execution owner; none of these values creates a task or timer.
#[allow(
    clippy::large_enum_variant,
    reason = "One funded capture transfer stays inline until its existing finite worker admits the owned input; an extra box would allocate before that admission"
)]
pub enum DriverEvent {
    /// Encode verified loaded data for bounded materialization off the UI thread.
    EncodeRestore {
        /// Exact host-owned materialization request.
        request_id: String,
        /// Exact host-owned transfer.
        transfer_id: String,
        /// Original immutable load with its memory custody retained.
        restored: Arc<RestoredWorkspace>,
    },
    /// Complete digest/JSON validation of an assembled capture on a worker.
    DecodeCapture {
        /// Exact original request; another request cannot adopt this result.
        request_id: String,
        /// Complete frozen application generation fence.
        ticket: CaptureTicket,
        /// Commit's transfer identity, checked again by the worker before decoding.
        transfer_id: String,
        /// Ordered bytes and their original independent session reservation.
        transfer: CaptureTransfer,
    },
}
#[cfg(all(windows, any(feature = "native-composition", test)))]
pub(crate) use native::NativeOriginalResolution;
struct Restore {
    request: String,
    transfer: String,
    checkpoint: String,
    revision: u64,
    deadline: Moment,
    bindings: Option<Vec<SessionBinding>>,
    fence: SessionFence,
    sent: bool,
}
struct Capture {
    request: String,
    ticket: CaptureTicket,
    deadline: Moment,
    transfer: Option<CaptureTransfer>,
    processing: bool,
}
/// One selected profile/backend's protocol driver. Native origin/peer/lease and filesystem
/// authority remain with their adapters. This owner does bounded synchronous state work only.
pub struct WorkspaceDriver {
    policy: WorkspaceSession,
    scope: SessionScope,
    origin: String,
    identities: Identities,
    memory: MemoryLease,
    restored: Option<Arc<RestoredWorkspace>>,
    published: Option<Arc<transfer::CapturedWorkspace>>,
    restore: Option<Restore>,
    encoding: Option<(String, String)>,
    replay_retired: bool,
    capture: Option<Capture>,
    outbound: Option<Outbound>,
    messages: VecDeque<SessionMessage>,
    placement: Option<SessionPlacement>,
    placement_sequence: u64,
    media_bindings: Vec<SessionBinding>,
    close_cutoff: Option<Moment>,
    last_ack: Option<(String, String, u64, Vec<SessionBinding>, bool)>,
    originals: native::RecordedOutputs,
}
impl WorkspaceDriver {
    /// Compose verified store results with an independently granted current scope/origin/webview.
    /// `memory` is a dedicated execution reservation for current and incoming checkpoint state,
    /// independent of native panel/canvas transport credit.
    /// # Errors
    /// Refuses malformed identities, foreign checkpoint scope or application initialization.
    pub fn new(
        scope: SessionScope,
        origin: String,
        webview: String,
        restored: RestoredWorkspace,
        memory: MemoryLease,
    ) -> Result<Self, StoreError> {
        let checkpoint = restored.checkpoint.as_ref().map(|(_, checkpoint)| checkpoint);
        if checkpoint.is_some_and(|checkpoint| checkpoint.scope != scope) {
            return Err(StoreError::Protected);
        }
        let durable = match checkpoint {
            Some(checkpoint) => Some(DurableCheckpoint {
                id: CheckpointId::new(checkpoint.id.clone())?,
                revision: checkpoint.revision,
            }),
            None => None,
        };
        let placement = checkpoint.and_then(|checkpoint| checkpoint.placement.clone());
        let mut originals = native::RecordedOutputs::new(&memory)?;
        if let Some(checkpoint) = checkpoint {
            originals.remember(checkpoint)?;
        }
        let mut policy = WorkspaceSession::new(scope.clone(), origin.clone(), webview.clone())
            .map_err(application)?;
        policy.loaded(durable, restored.protected).map_err(application)?;
        let restored =
            checkpoint.is_some_and(|value| !value.entries.is_empty()).then(|| Arc::new(restored));
        Ok(Self {
            policy,
            scope,
            origin,
            identities: Identities::new(webview),
            memory,
            restored,
            published: None,
            restore: None,
            encoding: None,
            replay_retired: false,
            capture: None,
            outbound: None,
            messages: VecDeque::new(),
            placement,
            placement_sequence: 0,
            media_bindings: vec![],
            close_cutoff: None,
            last_ack: None,
            originals,
        })
    }
    /// Bound current/incoming decoded checkpoints and process-local original recognition.
    pub const fn reservation_bytes() -> u64 {
        8 * sugarsubstitute_contracts::desktop_session::MAX_CHECKPOINT_BYTES as u64
            + native::MEMORY_BYTES
    }
    /// Borrow the independently chosen namespace for the current native bootstrap.
    pub fn scope(&self) -> &SessionScope {
        &self.scope
    }
    /// Retain preceding durable data for immutable output verification on the publisher worker.
    pub fn baseline(&self) -> CheckpointBaseline {
        CheckpointBaseline { restored: self.restored.clone(), published: self.published.clone() }
    }
    /// Suspend publication after a permanently incompatible native association is observed.
    /// The composing owner retains its original cause; authored originals remain untouched.
    pub fn protect_originals(&mut self) {
        self.policy.protect_originals();
    }
    /// Borrow the native placement before constructing or revealing the main window.
    pub fn placement(&self) -> Option<&SessionPlacement> {
        self.placement.as_ref()
    }
    /// Borrow the host-granted current webview spelling.
    pub fn webview(&self) -> &str {
        &self.policy.fence().webview
    }
    /// Admit an actual current native placement observation; old sequence values cannot rewind it.
    /// # Errors
    /// Refuses invalid native values and preserves autosave deadline overflow.
    pub fn observe_placement(
        &mut self,
        sequence: u64,
        placement: SessionPlacement,
        now: Moment,
    ) -> Result<(), StoreError> {
        if sequence <= self.placement_sequence {
            return Ok(());
        }
        if !placement.valid() {
            return Err(SessionError::Invalid.into());
        }
        if self.close_cutoff.is_some_and(|cutoff| now < cutoff) {
            self.policy.sampled_during_close(now).map_err(application)?;
        } else {
            self.policy.changed(now).map_err(application)?;
        }
        self.placement_sequence = sequence;
        self.placement = Some(placement);
        Ok(())
    }
    /// Adopt one completed worker encoding for the exact owned restore request.
    /// # Errors
    /// Refuses stale, duplicate or foreign encoding completion before native delivery.
    pub fn encoded_restore(
        &mut self,
        request: &str,
        transfer: &str,
        encoded: EncodedMaterialization,
    ) -> Result<(), StoreError> {
        if self.encoding.as_ref().is_none_or(|owned| owned.0 != request || owned.1 != transfer) {
            return Err(SessionError::Stale.into());
        }
        self.encoding = None;
        if !self.owns_restore(request) {
            return Ok(());
        }
        let restore = self.restore.as_ref().ok_or(SessionError::Stale)?;
        if restore.transfer != transfer || self.outbound.is_some() {
            return Err(SessionError::Stale.into());
        }
        self.outbound = Some(Outbound::new(request.into(), transfer.into(), encoded));
        Ok(())
    }
    /// Borrow the earliest original protocol or autosave cutoff for the existing native pump.
    pub fn deadline(&self) -> Option<Moment> {
        self.policy
            .deadline()
            .filter(|_| self.encoding.is_none())
            .into_iter()
            .chain(self.restore.as_ref().map(|value| value.deadline))
            .chain(self.capture.as_ref().map(|value| value.deadline))
            .min()
    }
    /// Freeze a final capture while the actual page/native peer is retained.
    /// # Errors
    /// Preserves the application's original cutoff and refusal; never renews a shutdown budget.
    pub fn begin_close(&mut self, now: Moment, cutoff: Moment) -> Result<(), StoreError> {
        self.close_cutoff = Some(self.close_cutoff.map_or(cutoff, |previous| previous.min(cutoff)));
        self.policy.begin_close(now, cutoff).map_err(application)
    }
    /// Whether acknowledged final state or the original cutoff permits actual native close.
    pub fn close_ready(&self, now: Moment) -> bool {
        self.policy.close_ready(now)
            && (self.last_ack.as_ref().is_none_or(|ack| ack.4)
                || self.close_cutoff.is_some_and(|cutoff| now >= cutoff))
    }
    /// Abandon only an unsubmitted capture after native retirement; callers first observe any
    /// actual worker terminal before invoking this method.
    /// # Errors
    /// Preserves the application's exact active-ticket transition.
    pub fn abandon_capture(&mut self, now: Moment) -> Result<(), StoreError> {
        if let Some(capture) = self.capture.take() {
            self.policy.finished(&capture.ticket, false, now).map_err(application)?;
        }
        Ok(())
    }
    /// Coalesce independently observed native input changes, including a final close sample.
    /// # Errors
    /// Refuses expired close or unrepresentable quiet deadlines.
    pub fn native_changed(&mut self, now: Moment) -> Result<(), StoreError> {
        if self.close_cutoff.is_some() {
            self.policy.sampled_during_close(now).map_err(application)
        } else {
            self.policy.changed(now).map_err(application)
        }
    }
    /// Produce at most one bounded message, preserving ordering under native backpressure.
    pub fn next_message(&mut self) -> Option<SessionMessage> {
        if let Some(message) = self.messages.pop_front() {
            return Some(message);
        }
        if !self.policy.committed() {
            return None;
        }
        let restore = self.restore.as_mut()?;
        let outbound = self.outbound.as_mut()?;
        let message = outbound.next(&restore.fence);
        if message
            .as_ref()
            .is_some_and(|(_, operation)| matches!(operation, SessionOperation::Materialize { .. }))
        {
            restore.sent = true;
        }
        if message.is_none() {
            self.outbound = None;
        }
        message.map(|(request, operation)| self.message(request, operation))
    }
    /// Keep an unadmitted message at the head; a refused native command cannot silently lose it.
    /// # Errors
    /// Refuses bounded local queue overload explicitly.
    pub fn return_message(&mut self, message: SessionMessage) -> Result<(), StoreError> {
        if self.messages.len() >= 8 {
            return Err(SessionError::TooLarge.into());
        }
        self.messages.push_front(message);
        Ok(())
    }
    /// Produce an explicit rejection without advancing any capture or durable state.
    /// # Errors
    /// Refuses bounded response overload.
    pub fn refused(&mut self, request: String) -> Result<(), StoreError> {
        self.queue(
            request,
            SessionOperation::Error {
                failure: SessionFailure {
                    code: "invalid".into(),
                    retryable: false,
                    entry_id: None,
                    output_id: None,
                },
            },
        )
    }
    fn message(&self, request_id: String, operation: SessionOperation) -> SessionMessage {
        SessionMessage {
            protocol: "sugarcubes.desktop-session".into(),
            version: 1,
            request_id,
            scope: self.scope.clone(),
            origin: self.origin.clone(),
            operation,
        }
    }
    fn queue(&mut self, request: String, operation: SessionOperation) -> Result<(), StoreError> {
        if self.messages.len() >= 8 {
            return Err(SessionError::TooLarge.into());
        }
        self.messages.push_back(self.message(request, operation));
        Ok(())
    }
}
fn application(
    cause: sugarsubstitute_application::workspace_session::WorkspaceError,
) -> StoreError {
    use sugarsubstitute_application::workspace_session::WorkspaceError;
    match cause {
        WorkspaceError::Contract(cause) => StoreError::Contract(cause),
        WorkspaceError::Protected => StoreError::Protected,
        other => StoreError::Storage(io::Error::other(other)),
    }
}

fn same_runtime(left: &SessionBinding, right: &SessionBinding) -> bool {
    left.entry_id == right.entry_id
        && left.runtime_session == right.runtime_session
        && left.generation == right.generation
}
