// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Snapshot closed collection stages and retained typed causes without changing custody.
use super::*;
use std::error::Error;

/// Last entered original collection operation; cleanup never overwrites this stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production) enum CollectionStage {
    NotStarted,
    ValidateInput,
    Memory,
    Storage,
    Operation,
    Repository,
    Namespace,
    Writer,
    ValidateStore,
    PrepareCapture,
    OpenCapture,
    PrepareDisposition,
    Preflight,
    BeginPublish,
    CopyPayload,
    ObserveEvidence,
    Commit,
    PublishJournal,
    SettleJournal,
    ObserveCommitted,
    Committed,
}
/// Inline passive diagnostic values; Debug invokes no runtime cause or payload formatter.
#[derive(Clone, Copy, Debug)]
pub(in crate::production) struct CollectionDiagnostic {
    pub(in crate::production) stage: CollectionStage,
    pub(in crate::production) store: Option<IncidentStoreFailureCode>,
    pub(in crate::production) storage: Option<DiagnosticStorageFailureCode>,
    pub(in crate::production) native_stage: Option<&'static str>,
    pub(in crate::production) native_win32_code: Option<u32>,
    pub(in crate::production) report: Option<FinishedFailureDiagnostic>,
    pub(in crate::production) report_clock_failed: bool,
    pub(in crate::production) journal: Option<FinishedFailureDiagnostic>,
    pub(in crate::production) journal_storage: Option<DiagnosticStorageFailureCode>,
    pub(in crate::production) journal_post_panicked: bool,
    pub(in crate::production) cleanup: Option<IncidentStoreFailureCode>,
    pub(in crate::production) decision: Option<IncidentDecisionError>,
    pub(in crate::production) memory_requested_bytes: Option<u64>,
    pub(in crate::production) memory_available_bytes: Option<u64>,
    pub(in crate::production) committed: bool,
    pub(in crate::production) retired: bool,
}
impl CollectionOperation {
    /// Borrow only passive typed values while all actual errors and native outputs stay retained.
    pub(in crate::production) fn diagnostic(&self) -> CollectionDiagnostic {
        let storage = self.first.as_ref().and_then(|cause| match cause {
            IncidentStoreError::Storage(cause) => Some(cause),
            _ => None,
        });
        let native = storage
            .and_then(Error::source)
            .and_then(|cause| cause.downcast_ref::<sugarsubstitute_supervision::ClaimError>());
        CollectionDiagnostic {
            stage: self.stage,
            store: self.first.as_ref().map(IncidentStoreError::code),
            storage: storage.map(DiagnosticStorageError::code),
            native_stage: native.map(sugarsubstitute_supervision::ClaimError::operation),
            native_win32_code: native.and_then(sugarsubstitute_supervision::ClaimError::win32_code),
            report: self.report.as_ref().and_then(CapturedReport::failure_diagnostic),
            report_clock_failed: self
                .report
                .as_ref()
                .is_some_and(|report| report.clock_failure().is_some()),
            journal: self.report.as_ref().and_then(CapturedReport::journal_failure_diagnostic),
            journal_storage: self.report.as_ref().and_then(CapturedReport::journal_storage_failure),
            journal_post_panicked: self
                .report
                .as_ref()
                .is_some_and(CapturedReport::journal_post_panicked),
            cleanup: self.retirement.as_ref().map(IncidentStoreError::code),
            decision: self.decision,
            memory_requested_bytes: self.memory.map(|cause| cause.requested_bytes),
            memory_available_bytes: self.memory.map(|cause| cause.available_bytes),
            committed: self.committed.is_some(),
            retired: self.retired,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn collection_projection_keeps_actual_stage_private_codec_and_original_cause_borrowed()
    -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut operation = CollectionOperation::new();
        let cause = serde_json::from_slice::<serde_json::Value>(b"private-provider-payload")
            .err()
            .ok_or("invalid fixture accepted")?;
        operation.stage = CollectionStage::ValidateStore;
        operation.first = Some(IncidentStoreError::Codec(cause));
        let original = std::ptr::from_ref(operation.failure().ok_or("original cause missing")?);
        let diagnostic = operation.diagnostic();
        assert_eq!(diagnostic.stage, CollectionStage::ValidateStore);
        assert_eq!(diagnostic.store, Some(IncidentStoreFailureCode::Codec));
        assert!(diagnostic.report.is_none());
        assert!(diagnostic.storage.is_none());
        assert!(!format!("{diagnostic:?}").contains("private-provider"));
        assert_eq!(operation.failure().map(std::ptr::from_ref), Some(original));
        assert!(!operation.releasable());
        Ok(())
    }
}
