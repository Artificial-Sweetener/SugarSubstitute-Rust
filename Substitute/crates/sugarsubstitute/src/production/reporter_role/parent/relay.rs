// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Run finite channel commands while retaining authenticated and rejected original custody.
use super::*;
impl ReporterParent {
    /// Decode on a genuine finite worker; complete original received guards move with the result.
    pub(in crate::production) fn receive(
        &mut self,
    ) -> Result<
        Option<(
            sugarsubstitute_crash::diagnostic_client::ReceivedDiagnosticRequest,
            sugarsubstitute_crash::diagnostic_client::DiagnosticAdmission,
        )>,
        ReporterIssue,
    > {
        if self.failure_transferred {
            return Err(ReporterIssue::Transport);
        }
        self.channel.receive().map_err(|_| ReporterIssue::Wire)
    }
    /// Save the exact observed terminal receipt without consuming rejected request custody.
    /// Completion grants no durable acknowledgement or native-family settlement.
    pub(in crate::production) fn complete(
        &mut self,
        issued: &sugarsubstitute_crash::diagnostic_client::IssuedDiagnosticRequest,
        reply: sugarsubstitute_crash::diagnostic_client::DiagnosticReply,
    ) -> Result<(), ReporterIssue> {
        if self.failure_transferred {
            return Err(ReporterIssue::Transport);
        }
        self.channel.complete(issued, reply).map_err(|_| ReporterIssue::Wire)
    }
    pub(in crate::production) fn continue_read(&mut self) -> Result<(), ReporterIssue> {
        if self.failure_transferred {
            return Err(ReporterIssue::Transport);
        }
        self.channel.continue_read().map_err(|_| ReporterIssue::Transport)
    }
    /// Encode and submit only from the real finite Core worker, using the original session allocation.
    pub(in crate::production) fn send(
        &mut self,
        until: Moment,
        frame: sugarsubstitute_crash::diagnostic_client::DiagnosticFrame<'_>,
    ) -> Result<Option<LifecyclePipeOperationId>, ReporterIssue> {
        if self.failure_transferred || !self.authenticated() || self.send_error.is_some() {
            return Err(ReporterIssue::Transport);
        }
        let Some(buffer) = self.channel.session.control().take_write_buffer().map_err(|cause| {
            self.channel.first.get_or_insert(cause);
            ReporterIssue::Transport
        })?
        else {
            return Ok(None);
        };
        let operation = match self.channel.next() {
            Ok(operation) => operation,
            Err(cause) => {
                self.channel.first.get_or_insert(cause);
                self.rejected_output = Some(buffer);
                return Err(ReporterIssue::Transport);
            }
        };
        match sugarsubstitute_crash::diagnostic_client::send_diagnostic(
            self.channel.session.control(),
            operation,
            until,
            buffer,
            frame,
        ) {
            Ok(_) => Ok(Some(operation)),
            Err(cause) => {
                self.send_error = Some(cause.error);
                self.rejected_output = Some(cause.buffer);
                Err(ReporterIssue::Wire)
            }
        }
    }
    pub(in crate::production) fn take_written(
        &mut self,
    ) -> Option<sugarsubstitute_execution::purpose_pipe::LifecyclePipeOperationId> {
        self.channel.written.take()
    }
}
