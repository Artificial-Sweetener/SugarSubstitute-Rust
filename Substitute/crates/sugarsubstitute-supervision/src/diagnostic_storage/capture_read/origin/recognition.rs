// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Classify only an actual checked canonical origin Journal for preservation during this scan.
use super::*;
pub(in crate::diagnostic_storage) const ORIGIN_BOUND: usize = DIAGNOSTIC_SCAN_BOUND;
pub(in crate::diagnostic_storage) struct RecognizedOrigin {
    operation: LogicalStoreOperationKey,
    facts: DiagnosticReadFacts,
    generation: u64,
}
pub(in crate::diagnostic_storage) fn recognized_origin(
    data: &crate::diagnostic_storage::State,
    entry: &crate::diagnostic_storage::inventory::ObservedEntry,
) -> bool {
    entry.class == DiagnosticEntryClass::Journal
        && data.origins.iter().any(|known| {
            known.generation == data.scan_generation
                && known.facts.file_identity() == entry.identity
                && known.facts.bytes() == entry.bytes
                && crate::durable_storage::protected::record_file::diagnostic_object_binding(
                    DiagnosticObject::Journal(known.operation),
                    known.operation.owner().namespace,
                )
                .1 == entry.digest
        })
}
impl DiagnosticStorage {
    /// Recognize one existing canonical protected origin Journal solely for preservation.
    /// This grants no capture-directory access, source disposition or historical grant.
    /// A fresh scan requires the same actual file to be checked again.
    /// # Errors
    /// Foreign, changed, malformed, incomplete or excess records remain unclassified.
    pub fn recognize_recovery_capture_origin(
        &mut self,
        read: &mut DiagnosticReadLease,
        validator: &impl RecoveryCaptureOriginValidator,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<(), DiagnosticStorageError> {
        drop(self.check(operation, context)?);
        if !Shared::ptr_eq(&read.shared, &self.shared) || read.operation != operation.key {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        let bytes = read.whole_source(context)?;
        let mut decode = self.recovery_decode(operation, context)?;
        let record = validator.validate(&mut decode, &bytes).map_err(|cause| {
            native(operation.key, Code::Read, ClaimError::new("capture_origin_codec", cause))
        })?;
        drop(decode);
        drop(bytes);
        if record.operation.owner().namespace != operation.key.owner().namespace {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        read.bind_object(DiagnosticObject::Journal(record.operation), context)?;
        let mut state = self.check(operation, context)?;
        let data = state.data_mut()?;
        let generation = data.scan_generation;
        if !data.inventory_complete
            || !data.scan_entries.iter().any(|entry| {
                entry.class == DiagnosticEntryClass::Journal
                    && entry.identity == read.facts.file_identity()
                    && entry.bytes == read.facts.bytes()
                    && crate::durable_storage::protected::record_file::diagnostic_object_binding(
                        DiagnosticObject::Journal(record.operation),
                        operation.key.owner().namespace,
                    )
                    .1 == entry.digest
            })
        {
            return Err(error(operation.key, Code::ChangedIdentity));
        }
        if let Some(known) =
            data.origins.iter_mut().find(|known| known.operation == record.operation)
        {
            if known.facts != read.facts {
                return Err(error(operation.key, Code::ChangedIdentity));
            }
            known.generation = generation;
        } else {
            if data.origins.len() >= ORIGIN_BOUND {
                return Err(error(operation.key, Code::Capacity));
            }
            data.origins.push(RecognizedOrigin {
                operation: record.operation,
                facts: read.facts,
                generation,
            });
        }
        Ok(())
    }
}
