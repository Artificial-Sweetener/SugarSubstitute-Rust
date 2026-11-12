// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire actual export before the existing genuine Reporter family/pipe disposal boundary.
use super::*;
impl IncidentReporting {
    /// No returned proof authorizes later effects; each cleanup runs with its own active context.
    pub(in crate::production::entry_activation::incident_reporting) fn retire_export(
        &mut self,
        host: &mut IncidentHost,
        resources: &EntryResources<'_>,
        transactions: &mut DiagnosticTransactions,
    ) -> Result<bool> {
        let Some(work) = self.export_work.as_mut() else { return Ok(true) };
        if matches!(work.material, ExportMaterial::Returned { .. }) {
            return Ok(true);
        }
        let Some(IncidentEffect::Export { scope, .. }) = self.retained_effect.as_ref() else {
            return Err("Original retiring export operation missing".into());
        };
        let operation = scope.operation();
        let service =
            if host.closing() { host.retiring_export_service()? } else { host.export_service()? };
        if !work.submitted {
            if work.bound && matches!(work.material, ExportMaterial::Bound) {
                let Some(input) = service.take_unsubmitted_input()? else { return Ok(false) };
                work.material = ExportMaterial::Input(input);
            }
            transactions.retired(operation, &mut work.transaction)?;
            return Ok(true);
        }
        service.cancel();
        match service.advance_cleanup(resources.client) {
            Ok(_) => {}
            Err(ExportIssue::Admission) => return Ok(false),
            Err(cause) => return Err(cause.into()),
        }
        work.wake_until = service.next_deadline();
        if !service.checked_terminal() {
            return Ok(false);
        }
        if matches!(work.material, ExportMaterial::Bound) {
            let Some(terminal) = service.take_terminal() else { return Ok(false) };
            work.material = ExportMaterial::Retiring(terminal);
        }
        transactions.retired(operation, &mut work.transaction)?;
        Ok(true)
    }
    /// Return original incoming tokens only after actual family settlement; keep whole view/custody.
    pub(in crate::production::entry_activation::incident_reporting) fn return_retiring_export_inputs(
        &mut self,
    ) -> Result<()> {
        let Some(work) = self.export_work.as_mut() else { return Ok(()) };
        if matches!(work.material, ExportMaterial::Returned { .. }) {
            return Ok(());
        }
        let material = std::mem::replace(&mut work.material, ExportMaterial::Bound);
        let (view, custody) = match material {
            ExportMaterial::Retiring(terminal) => {
                let (received, _issued, _receipt, view, custody): ExportCompletionParts =
                    terminal.into_completion_parts();
                drop(received);
                (view, Some(custody))
            }
            ExportMaterial::Input(ExportInput { protected, view, received, issued: _issued }) => {
                drop(received);
                drop(protected);
                (view, None)
            }
            ExportMaterial::Selected { original, view } => {
                drop(original);
                (view, None)
            }
            ExportMaterial::Completion { view, custody, .. } => (view, custody),
            ExportMaterial::Bound => {
                return Err("Original export terminal was not actually returned".into());
            }
            ExportMaterial::Returned { view, custody } => (view, custody),
        };
        work.material = ExportMaterial::Returned { view, custody };
        Ok(())
    }
    /// Dispose presentation only after actual pipe settlement; strict original outcomes stay funded.
    pub(in crate::production::entry_activation::incident_reporting) fn dispose_retired_export(
        &mut self,
        host: &mut IncidentHost,
    ) -> Result<()> {
        let Some(work) = self.export_work.as_ref() else { return Ok(()) };
        if !matches!(work.material, ExportMaterial::Returned { .. }) {
            return Err("Original export inputs have not actually returned".into());
        }
        let submitted = work.submitted;
        drop(self.export_work.take());
        let service =
            if host.closing() { host.retiring_export_service()? } else { host.export_service()? };
        if submitted {
            if service.receipt().is_some() || service.expected_receiving_failure() {
                service.release_adopted()?;
            } else {
                tracing::error!(
                    operation = "report_export_original_failure_retained",
                    "Original supervised export outcomes remain retained for final host disposal"
                );
            }
        } else {
            service.release_unsubmitted()?;
        }
        self.retained_effect = None;
        Ok(())
    }
}
