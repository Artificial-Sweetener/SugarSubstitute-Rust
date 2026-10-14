// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the original Capture source while preparing a finite finished-report transaction.
use super::*;
use crate::incident::{CapturedReport, IncidentStoreError};
use sugarsubstitute_contracts::{crash::CaptureOperation, execution::JobContext};
use sugarsubstitute_supervision::diagnostic_storage::{
    DiagnosticStorage, DiagnosticStorageOperation,
};
/// Borrow the same registered Capture lifetime into an owned finite Storage worker.
/// This capability has no public constructor, path projection or cloning API.
pub struct FinishedCaptureSource {
    shared: Arc<Shared>,
}
impl WindowsCrashCapture {
    /// Retain the actual registered capture source before submitting collection work.
    /// # Errors
    /// Missing or unfinished capture registration cannot issue a finished source.
    pub fn finished_source(&self) -> Result<FinishedCaptureSource, IncidentStoreError> {
        let shared = self.shared.as_ref().ok_or(IncidentStoreError::RecoveryConflict)?;
        let native = lock(&shared.native);
        if !native.registered || native.retired {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        drop(native);
        Ok(FinishedCaptureSource { shared: shared.clone() })
    }
}
impl FinishedCaptureSource {
    /// Install a genuine finished source in a finite admitted Storage worker.
    /// The same Capture source remains retained until this report is retired and dropped.
    /// # Errors
    /// A missing/borrowed/unregistered Capture or failed source preparation leaves all
    /// partial native outputs in `output`; no path, serialized fact or replacement claim is used.
    pub fn prepare_finished(
        &self,
        output: &mut Option<CapturedReport>,
        storage: &mut DiagnosticStorage,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<(), IncidentStoreError> {
        let shared = &self.shared;
        if shared.execution_id != context.owner_id() || operation.key().owner() != shared.owner {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        if output.is_none() {
            *output = Some(CapturedReport::new(shared.clone()));
        }
        let report = output.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        if !Arc::ptr_eq(shared, &report.source) {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        let mut loan = loan::NativeLoan::take(shared.clone(), CaptureOperation::Observation)
            .map_err(|cause| {
                lock(&shared.native).retain(cause);
                IncidentStoreError::RecoveryConflict
            })?
            .ok_or(IncidentStoreError::RecoveryConflict)?;
        loan.lease()
            .map_err(|cause| {
                lock(&shared.native).retain(cause);
                IncidentStoreError::RecoveryConflict
            })?
            .prepare_finished(report, storage, operation, context)
    }
}
