// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Separate expected receiving failures from original supervised worker and source failures.
use super::*;
use sugarsubstitute_crash::report_export::ReportExportCode;
impl ExportService {
    /// Compare only the original authenticated operation/binding retained before native effects.
    pub(in crate::production) fn matches_original(
        &self,
        operation: LogicalStoreOperationKey,
        binding: ReportBinding,
    ) -> bool {
        self.source.inspect(|source| source.correlation == Some((operation, binding))) == Some(true)
    }
    /// Borrow the original supervised completion, including its unchanged panic cause.
    /// Cancellation, timeout and worker failure cannot authorize a failed-save completion.
    pub(in crate::production) fn inspect_execution_failure<T>(
        &self,
        observe: impl FnOnce(&JobCompletion<bool, ExportIssue>) -> T,
    ) -> Option<T> {
        self.delivery
            .iter()
            .filter_map(|delivery| delivery.completion.as_ref())
            .find(|done| !matches!(done.outcome, JobOutcome::Success(_)))
            .map(observe)
    }
    /// Accept only an actual receiving destination/consent/OS failure after both checked closes.
    /// Authentication, source identity, control, worker failure and uncertain cleanup stay strict.
    pub(in crate::production) fn expected_receiving_failure(&self) -> bool {
        if self.delivery_error.is_some()
            || self.inspect_execution_failure(|_| ()).is_some()
            || !self.checked_terminal()
            || !self.delivery[..2].iter().all(|delivery| {
                matches!(
                    delivery.completion.as_ref().map(|done| &done.outcome),
                    Some(JobOutcome::Success(false))
                )
            })
        {
            return false;
        }
        self.source.inspect(|operation| {
            operation.receiving_failure.is_none()
                && operation.source_failure.is_none()
                && (operation.empty_destination_rejection()
                    || matches!(operation.first, Some(ExportCause::State))
                        && operation
                            .export
                            .as_ref()
                            .and_then(ReportExportOperation::original_failure)
                            .is_some_and(|cause| {
                                matches!(
                                    cause,
                                    ReportExportCause::Native(_)
                                        | ReportExportCause::Rejected(
                                            ReportExportCode::Destination
                                                | ReportExportCode::Consent
                                        )
                                )
                            }))
        }) == Some(true)
    }
    /// Observe original retained causes and receiving custody without transferring native rights.
    pub(in crate::production) fn inspect_failure<T>(
        &self,
        observe: impl FnOnce(
            Option<&(dyn std::error::Error + 'static)>,
            Option<sugarsubstitute_crash::report_export::ReportExportCustody>,
        ) -> T,
    ) -> Option<T> {
        self.source.inspect(|operation| {
            let execution = self.delivery.iter().find(|delivery| delivery.unexpected());
            let failure: Option<&(dyn std::error::Error + 'static)> = if let Some(delivery) =
                execution
            {
                Some(delivery)
            } else if let Some(issue) = self.delivery_error.as_ref() {
                Some(issue)
            } else if operation
                .export
                .as_ref()
                .is_some_and(|export| export.original_failure().is_some())
            {
                operation.export.as_ref().map(|export| export as &(dyn std::error::Error + 'static))
            } else if operation.dump_reader.as_ref().is_some_and(|reader| {
                reader.failure().is_some() || reader.retirement_failure().is_some()
            }) {
                operation
                    .dump_reader
                    .as_ref()
                    .map(|reader| reader as &(dyn std::error::Error + 'static))
            } else {
                operation
                    .first
                    .as_ref()
                    .or(operation.receiving_failure.as_ref())
                    .or(operation.source_failure.as_ref())
                    .map(|cause| cause as &(dyn std::error::Error + 'static))
            };
            observe(failure, operation.export.as_ref().map(ReportExportOperation::custody))
        })
    }
}
