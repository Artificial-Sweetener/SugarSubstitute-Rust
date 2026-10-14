// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Match authenticated control kinds before resolving the original native action.
use super::*;
impl ReporterSession {
    pub(super) fn control(&mut self, control: ControlResult) -> Result<(), ReportSurfaceError> {
        if self.rejected_control.is_some() {
            return Err(Self::rejected("reporter_rejected_control_retained"));
        }
        let update = (|| {
            let (request, command, wire) =
                self.pending.ok_or_else(|| Self::rejected("reporter_unrequested_result"))?;
            control.validate(command, wire)?;
            let binding = command.binding();
            let outcome = |result| match result {
                DiagnosticActionResult::Success => Ok(()),
                DiagnosticActionResult::Failed(_) => Err(ReportSurfaceFailure::Store),
            };
            Ok(match &control.decision {
                ControlDecision::Copy(result) => ReportSurfaceUpdate::CopyAuthorized {
                    request,
                    binding,
                    outcome: outcome(*result),
                    counter: control
                        .counter
                        .ok_or_else(|| Self::rejected("reporter_copy_counter_missing"))?,
                },
                ControlDecision::Export(result) => ReportSurfaceUpdate::ActionResult {
                    request,
                    binding,
                    outcome: outcome(*result),
                },
                ControlDecision::Close(acknowledgement) => {
                    ReportSurfaceUpdate::CloseAfterDurableAck {
                        binding,
                        acknowledgement: *acknowledgement,
                    }
                }
            })
        })();
        match update {
            Ok(update) => {
                self.update = Some(update);
                self.pending = None;
                Ok(())
            }
            Err(cause) => {
                self.rejected_control = Some(control);
                Err(cause)
            }
        }
    }
}
impl ControlResult {
    fn validate(
        &self,
        command: ReportCommand,
        wire: DiagnosticRequestId,
    ) -> Result<(), ReportSurfaceError> {
        let binding = command.binding();
        if self.request != wire
            || self.session != binding.session()
            || self.incident.key != binding.incident()
        {
            return Err(ReporterSession::rejected("reporter_result_identity"));
        }
        match (&self.decision, command.action()) {
            (ControlDecision::Copy(_), ReportAction::CopyText) => {
                if self.counter.is_none() {
                    return Err(ReporterSession::rejected("reporter_copy_counter_missing"));
                }
            }
            (ControlDecision::Export(_), ReportAction::Export(_)) => {}
            (
                ControlDecision::Close(acknowledgement),
                ReportAction::Dismiss | ReportAction::DismissAndRequestRestart,
            ) => {
                if acknowledgement.incident() != self.incident.key
                    || acknowledgement.revision() != self.incident.revision
                    || acknowledgement.revision() <= binding.revision()
                {
                    return Err(ReporterSession::rejected("reporter_close_ack_header"));
                }
                return Ok(());
            }
            _ => return Err(ReporterSession::rejected("reporter_result_action_kind")),
        }
        if self.incident.revision != binding.revision() {
            return Err(ReporterSession::rejected("reporter_result_revision"));
        }
        Ok(())
    }
}
#[cfg(test)]
#[path = "control/tests.rs"]
mod tests;
