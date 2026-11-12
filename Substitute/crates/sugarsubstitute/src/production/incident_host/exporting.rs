// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the original export controller and checked actual receiving result.
use super::super::diagnostic_helper::exporting::ExportService;
use super::*;
impl IncidentHost {
    /// Borrow the same32KiB export source inside the original controller admission.
    pub(in crate::production) fn export_service(
        &mut self,
    ) -> Result<&mut ExportService, HostIssue> {
        if self.closing {
            return Err(HostIssue::Closed);
        }
        self.retained_export_service()
    }
    /// Borrow existing export custody during actual checked teardown, without new work.
    pub(in crate::production) fn retiring_export_service(
        &mut self,
    ) -> Result<&mut ExportService, HostIssue> {
        if !self.closing {
            return Err(HostIssue::Started);
        }
        self.retained_export_service()
    }
    /// Locate the genuinely retained funded controller across normal and close-only paths.
    fn retained_export_service(&mut self) -> Result<&mut ExportService, HostIssue> {
        match self.completion.as_mut().map(|done| &mut done.outcome) {
            Some(JobOutcome::Success(value)) => Ok(&mut value.policy.exporting),
            _ => Err(HostIssue::Unprepared),
        }
    }
    /// Feed the original export deliveries from the parent's single immutable batch.
    pub(in crate::production) fn capture_export(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<bool, HostIssue> {
        match self.completion.as_mut().map(|done| &mut done.outcome) {
            Some(JobOutcome::Success(value)) => {
                value.policy.exporting.capture(batch).map_err(|_| HostIssue::Identity)
            }
            _ => Ok(false),
        }
    }
    /// Adopt failed admission only after actual unpaired cancellation with original input still retained.
    /// No Main/native factory ran, and no native terminal/receipt or acknowledgement is inferred.
    pub(in crate::production) fn adopt_export_admission_failure(
        &mut self,
        scope: IncidentOperationScope,
        binding: ReportBinding,
    ) -> Result<(), HostIssue> {
        let service = self.export_service()?;
        if !service.admission_rejected_for(scope.operation(), binding)
            || service.checked_terminal()
            || service.receipt().is_some()
            || scope.incident() != binding.incident()
            || scope.expected_revision() != binding.revision()
        {
            return Err(HostIssue::Identity);
        }
        self.observe(IncidentServiceObservation::Failed {
            scope,
            failure: IncidentServiceFailure::Export,
        })
    }
    /// Finish a failed export only after both actual Main and checked cleanup deliveries.
    /// No saved receipt, acknowledgement or revision mutation is inferred.
    pub(in crate::production) fn adopt_export_failure(
        &mut self,
        scope: IncidentOperationScope,
        binding: ReportBinding,
    ) -> Result<(), HostIssue> {
        let service = self.export_service()?;
        if !service.checked_terminal() {
            return Err(HostIssue::Pending);
        }
        if !service.expected_receiving_failure()
            || !service.matches_original(scope.operation(), binding)
        {
            return Err(HostIssue::Identity);
        }
        if service.receipt().is_some()
            || scope.incident() != binding.incident()
            || scope.expected_revision() != binding.revision()
        {
            return Err(HostIssue::Identity);
        }
        self.observe(IncidentServiceObservation::Failed {
            scope,
            failure: IncidentServiceFailure::Export,
        })
    }
    /// Observe export completion only from the actual checked receiving and source receipt.
    pub(in crate::production) fn adopt_export(
        &mut self,
        scope: IncidentOperationScope,
        binding: ReportBinding,
    ) -> Result<(), HostIssue> {
        let service = self.export_service()?;
        if !service.matches_original(scope.operation(), binding) {
            return Err(HostIssue::Identity);
        }
        let receipt = service.receipt().ok_or(HostIssue::Pending)?;
        if receipt.binding != binding
            || scope.incident() != binding.incident()
            || scope.expected_revision() != binding.revision()
        {
            return Err(HostIssue::Identity);
        }
        self.observe(IncidentServiceObservation::ExportFinished { scope, binding })
    }
}
