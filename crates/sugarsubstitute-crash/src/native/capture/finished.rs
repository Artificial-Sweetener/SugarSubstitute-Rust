// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Issue finished reads only through the same registered capture and process-PIN module.
use super::*;
use crate::incident::{CapturedReport, IncidentStoreError};
use sugarsubstitute_supervision::diagnostic_storage::{
    DiagnosticStorage, DiagnosticStorageOperation,
};
impl Capture {
    pub(super) fn prepare_finished_native(
        &mut self,
        report: &mut CapturedReport,
        storage: &mut DiagnosticStorage,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<(), IncidentStoreError> {
        let fields = self.fields.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        let api = fields.api.as_ref().ok_or(IncidentStoreError::RecoveryConflict)?.finished;
        if !fields.registered
            || fields.io_retired
            || report.token.is_some()
            || context.owner_id() != report.source.execution_id
            || operation.key().owner() != report.source.owner
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        if report.generation.is_none() {
            storage.prepare_capture_read(
                &mut fields.storage,
                &mut report.generation,
                operation,
                context,
            )?;
        }
        if report.cpp.is_none() && report.rust.is_none() {
            let (cpp, rust) = storage.reserve_finished_owner(
                report.generation.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?,
                operation,
                context,
            )?;
            report.cpp = Some(cpp);
            report.rust = Some(rust);
        }
        super::super::finished_report::Token::install(
            &mut report.token,
            api,
            &mut report.generation,
            &mut report.cpp,
            &mut report.rust,
            context,
        )
        .map_err(|_| IncidentStoreError::RecoveryConflict)?;
        report
            .token
            .as_mut()
            .ok_or(IncidentStoreError::RecoveryConflict)?
            .encode_deadline(&fields.execution)
            .map_err(|_| IncidentStoreError::RecoveryConflict)
    }
}
