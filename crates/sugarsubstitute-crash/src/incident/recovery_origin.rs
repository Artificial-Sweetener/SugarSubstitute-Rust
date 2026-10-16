// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish prelaunch grant correlation and rediscover it through genuine protected read custody.
use super::*;
use crate::capture::CaptureRootSource;
use sugarsubstitute_contracts::{crash::RecoveryCaptureIdentity, execution::JobContext};
use sugarsubstitute_execution::RegisteredRecoveryGrant;
use sugarsubstitute_supervision::diagnostic_storage::*;
/// One original origin preparation, retained before any publication effect.
/// Only checked() exposes completed real read custody; a partial value cannot release creation.
pub struct RecoveryCaptureOriginCommit {
    record: RecoveryCaptureOriginRecord,
    seed: Option<SeedCommit>,
    activation: Option<DiagnosticActivation>,
    checked: Option<CheckedRecoveryCaptureOrigin>,
    _custody: DiagnosticRecoveryCaptureOriginLoan,
}
const _: () = assert!(
    size_of::<RecoveryCaptureOriginCommit>() <= DIAGNOSTIC_CAPTURE_ORIGIN_COMMIT_INLINE_BYTES
);
impl RecoveryCaptureOriginCommit {
    /// Observe exact once-only historical matching data.
    pub const fn origin(&self) -> RecoveryCaptureIdentity {
        self.record.origin()
    }
    /// Observe original preparation key, never a renewed budget.
    pub const fn operation(&self) -> sugarsubstitute_contracts::incident::LogicalStoreOperationKey {
        self.record.operation()
    }
    /// Checked-close the actual retained canonical read before repository cleanup.
    /// The durable journal remains intact and its immutable activation facts remain owned.
    /// # Errors
    /// Preserves original context/native close failure and retains the same live read slot.
    pub fn retire(&mut self, context: &JobContext) -> Result<(), IncidentStoreError> {
        if let Some(checked) = &mut self.checked {
            checked.retire(context)?;
        }
        Ok(())
    }
    /// Borrow only actually completed protected origin custody.
    /// # Errors
    /// Partial activation/read/cleanup never grants a prepared-creation fact.
    pub fn checked(&self) -> Result<&CheckedRecoveryCaptureOrigin, IncidentStoreError> {
        self.checked.as_ref().ok_or(IncidentStoreError::RecoveryConflict)
    }
}
/// Actual strict incident codec for the protected storage origin-validation boundary.
pub struct IncidentRecoveryCaptureOriginValidator;
impl RecoveryCaptureOriginValidator for IncidentRecoveryCaptureOriginValidator {
    fn validate(
        &self,
        decode: &mut DiagnosticRecoveryDecodeLoan,
        source: &DiagnosticRecoverySource,
    ) -> Result<RecordedRecoveryCaptureOrigin, DiagnosticRecoveryValidationError> {
        use super::allocation::ValidationFailure as Failure;
        use sugarsubstitute_supervision::diagnostic_storage::DiagnosticRecoveryValidationCode as Code;
        super::allocation::validation(decode, || {
            if source.class() != DiagnosticEntryClass::Journal {
                return Err(Failure::rejected(Code::InvalidIdentity));
            }
            let state = decode_store_state(source.as_bytes())
                .map_err(|cause| Failure::store(Code::InvalidJournal, cause))?;
            let IncidentStoreState::RecoveryCaptureOrigin(record) = state else {
                return Err(Failure::rejected(Code::InvalidJournal));
            };
            let canonical =
                encode_store_state(&IncidentStoreState::RecoveryCaptureOrigin(record.clone()))
                    .map_err(|cause| Failure::store(Code::InvalidJournal, cause))?;
            if canonical.as_slice() != source.as_bytes()
                || source.facts().bytes() != canonical.len() as u64
                || source.facts().sha256() != codec_ledger::digest(&canonical)
                || source
                    .object()
                    .is_some_and(|object| object != DiagnosticObject::Journal(record.operation()))
            {
                return Err(Failure::rejected(Code::InvalidIdentity));
            }
            RecordedRecoveryCaptureOrigin::new(
                record.operation(),
                record.origin(),
                record.root_identity(),
                record.build().clone(),
            )
            .map_err(Failure::from)
        })
    }
}
impl IncidentRepository {
    /// Durably prepare one original grant before any successor process creation.
    /// Use a distinct genuine repository operation for each grant; the existing report
    /// journal is not replaced. Output retains actual pending/partial custody on failure.
    /// # Errors
    /// Repeated/foreign grants, occupied output, native failures or original deadline stop launch.
    pub fn publish_recovery_capture_origin(
        &mut self,
        grant: &RegisteredRecoveryGrant,
        source: &CaptureRootSource,
        at: sugarsubstitute_contracts::incident::IncidentUtc,
        output: &mut Option<RecoveryCaptureOriginCommit>,
        context: &JobContext,
    ) -> Result<(), IncidentStoreError> {
        grant.check_context(context).map_err(|cause| {
            IncidentStoreError::Storage(DiagnosticStorageError::from_recovery_grant(cause))
        })?;
        if output.is_some()
            || self.pending.is_some()
            || self.preflight.is_some()
            || grant.key().run().owner != self.operation.key().owner()
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        let custody = self.storage.recovery_capture_origin(&self.operation, context)?;
        let record = RecoveryCaptureOriginRecord::new(
            self.operation.key(),
            RecoveryCaptureIdentity::from_grant(grant.key()),
            source.origin_root(&self.operation, context)?,
            &source.recorded_build()?,
        )?;
        *output = Some(RecoveryCaptureOriginCommit {
            record: record.clone(),
            seed: None,
            activation: None,
            checked: None,
            _custody: custody,
        });
        let producer =
            sugarsubstitute_contracts::crash::CaptureGenerationOrigin::Recovery(record.origin())
                .producer_subject()?;
        let seed = sugarsubstitute_contracts::incident::IncidentSeed::new(
            sugarsubstitute_contracts::incident::IncidentKey::new(producer),
            sugarsubstitute_contracts::incident::IncidentRevision::new(1)?,
            record.build().clone(),
            at,
            sugarsubstitute_contracts::incident::IncidentStage::SupervisorInitialization,
        )?;
        // Actual durable initialization intent precedes every fresh process launch; no exit is claimed.
        let committed = self.publish_seed(seed, context)?;
        output.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?.seed = Some(committed);
        let mut encoded = self.storage.record_buffer(&self.operation, context)?;
        let mut decode = self.storage.recovery_decode(&self.operation, context)?;
        super::allocation::decode(&mut decode, || {
            codec_ledger::write_store_state(
                &IncidentStoreState::RecoveryCaptureOrigin(record.clone()),
                &mut encoded,
            )
        })?;
        drop(decode);
        self.validate_store(context)?;
        let actual = self.activate_bytes(
            DiagnosticObject::Journal(record.operation()),
            encoded.as_bytes(),
            DiagnosticPredecessor::Absent,
            context,
        )?;
        output.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?.activation = Some(actual);
        drop(encoded);
        let read = self.storage.read(
            DiagnosticObject::Journal(record.operation()),
            4096,
            &self.operation,
            context,
        )?;
        let checked = source.check_origin_read(
            read,
            &IncidentRecoveryCaptureOriginValidator,
            &mut self.storage,
            &self.operation,
            context,
        )?;
        let seed = output
            .as_ref()
            .and_then(|output| output.seed.as_ref())
            .ok_or(IncidentStoreError::RecoveryConflict)?;
        if seed.operation() != record.operation()
            || seed.seed().build() != record.build()
            || seed.key().subject() != producer
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        let actual = output
            .as_ref()
            .and_then(|output| output.activation.as_ref())
            .ok_or(IncidentStoreError::RecoveryConflict)?;
        if checked.build() != record.build()
            || checked.origin() != record.origin()
            || checked.historical_operation() != record.operation()
            || checked.journal_facts() != actual.facts()
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        output.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?.checked = Some(checked);
        grant.check_context(context).map_err(|cause| {
            IncidentStoreError::Storage(DiagnosticStorageError::from_recovery_grant(cause))
        })
    }
    /// Retain one complete bounded protected inventory for historical origin discovery.
    /// Only canonical Journal candidates may be passed to discover_recovery_capture_origin;
    /// this observation cannot open unknown paths or grant capture/source disposition.
    /// # Errors
    /// Unknown/absent ledger, incomplete enumeration or original control failure blocks scanning.
    pub fn recovery_capture_inventory(
        &mut self,
        context: &JobContext,
    ) -> Result<DiagnosticInventory, IncidentStoreError> {
        self.validate_store(context)?;
        self.inventory(context)
    }
    /// Check exactly one canonical bounded Journal inventory candidate for an earlier recovery origin.
    /// A valid unrelated journal returns None unchanged. Unknown/corrupt records fail closed;
    /// unknown capture directories are never enumerated or opened by this operation.
    /// # Errors
    /// Original read/native/codec failures remain retained; no historical grant is reconstructed.
    pub fn discover_recovery_capture_origin(
        &mut self,
        source: &CaptureRootSource,
        candidate: &DiagnosticInventoryEntry,
        context: &JobContext,
    ) -> Result<Option<CheckedRecoveryCaptureOrigin>, IncidentStoreError> {
        source.origin_root(&self.operation, context)?;
        if candidate.class() != DiagnosticEntryClass::Journal || candidate.bytes() > 4096 {
            return Ok(None);
        }
        let (bytes, _, index) = self.read_candidate(candidate, context)?;
        let mut decode = self.storage.recovery_decode(&self.operation, context)?;
        let state =
            super::allocation::decode(&mut decode, || decode_store_state(bytes.as_bytes()))?;
        drop(decode);
        drop(bytes);
        if !matches!(state, IncidentStoreState::RecoveryCaptureOrigin(_)) {
            self.close_read(index, context)?;
            return Ok(None);
        }
        let read = self.reads.remove(index)?;
        Ok(Some(source.check_origin_read(
            read,
            &IncidentRecoveryCaptureOriginValidator,
            &mut self.storage,
            &self.operation,
            context,
        )?))
    }
}
