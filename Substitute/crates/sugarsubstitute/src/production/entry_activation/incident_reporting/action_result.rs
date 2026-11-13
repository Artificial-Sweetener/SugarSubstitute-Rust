// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Return a current authenticated copy authorization through the same original relay turn.
use super::*;
impl IncidentReporting {
    /// Reject this authenticated restart request before policy acknowledgement or revision mutation.
    /// The same original frame/token, finite cutoff and completion remain owned until real delivery.
    pub(super) fn reject_restart(
        &mut self,
        binding: ReportBinding,
        relay_until: Moment,
    ) -> Result<()> {
        if self.pending_control.is_some() || self.binding != Some(binding) {
            return Err("Original restart rejection control remains occupied or foreign".into());
        }
        let original = self.request.as_ref().ok_or("Original restart request missing")?;
        let key = self.key.ok_or("Actual Reporter key missing")?;
        if original.authentication.received.key() != key {
            return Err("Original restart request source changed".into());
        }
        let until = relay_until.min(original.context.budget().until());
        self.pending_control = Some(ReporterControlInput {
            key,
            request: original.authentication.issued.request(),
            context: DiagnosticContext {
                incident: Some(DiagnosticIncident {
                    key: binding.incident(),
                    revision: binding.revision(),
                }),
                ..DiagnosticContext::default()
            },
            session: binding.session(),
            until,
            command: ReporterControl::RestartUnavailable,
        });
        self.relay_until = Some(until);
        Ok(())
    }
    /// Admit the native clipboard decision only after the actual policy accepted its binding.
    /// The original received frame and issued token remain held for router completion.
    /// # Errors
    /// Full slots or a foreign session leave the unchanged original request retained.
    pub(super) fn authorize_copy(
        &mut self,
        binding: ReportBinding,
        relay_until: Moment,
    ) -> Result<()> {
        if self.pending_control.is_some() || self.binding != Some(binding) {
            return Err("Original copy authorization control remains occupied or foreign".into());
        }
        let request = self.request.as_ref().ok_or("Original copy request missing")?;
        let key = self.key.ok_or("Actual Reporter key missing")?;
        if request.authentication.received.key() != key {
            return Err("Original copy request source changed".into());
        }
        let until = relay_until.min(request.context.budget().until());
        self.pending_control = Some(ReporterControlInput {
            key,
            request: request.authentication.issued.request(),
            context: DiagnosticContext {
                incident: Some(DiagnosticIncident {
                    key: binding.incident(),
                    revision: binding.revision(),
                }),
                ..DiagnosticContext::default()
            },
            session: binding.session(),
            until,
            command: ReporterControl::Copy(DiagnosticActionResult::Success),
        });
        self.relay_until = Some(until);
        Ok(())
    }
}
