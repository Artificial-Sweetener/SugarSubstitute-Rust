// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Emit one native result only from the genuine checked terminal and original frame correlation.
use super::*;
impl IncidentReporting {
    /// Reject actual overload only after every admitted unpaired cleanup was cancelled and observed.
    /// Original source/input/view/request stay retained through the ordinary router completion.
    /// This records failed admission, never a native export result or physical retirement proof.
    pub(super) fn complete_rejected_export_control(
        &mut self,
        scope: IncidentOperationScope,
        binding: ReportBinding,
        input: ExportInput,
    ) -> Result<()> {
        let work = self.export_work.as_mut().ok_or("Original rejected export owner missing")?;
        work.material = ExportMaterial::Input(input);
        if work.submitted
            || !work.rejected_admission
            || self.completion.is_some()
            || self.pending_control.is_some()
            || self.binding != Some(binding)
        {
            return Err("Original rejected export disposition changed with inputs retained".into());
        }
        let ExportMaterial::Input(input) = &work.material else {
            return Err("Original rejected export input missing".into());
        };
        if self.key != Some(input.received.key())
            || input.view.as_ref().is_none_or(|view| view.view().binding() != binding)
        {
            return Err("Original rejected export correlation changed with inputs retained".into());
        }
        let control = self::input(
            input.received.key(),
            input.issued.request(),
            binding,
            scope.original().until(),
            DiagnosticActionResult::Failed(ReportFailure::Export),
        );
        let work = self.export_work.as_mut().ok_or("Original rejected export owner missing")?;
        let prior = std::mem::replace(&mut work.material, ExportMaterial::Bound);
        let ExportMaterial::Input(input) = prior else {
            work.material = prior;
            return Err("Original rejected export input changed".into());
        };
        let ExportInput { protected, view, received, issued } = input;
        drop(protected);
        work.material =
            ExportMaterial::Completion { request: issued.request(), view, custody: None };
        work.wake_until = Some(scope.original().until());
        self.relay_until = Some(scope.original().until());
        self.pending_control = Some(control);
        self.completion = Some(ReporterCompletion {
            received,
            issued,
            reply: DiagnosticReply::Failed(DiagnosticOperationFailure::Export),
        });
        Ok(())
    }
    /// Keep whole originals on foreign facts, occupied output slots or policy adoption rejection.
    /// The service admits a terminal only after all four real deliveries and both checked closes.
    pub(super) fn complete_export_control(
        &mut self,
        host: &mut IncidentHost,
        scope: IncidentOperationScope,
        binding: ReportBinding,
    ) -> Result<()> {
        if self.completion.is_some() || self.pending_control.is_some() {
            return Err("Original export output slot occupied with originals retained".into());
        }
        let service = host.export_service()?;
        if !service.checked_terminal()
            || service.receipt().is_none() && !service.expected_receiving_failure()
        {
            Self::observe_export_failure(service);
            return Err("Unexpected original export outcome retained for checked retirement".into());
        }
        let work = self.export_work.as_mut().ok_or("Original export completion owner missing")?;
        let terminal = service.take_terminal().ok_or("Actual checked export terminal missing")?;
        work.material = ExportMaterial::Retiring(terminal);
        let ExportMaterial::Retiring(terminal) = &work.material else {
            return Err("Original export terminal phase changed".into());
        };
        if self.key != Some(terminal.received.key())
            || self.binding != Some(binding)
            || terminal.view.as_ref().is_none_or(|view| view.view().binding() != binding)
            || terminal.receipt.is_some_and(|receipt| receipt.binding != binding)
        {
            return Err(
                "Original export completion correlation changed with originals retained".into()
            );
        }
        let saved = terminal.receipt.is_some();
        let control = self::input(
            terminal.received.key(),
            terminal.issued.request(),
            binding,
            scope.original().until(),
            if saved {
                DiagnosticActionResult::Success
            } else {
                DiagnosticActionResult::Failed(ReportFailure::Export)
            },
        );
        if saved {
            host.adopt_export(scope, binding)?;
        } else {
            host.adopt_export_failure(scope, binding)?;
        }
        let work = self.export_work.as_mut().ok_or("Original export completion owner missing")?;
        let prior = std::mem::replace(&mut work.material, ExportMaterial::Bound);
        let ExportMaterial::Retiring(terminal) = prior else {
            work.material = prior;
            return Err("Original export terminal phase changed with originals retained".into());
        };
        let (received, issued, receipt, view, custody): ExportCompletionParts =
            terminal.into_completion_parts();
        work.material =
            ExportMaterial::Completion { request: issued.request(), view, custody: Some(custody) };
        work.wake_until = Some(scope.original().until());
        self.relay_until = Some(scope.original().until());
        self.pending_control = Some(control);
        self.completion = Some(ReporterCompletion {
            received,
            issued,
            reply: receipt.map_or(
                DiagnosticReply::Failed(DiagnosticOperationFailure::Export),
                |receipt| DiagnosticReply::Completed {
                    bytes: receipt.bytes,
                    sha256: receipt.sha256,
                    revision: Some(receipt.binding.revision()),
                },
            ),
        });
        Ok(())
    }
}
/// Construct inert wire facts from Core-validated originals; this creates no native/result proof.
/// Carry the authenticated incident with no operation or UTC; finite Core encodes the required
/// result counter once from this same original cutoff before sending.
fn input(
    key: DiagnosticKey,
    request: DiagnosticRequestId,
    binding: ReportBinding,
    until: Moment,
    result: DiagnosticActionResult,
) -> ReporterControlInput {
    ReporterControlInput {
        key,
        request,
        context: DiagnosticContext {
            incident: Some(DiagnosticIncident {
                key: binding.incident(),
                revision: binding.revision(),
            }),
            ..DiagnosticContext::default()
        },
        session: binding.session(),
        until,
        command: ReporterControl::Export(result),
    }
}

#[cfg(test)]
#[path = "control/tests.rs"]
mod tests;
