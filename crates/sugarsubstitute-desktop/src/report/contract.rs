// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Expose the one scoped native report lifetime and inert user-selection observations.
use super::{NativeReportView, ReportSurfaceError, ReportSurfaceFailure};
use std::{
    num::{NonZeroU64, NonZeroUsize},
    sync::Arc,
};
use sugarsubstitute_contracts::{
    execution::{CompletionWake, MemoryClaim},
    incident::{
        DurableAcknowledgement, ExportSelection, ReportBinding, ReportCommand, ReportFailure,
        ReportSessionId,
    },
};
/// Positive native-surface action identity; assigned without wrapping within the session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReportRequestId(pub(super) NonZeroU64);
impl ReportRequestId {
    /// Return positive correlation data, never a native capability.
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}
/// Actual bounded save-selection data, separately validated by the finite Storage helper.
pub struct SaveSelection {
    pub(super) request: ReportRequestId,
    pub(super) binding: ReportBinding,
    pub(super) destination: Vec<u16>,
    pub(super) content: ExportSelection,
    pub(super) overwrite_consented: bool,
    pub(super) _claim: MemoryClaim,
}
impl SaveSelection {
    /// Maximum selected destination length in UTF16 code units, excluding terminator.
    pub const MAX_UNITS: usize = 1024;
    /// Return the actual originating native request.
    pub const fn request(&self) -> ReportRequestId {
        self.request
    }
    /// Return the exact visible view at selection time.
    pub const fn binding(&self) -> ReportBinding {
        self.binding
    }
    /// Borrow selected destination data; this proves no native filesystem identity.
    pub fn destination(&self) -> &[u16] {
        &self.destination
    }
    /// Return TextOnly default or actual explicit sensitive-memory consent.
    pub const fn content(&self) -> ExportSelection {
        self.content
    }
    /// Return actual selected-name overwrite consent, not a file predecessor proof.
    pub const fn overwrite_consented(&self) -> bool {
        self.overwrite_consented
    }
}
/// Actual UI facts require separate peer authentication and host proof validation.
pub enum ReportSurfaceObservation {
    /// Actual native text/controls/window submission succeeded.
    Presented(ReportBinding),
    /// Exact same-session view and native controls actually replaced the old revision.
    ViewReplaced(ReportBinding),
    /// Actual clipboard transfer completed or failed, without acknowledgement.
    CopyFinished {
        /// Original surface request.
        request: ReportRequestId,
        /// Exact requested view.
        binding: ReportBinding,
        /// Actual transfer outcome.
        outcome: Result<(), ReportSurfaceFailure>,
    },
    /// Actual save picker selection plus explicit content/overwrite consent.
    SaveSelected(SaveSelection),
    /// Native user cancellation; no export operation has started.
    SaveCancelled {
        /// Original surface request.
        request: ReportRequestId,
        /// Exact requested view.
        binding: ReportBinding,
    },
    /// Actual UI resource/callback retirement; process/pipe/family retirement remains separate.
    NativeRetired(ReportSessionId),
    /// Preserve categorical UI failure independently of retained native custody.
    Failed {
        /// Current matching view.
        binding: ReportBinding,
        /// Actual category; original cause remains with this native owner.
        failure: ReportFailure,
    },
}
/// One bounded content decision applied on the surface's creating main thread.
pub enum ReportSurfaceUpdate {
    /// No new native operation.
    None,
    /// Replace actual text and bindings without launching another reporter.
    ReplaceView {
        /// Exact currently displayed view.
        old: ReportBinding,
        /// Exact newly committed view.
        current: ReportBinding,
        /// Charged immutable successor text.
        view: NativeReportView,
    },
    /// Resolve the current admitted surface action; stale/foreign outcomes reject.
    ActionResult {
        /// Original request.
        request: ReportRequestId,
        /// Exact requesting view.
        binding: ReportBinding,
        /// Host validation outcome. For CopyText, success permits the actual clipboard
        /// transfer, whose separate CopyFinished observation carries its native outcome.
        /// This content decision proves neither native completion nor durable mutation.
        outcome: Result<(), ReportSurfaceFailure>,
    },
    /// Authenticated composing host accepted an actual durable acknowledgement.
    CloseAfterDurableAck {
        /// Exact requested current view.
        binding: ReportBinding,
        /// Plain matching facts projected only from actual commit by the host.
        acknowledgement: DurableAcknowledgement,
    },
}
/// Coordinate one reporter's native lifetime without browser or backend dependencies.
/// Every method runs on the native main thread and returns promptly. The existing
/// execution owner handles jobs; cancellation never proves native/process settlement.
pub trait ReportSessionContent {
    /// Start once after a thread-safe native completion wake is available.
    /// # Errors
    /// Retain original cause and remain shutdown-capable after partial initialization.
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), ReportSurfaceError>;
    /// Drain at most min(maximum,32) decisions; native resources remain with the surface.
    /// # Errors
    /// Preserve causes while closing admission and continuing owned teardown.
    fn poll(&mut self, maximum: NonZeroUsize) -> Result<ReportSurfaceUpdate, ReportSurfaceError>;
    /// Accept one currently fenced native Copy/Dismiss/Restart intent with request identity.
    /// Export is admitted through actual SaveSelected instead of a direct click command.
    /// # Errors
    /// Preserve rejection without claiming acknowledgement or stopping actual work.
    fn action(
        &mut self,
        request: ReportRequestId,
        command: ReportCommand,
    ) -> Result<(), ReportSurfaceError>;
    /// Consume actual presentation, selection, replacement or UI retirement facts.
    /// # Errors
    /// Preserve original host cause; invalid facts cannot authorize native effects.
    fn observe(&mut self, observation: ReportSurfaceObservation) -> Result<(), ReportSurfaceError>;
    /// Close new admission and request existing owner shutdown without joining on UI.
    /// # Errors
    /// Preserve the cause while remaining pollable through actual settlement.
    fn request_shutdown(&mut self) -> Result<(), ReportSurfaceError>;
    /// Observe actual retained content transport/jobs/terminal disposal settlement.
    /// # Errors
    /// Preserve cleanup failures and keep polling; true is never deadline inference.
    fn poll_shutdown(&mut self) -> Result<bool, ReportSurfaceError>;
    /// Finish after actual UI retirement and wake/event-loop disconnection.
    /// # Errors
    /// Preserve final teardown failure without inventing process/family authority.
    fn finish_shutdown(&mut self) -> Result<(), ReportSurfaceError>;
}
/// Actual native surface receipt; construction grants no pipe/process/durable authority.
pub struct ReportSurfaceReceipt {
    pub(super) session: ReportSessionId,
    pub(super) presented: bool,
    pub(super) replacements: u64,
    pub(super) requests: u64,
}
impl ReportSurfaceReceipt {
    /// Return the actual independently retired UI session correlation.
    pub const fn session(&self) -> ReportSessionId {
        self.session
    }
    /// Return whether actual initial presentation succeeded.
    pub const fn presented(&self) -> bool {
        self.presented
    }
    /// Return actual successful same-session replacements.
    pub const fn replacements(&self) -> u64 {
        self.replacements
    }
    /// Return admitted actual native intents, excluding rejected stale input.
    pub const fn requests(&self) -> u64 {
        self.requests
    }
}
