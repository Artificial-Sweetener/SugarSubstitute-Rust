// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve exact authenticated control metadata separately from charged view transfers.
use super::*;
pub(in crate::production) struct ControlResult {
    pub(in crate::production::reporter_role) request: DiagnosticRequestId,
    pub(in crate::production::reporter_role) incident: DiagnosticIncident,
    pub(in crate::production::reporter_role) session: ReportSessionId,
    pub(in crate::production::reporter_role) decision: ControlDecision,
    pub(in crate::production::reporter_role) counter: Option<CounterDeadline>,
}
pub(in crate::production::reporter_role) enum ControlDecision {
    Copy(DiagnosticActionResult),
    Export(DiagnosticActionResult),
    Close(DurableAcknowledgement),
    RestartUnavailable,
}
impl ControlResult {
    pub(in crate::production::reporter_role) fn from_frame(
        frame: DiagnosticFrame<'_>,
    ) -> Result<Option<Self>, ViewFailure> {
        let control = match frame.command {
            DiagnosticCommand::CopyResult { session, result } => {
                Some((session, ControlDecision::Copy(result)))
            }
            DiagnosticCommand::ExportSelectionResult { session, result } => {
                Some((session, ControlDecision::Export(result)))
            }
            DiagnosticCommand::RestartUnavailable { session } => {
                Some((session, ControlDecision::RestartUnavailable))
            }
            DiagnosticCommand::CloseAfterDurableAck { session, acknowledgement } => {
                Some((session, ControlDecision::Close(acknowledgement)))
            }
            _ => None,
        };
        if let Some((session, decision)) = control {
            return Ok(Some(Self {
                request: frame.request,
                incident: frame
                    .context
                    .incident
                    .ok_or(ViewFailure::Wire(DiagnosticWireError::Shape))?,
                session,
                decision,
                counter: frame
                    .context
                    .deadline
                    .map(|deadline| {
                        CounterDeadline::new(deadline.ticks(), deadline.frequency())
                            .map_err(|_| ViewFailure::Wire(DiagnosticWireError::Time))
                    })
                    .transpose()?,
            }));
        }
        Ok(None)
    }
}
