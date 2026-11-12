// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode into an admitted destination while the original decode phase owns conversion scratch.
use super::super::{IncidentRepository, IncidentStoreError, StoredIncidentSummary};
use sugarsubstitute_contracts::{execution::JobContext, incident::IncidentSeed};
use sugarsubstitute_supervision::diagnostic_storage::DiagnosticRecordBuffer;
impl IncidentRepository {
    pub(in crate::incident) fn encode_seed(
        &mut self,
        seed: &IncidentSeed,
        context: &JobContext,
    ) -> Result<DiagnosticRecordBuffer, IncidentStoreError> {
        let mut output = self.storage.record_buffer(&self.operation, context)?;
        let mut phase = self.storage.recovery_decode(&self.operation, context)?;
        super::decode(&mut phase, || super::super::codec::write_seed(seed, &mut output))?;
        if output.as_bytes().len() > super::super::codec::SEED_BOUND {
            return Err(IncidentStoreError::RecordBound);
        }
        Ok(output)
    }
    pub(in crate::incident) fn encode_summary(
        &mut self,
        summary: &StoredIncidentSummary,
        context: &JobContext,
    ) -> Result<DiagnosticRecordBuffer, IncidentStoreError> {
        let mut output = self.storage.record_buffer(&self.operation, context)?;
        let mut phase = self.storage.recovery_decode(&self.operation, context)?;
        super::decode(&mut phase, || {
            super::super::codec_summary::write_summary(summary, &mut output)
        })?;
        Ok(output)
    }
}
