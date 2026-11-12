// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Open only a generation selected by a strict canonical protected promotion Journal read.
use super::*;
use sugarsubstitute_contracts::crash::CaptureGenerationOrigin;
/// Strict decoder matching data. Only the actual protected read and Root pins grant access.
pub struct RecordedCapturePromotion {
    historical: LogicalStoreOperationKey,
    origin: CaptureGenerationOrigin,
    identities: [[u8; 24]; 2],
}
impl RecordedCapturePromotion {
    /// Validate complete inert origin/operation and exact nonzero directory expectations.
    /// # Errors
    /// Foreign namespaces or absent/equal directory identities cannot name a generation.
    pub fn new(
        historical: LogicalStoreOperationKey,
        origin: CaptureGenerationOrigin,
        identities: [[u8; 24]; 2],
    ) -> Result<Self, DiagnosticRecoveryValidationError> {
        if historical.owner().namespace != origin.coordinator().namespace
            || identities.contains(&[0; 24])
            || identities[0] == identities[1]
        {
            return Err(DiagnosticRecoveryValidationError::rejected(
                DiagnosticRecoveryValidationCode::InvalidIdentity,
            ));
        }
        Ok(Self { historical, origin, identities })
    }
}
/// Real semantic adapter for the exact whole protected CapturePromotion record.
pub trait CapturePromotionValidator {
    /// Validate canonical bytes, file digest, full operation, origin and completed payload facts.
    /// # Errors
    /// Unknown schema, changed name/bytes or invalid complete plan retains the actual reader.
    fn validate(
        &self,
        decode: &mut DiagnosticRecoveryDecodeLoan,
        source: &DiagnosticRecoverySource,
    ) -> Result<RecordedCapturePromotion, DiagnosticRecoveryValidationError>;
}
impl DiagnosticStorage {
    /// Prepare fixed existing-only generation pins selected by an actual canonical Journal.
    /// No recorded operation or namespace grants opening independently of current Root custody.
    /// Output and all seven native slots are retained before any directory effect.
    /// # Errors
    /// Partial/changed reads, foreign Root, directory identity mismatch or original control rejects.
    pub fn prepare_capture_promotion_read(
        &mut self,
        source: &mut crate::ProtectedStorageRoot,
        journal: &mut DiagnosticReadLease,
        validator: &impl CapturePromotionValidator,
        output: &mut Option<ProtectedCaptureReadLease>,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<(), DiagnosticStorageError> {
        drop(self.check(operation, context)?);
        if output.is_some()
            || !Shared::ptr_eq(&journal.shared, &self.shared)
            || journal.operation != operation.key
        {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        let bytes = journal.whole_source(context)?;
        let mut decode = self.recovery_decode(operation, context)?;
        let record = validator.validate(&mut decode, &bytes).map_err(|cause| {
            native(operation.key, Code::Read, ClaimError::new("capture_promotion_codec", cause))
        })?;
        drop(decode);
        drop(bytes);
        if record.historical == operation.key
            || record.historical.owner().namespace != operation.key.owner().namespace
        {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        journal.bind_object(DiagnosticObject::Journal(record.historical), context)?;
        let mut state = self.check(operation, context)?;
        if state.data_mut()?.reads.len() + state.data_mut()?.capture_reads >= DIAGNOSTIC_READ_BOUND
            || state
                .data_mut()?
                .live_handles()
                .checked_add(7)
                .is_none_or(|count| count > DIAGNOSTIC_HANDLE_BOUND)
        {
            return Err(error(operation.key, Code::Capacity));
        }
        let original = state.data_mut()?.allocation.native_grant();
        drop(state);
        let owner = source
            .admit_recorded_capture_read(original, record.origin, context)
            .map_err(|cause| native(operation.key, Code::Admission, cause))?;
        let mut state = self.check(operation, context)?;
        if state.data_mut()?.reads.len() + state.data_mut()?.capture_reads >= DIAGNOSTIC_READ_BOUND
            || state
                .data_mut()?
                .live_handles()
                .checked_add(7)
                .is_none_or(|count| count > DIAGNOSTIC_HANDLE_BOUND)
        {
            return Err(error(operation.key, Code::Capacity));
        }
        state.data_mut()?.capture_reads += 1;
        state.data_mut()?.capture_handles += 7;
        *output = Some(ProtectedCaptureReadLease {
            owner,
            shared: Some(self.shared.clone()),
            operation: operation.key,
            reserved_handles: 7,
        });
        drop(state);
        source
            .prepare_recorded_capture_read_handles(
                &mut output.as_mut().ok_or_else(|| error(operation.key, Code::Closed))?.owner,
                record.origin,
                context,
            )
            .map_err(|cause| native(operation.key, Code::Read, cause))?;
        let guard =
            output.as_mut().ok_or_else(|| error(operation.key, Code::Closed))?.borrow(context)?;
        if guard.identities()? != record.identities {
            return Err(error(operation.key, Code::ChangedIdentity));
        }
        Ok(())
    }
}
