// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare complete charged authenticated report text on the execution worker before presentation.
use std::{collections::TryReserveError, error::Error, fmt};
use sugarsubstitute_contracts::{execution::*, incident::*};
use sugarsubstitute_crash::diagnostic_client::*;
use sugarsubstitute_desktop::report::{NativeReportView, ReportSurfaceError};
struct Text {
    bytes: Vec<u8>,
    claim: MemoryClaim,
}
pub(in crate::production) struct ViewReceiver {
    transfer: Option<DiagnosticTransfer>,
    text: Option<Text>,
    visible: Option<ReportBinding>,
    pending: Option<ReportBinding>,
    replacing: bool,
    error_claim: Option<MemoryClaim>,
    rejection: Option<ViewRejection>,
    funding: MemoryLease,
    owner: ExecutionId,
}
#[derive(Debug)]
pub(in crate::production) enum ViewFailure {
    Wire(DiagnosticWireError),
    Memory(MemoryClaimError),
    Allocation(TryReserveError),
    Text(std::string::FromUtf8Error),
    Clock(sugarsubstitute_execution::windows::NativeObservationError),
}
impl fmt::Display for ViewFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Reporter view preparation failed")
    }
}
impl Error for ViewFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Wire(cause) => cause,
            Self::Memory(cause) => cause,
            Self::Allocation(cause) => cause,
            Self::Text(cause) => cause,
            Self::Clock(cause) => cause,
        })
    }
}
/// Preserve the complete actual received guard on every semantic/allocation failure.
pub(in crate::production) struct ViewRejection {
    pub(in crate::production) cause: Option<ViewFailure>,
    _request: ReceivedDiagnosticRequest,
    _text: Option<MemoryClaim>,
    _original: MemoryLease,
}
/// Keep the unchanged worker cause ahead of its original allocation funding.
#[derive(Debug)]
pub(super) struct OwnedViewFailure {
    cause: ViewFailure,
    _text: Option<MemoryClaim>,
    _original: MemoryLease,
}
impl OwnedViewFailure {
    /// Transfer one original source/clock failure with its finite worker funding.
    pub(super) fn from_source(cause: ViewFailure, original: MemoryLease) -> Self {
        Self { cause, _text: None, _original: original }
    }
}
impl fmt::Display for OwnedViewFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cause.fmt(f)
    }
}
impl Error for OwnedViewFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}
/// Category only; the complete original request and cause remain in the funded receiver.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production) enum ViewIssue {
    Busy,
    Failed,
}
/// A complete charged view grants no presentation or acknowledgement proof.
pub(in crate::production) enum PreparedView {
    Partial,
    Initial(PreparedText),
    Replacement(PreparedText),
    Control(ControlResult),
}
#[path = "view/control.rs"]
mod control;
pub(super) use control::ControlDecision;
pub(in crate::production) use control::ControlResult;
impl ViewReceiver {
    pub(in crate::production) fn new(context: &JobContext) -> Self {
        Self {
            transfer: None,
            text: None,
            visible: None,
            pending: None,
            replacing: false,
            error_claim: None,
            rejection: None,
            funding: context.memory().clone(),
            owner: context.owner_id(),
        }
    }
    /// The caller submits this method on its actual finite Core worker; no UI callback parses bytes.
    pub(in crate::production) fn apply(
        &mut self,
        request: &mut Option<ReceivedDiagnosticRequest>,
        context: &JobContext,
    ) -> Result<PreparedView, ViewIssue> {
        if self.rejection.is_some() {
            return Err(ViewIssue::Busy);
        }
        let Some(borrowed) = request.as_ref() else {
            return Err(ViewIssue::Busy);
        };
        match self.apply_bytes(borrowed, context) {
            Ok(view) => {
                drop(request.take());
                Ok(view)
            }
            Err(cause) => {
                if let Some(request) = request.take() {
                    self.rejection = Some(ViewRejection {
                        cause: Some(cause),
                        _request: request,
                        _text: self.error_claim.take(),
                        _original: context.memory().clone(),
                    });
                }
                Err(ViewIssue::Failed)
            }
        }
    }
    /// Borrow the original worker cause without detaching the complete received loan.
    pub(in crate::production) fn failure(&self) -> Option<&ViewFailure> {
        self.rejection.as_ref().and_then(|failure| failure.cause.as_ref())
    }
    /// Transfer the exact cause once; its received loan remains owned until checked teardown.
    pub(super) fn take_failure(&mut self) -> Option<OwnedViewFailure> {
        let failure = self.rejection.as_mut()?;
        Some(OwnedViewFailure {
            cause: failure.cause.take()?,
            _text: failure._text.take(),
            _original: failure._original.clone(),
        })
    }
    fn apply_bytes(
        &mut self,
        request: &ReceivedDiagnosticRequest,
        context: &JobContext,
    ) -> Result<PreparedView, ViewFailure> {
        let frame = request.decode().map_err(ViewFailure::Wire)?;
        self.apply_frame(frame, context)
    }
    fn apply_frame(
        &mut self,
        frame: DiagnosticFrame<'_>,
        context: &JobContext,
    ) -> Result<PreparedView, ViewFailure> {
        if let Some(control) = ControlResult::from_frame(frame)? {
            return Ok(PreparedView::Control(control));
        }
        let binding = DiagnosticTransferBinding::from_frame(&frame).map_err(ViewFailure::Wire)?;
        match frame.command {
            DiagnosticCommand::BeginView { session, total, sha256 } => {
                if self.visible.is_some() {
                    return Err(ViewFailure::Wire(DiagnosticWireError::Stale));
                }
                self.begin(binding, session, total, sha256, false, context)?;
                Ok(PreparedView::Partial)
            }
            DiagnosticCommand::ReplaceView { session, old, total, sha256 } => {
                let current = self.visible.ok_or(ViewFailure::Wire(DiagnosticWireError::Stale))?;
                let incident =
                    frame.context.incident.ok_or(ViewFailure::Wire(DiagnosticWireError::Shape))?;
                if current.session() != session
                    || current.revision() != old
                    || current.incident() != incident.key
                    || incident.revision <= old
                {
                    return Err(ViewFailure::Wire(DiagnosticWireError::Stale));
                }
                self.begin(binding, session, total, sha256, true, context)?;
                Ok(PreparedView::Partial)
            }
            DiagnosticCommand::ViewChunk { chunk, .. } => {
                let text =
                    self.text.as_mut().ok_or(ViewFailure::Wire(DiagnosticWireError::Incomplete))?;
                if text.bytes.len() as u64 != chunk.offset
                    || text
                        .bytes
                        .len()
                        .checked_add(chunk.bytes.len())
                        .is_none_or(|total| total > text.bytes.capacity())
                {
                    return Err(ViewFailure::Wire(DiagnosticWireError::Capacity));
                }
                self.transfer
                    .as_mut()
                    .ok_or(ViewFailure::Wire(DiagnosticWireError::Incomplete))?
                    .accept(binding, chunk)
                    .map_err(ViewFailure::Wire)?;
                text.bytes.extend_from_slice(chunk.bytes);
                Ok(PreparedView::Partial)
            }
            DiagnosticCommand::EndView { session, sha256 } => {
                self.transfer
                    .as_mut()
                    .ok_or(ViewFailure::Wire(DiagnosticWireError::Incomplete))?
                    .finish(binding, sha256)
                    .map_err(ViewFailure::Wire)?;
                let incident =
                    frame.context.incident.ok_or(ViewFailure::Wire(DiagnosticWireError::Shape))?;
                let view_binding = ReportBinding::new(incident.key, incident.revision, session);
                let bytes = &self
                    .text
                    .as_ref()
                    .ok_or(ViewFailure::Wire(DiagnosticWireError::Incomplete))?
                    .bytes;
                if bytes.is_empty()
                    || bytes.contains(&0)
                    || bytes.contains(&b'\r')
                    || bytes.last() != Some(&b'\n')
                {
                    return Err(ViewFailure::Wire(DiagnosticWireError::Shape));
                }
                let text =
                    self.text.take().ok_or(ViewFailure::Wire(DiagnosticWireError::Incomplete))?;
                self.error_claim = Some(text.claim);
                let text_string = String::from_utf8(text.bytes).map_err(ViewFailure::Text)?;
                let claim =
                    self.error_claim.take().ok_or(ViewFailure::Wire(DiagnosticWireError::Shape))?;
                let view = PreparedText { binding: view_binding, text: text_string, claim };
                self.transfer = None;
                self.pending = Some(view_binding);
                Ok(if self.replacing {
                    PreparedView::Replacement(view)
                } else {
                    PreparedView::Initial(view)
                })
            }
            _ => Err(ViewFailure::Wire(DiagnosticWireError::Shape)),
        }
    }
    fn begin(
        &mut self,
        binding: DiagnosticTransferBinding,
        session: ReportSessionId,
        total: u64,
        sha256: [u8; 32],
        replacing: bool,
        context: &JobContext,
    ) -> Result<(), ViewFailure> {
        if self.transfer.is_some()
            || self.pending.is_some()
            || binding.key().session().get() != session.get()
        {
            return Err(ViewFailure::Wire(DiagnosticWireError::Stale));
        }
        let transfer =
            DiagnosticTransfer::new(binding, DiagnosticTransferKind::View, total, sha256)
                .map_err(ViewFailure::Wire)?;
        if context.owner_id() != self.owner {
            return Err(ViewFailure::Wire(DiagnosticWireError::Identity));
        }
        let claim = self
            .funding
            .claim(NativeReportView::MAX_BYTES as u64 + 512)
            .map_err(ViewFailure::Memory)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(total as usize).map_err(ViewFailure::Allocation)?;
        if bytes.capacity() > NativeReportView::MAX_BYTES {
            return Err(ViewFailure::Wire(DiagnosticWireError::Capacity));
        }
        self.text = Some(Text { bytes, claim });
        self.transfer = Some(transfer);
        self.replacing = replacing;
        Ok(())
    }
    /// Only the actual native surface observation advances the visible binding.
    pub(in crate::production) fn presented(
        &mut self,
        binding: ReportBinding,
    ) -> Result<(), ViewIssue> {
        if self.pending != Some(binding) {
            return Err(ViewIssue::Failed);
        }
        self.visible = Some(binding);
        self.pending = None;
        Ok(())
    }
}

/// Whole UTF8 and its original claim move together into the affine native presentation boundary.
pub(in crate::production) struct PreparedText {
    binding: ReportBinding,
    pub(super) text: String,
    claim: MemoryClaim,
}
impl PreparedText {
    /// Native shape validation consumes already decoded text; this parses no wire on the UI thread.
    pub(in crate::production) fn into_native(self) -> Result<NativeReportView, ReportSurfaceError> {
        NativeReportView::from_claimed(self.binding, self.text, self.claim)
    }
}

#[cfg(test)]
#[path = "view/tests.rs"]
mod tests;
