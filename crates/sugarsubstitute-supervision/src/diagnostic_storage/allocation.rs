// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit closed recovery destinations under the one original protected claim.
mod buffer;
mod collection;
mod decode;
mod incident_validation;
mod ledger;
pub(super) mod loans;
mod native;
mod reservation;
mod roles;
use super::*;
pub use buffer::DiagnosticRecordBuffer;
pub use collection::{DiagnosticRecoveryCollectionErrorLoan, DiagnosticRecoveryCollectionLoan};
pub use decode::{DiagnosticRecoveryDecodeErrorLoan, DiagnosticRecoveryDecodeLoan};
pub use incident_validation::{
    DIAGNOSTIC_INCIDENT_VALIDATION_OUTER_ALIGNMENT, DIAGNOSTIC_INCIDENT_VALIDATION_OUTER_BYTES,
    DIAGNOSTIC_INCIDENT_VALIDATION_PAYLOAD_ALIGNMENT, DIAGNOSTIC_INCIDENT_VALIDATION_PAYLOAD_BYTES,
    DiagnosticIncidentValidationCauseLoan, DiagnosticIncidentValidationControls,
};
pub use ledger::{DIAGNOSTIC_COLLECTION_BYTES, DIAGNOSTIC_DECODE_BYTES, DIAGNOSTIC_RESIDUAL_BYTES};
pub use loans::{
    DiagnosticPayloadDecisionAllocationLoan, DiagnosticPendingPublicationLoan,
    DiagnosticPublicationIdentity, DiagnosticRecoveryRecordLoan, DiagnosticRecoverySeedLoan,
    DiagnosticRetainedPreflightLoan,
};
pub(crate) use native::*;
pub(super) use reservation::{reservation, reservation_for_loan};
pub use roles::*;
use sugarsubstitute_contracts::incident::MAX_ATTACHMENTS;

pub(super) use ledger::Owner as DiagnosticAllocationOwner;
pub(super) use ledger::Residual as DiagnosticAllocationLoan;
pub(super) use loans::RetainedIntent;
pub(super) fn payload_preparation(
    owner: &DiagnosticAllocationOwner,
    key: LogicalStoreOperationKey,
) -> Result<DiagnosticAllocationLoan, DiagnosticStorageError> {
    owner.residual(roles::ResidualRole::PayloadPreparation, key)
}
pub(super) fn payload_intent(
    owner: &DiagnosticAllocationOwner,
    key: LogicalStoreOperationKey,
) -> Result<DiagnosticAllocationLoan, DiagnosticStorageError> {
    owner.residual(roles::ResidualRole::PayloadIntent, key)
}
pub(super) fn migration_intent(
    owner: &DiagnosticAllocationOwner,
    key: LogicalStoreOperationKey,
) -> Result<DiagnosticAllocationLoan, DiagnosticStorageError> {
    owner.residual(roles::ResidualRole::MigrationIntent, key)
}
pub(super) fn whole_source(
    owner: &DiagnosticAllocationOwner,
    key: LogicalStoreOperationKey,
) -> Result<DiagnosticAllocationLoan, DiagnosticStorageError> {
    owner.residual(roles::ResidualRole::WholeSource, key)
}
pub(super) fn inventory(
    owner: &DiagnosticAllocationOwner,
    key: LogicalStoreOperationKey,
) -> Result<DiagnosticAllocationLoan, DiagnosticStorageError> {
    owner.residual(roles::ResidualRole::Inventory, key)
}
pub(super) fn scan_binding(
    owner: &DiagnosticAllocationOwner,
    key: LogicalStoreOperationKey,
) -> Result<DiagnosticAllocationLoan, DiagnosticStorageError> {
    owner.residual(roles::ResidualRole::ScanBinding, key)
}
pub(super) fn owner(
    memory: MemoryClaim,
) -> Result<DiagnosticAllocationOwner, DiagnosticStorageError> {
    ledger::Ledger::admit(memory)
}
pub(super) fn decode_callback<T>(
    storage: &mut DiagnosticStorage,
    operation: &DiagnosticStorageOperation,
    context: &JobContext,
    callback: impl FnOnce(
        &mut DiagnosticRecoveryDecodeLoan,
    ) -> Result<T, DiagnosticRecoveryValidationError>,
) -> Result<T, DiagnosticStorageError> {
    let mut decode = storage.recovery_decode(operation, context)?;
    let grant = decode
        .original
        .as_ref()
        .ok_or_else(|| error(operation.key, Code::Closed))?
        ._phase
        .owner
        .native_grant();
    let allocation = grant.native_cause().map_err(|cause| {
        native(operation.key, Code::Admission, ClaimError::diagnostic_allocation(cause))
    })?;
    let destination = crate::claim_error::NativeCauseDestination::diagnostic(allocation, grant);
    let outcome = callback(&mut decode);
    let after = decode.check(operation, context);
    match outcome {
        Ok(value) => {
            after?;
            Ok(value)
        }
        Err(cause) => Err(native(
            operation.key,
            Code::InvalidIdentity,
            ClaimError::native(
                "diagnostic_decode_validation",
                crate::claim_error::BoundedNativeCause::DiagnosticValidation(cause),
                destination,
            ),
        )
        .with_post_control(after.err())),
    }
}
impl DiagnosticStorage {
    pub(super) fn payload_decision(
        &self,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticPayloadDecisionAllocationLoan, DiagnosticStorageError> {
        let (owner, _) = self.allocation_binding(operation, context)?;
        let allocation = owner.residual(roles::ResidualRole::PayloadDecision, operation.key)?;
        let reservation = allocation::reservation(&owner, Some(operation.key))?;
        let mut victims = Vec::new();
        victims.try_reserve_exact(MAX_ATTACHMENTS).map_err(|cause| reservation.failed(cause))?;
        if victims.capacity() != MAX_ATTACHMENTS {
            return Err(error(operation.key, Code::Capacity));
        }
        Ok(DiagnosticPayloadDecisionAllocationLoan {
            victims,
            key: operation.key,
            _allocation: allocation,
        })
    }
    fn allocation_binding(
        &self,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<(DiagnosticAllocationOwner, loans::Binding), DiagnosticStorageError> {
        let mut state = self.check(operation, context)?;
        let owner = state.data_mut()?.allocation.clone();
        let binding = loans::Binding::from_operation(
            state
                .data_mut()?
                .operation
                .as_ref()
                .ok_or_else(|| error(operation.key, Code::Closed))?,
        );
        drop(state);
        Ok((owner, binding))
    }
    /// Admit the sole128-record collection including both vector allocations.
    /// # Errors
    /// Rejects foreign/expired operation or another live collection phase.
    pub fn recovery_collection(
        &mut self,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticRecoveryCollectionLoan, DiagnosticStorageError> {
        let (owner, binding) = self.allocation_binding(operation, context)?;
        let phase = owner.phase(ledger::Phase::Collection, operation.key)?;
        Ok(DiagnosticRecoveryCollectionLoan { binding, issued: [0; 2], phase })
    }
    /// Admit the sole bounded decode phase, retained through any escaping cause.
    /// # Errors
    /// Rejects foreign/expired operation or another live decode/error phase.
    pub fn recovery_decode(
        &mut self,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticRecoveryDecodeLoan, DiagnosticStorageError> {
        let (owner, binding) = self.allocation_binding(operation, context)?;
        let phase = owner.phase(ledger::Phase::Decode, operation.key)?;
        let mut loan = DiagnosticRecoveryDecodeLoan {
            original: Some(Arc::new(decode::DecodeCustody {
                binding: Mutex::new(binding),
                _phase: phase,
            })),
            error_issued: false,
            validation_issued: false,
        };
        loan.check(operation, context)?;
        Ok(loan)
    }
    /// Admit an independent escaped Summary destination before moving/cloning it.
    /// # Errors
    /// Rejects invalid operation/context and exhausted residual capacity.
    pub fn recovery_record(
        &mut self,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticRecoveryRecordLoan, DiagnosticStorageError> {
        let (owner, _) = self.allocation_binding(operation, context)?;
        Ok(DiagnosticRecoveryRecordLoan {
            _allocation: owner.residual(roles::ResidualRole::Record, operation.key)?,
        })
    }
    /// Admit an independent seed destination before moving/cloning its data.
    /// # Errors
    /// Rejects invalid operation/context and exhausted residual capacity.
    pub fn recovery_seed(
        &mut self,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticRecoverySeedLoan, DiagnosticStorageError> {
        let (owner, _) = self.allocation_binding(operation, context)?;
        Ok(DiagnosticRecoverySeedLoan {
            _allocation: owner.residual(roles::ResidualRole::Seed, operation.key)?,
        })
    }
    /// Admit retained preflight seed/context before constructing its destination.
    /// # Errors
    /// Rejects invalid operation/context and exhausted residual capacity.
    pub fn recovery_preflight(
        &mut self,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticRetainedPreflightLoan, DiagnosticStorageError> {
        let (owner, _) = self.allocation_binding(operation, context)?;
        Ok(DiagnosticRetainedPreflightLoan {
            _allocation: owner.residual(roles::ResidualRole::Preflight, operation.key)?,
        })
    }
    /// Admit complete pending state and literal attachment capacity before cloning.
    /// # Errors
    /// Rejects invalid operation/context and exhausted residual capacity.
    pub fn recovery_pending(
        &mut self,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticPendingPublicationLoan, DiagnosticStorageError> {
        let (owner, _) = self.allocation_binding(operation, context)?;
        Ok(DiagnosticPendingPublicationLoan {
            _allocation: owner.residual(roles::ResidualRole::Pending, operation.key)?,
        })
    }
    /// Admit one actual repository correlation before allocating its Arc control.
    /// # Errors
    /// Rejects invalid operation/context and exhausted residual capacity.
    pub fn recovery_publication_identity(
        &mut self,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticPublicationIdentity, DiagnosticStorageError> {
        let (owner, binding) = self.allocation_binding(operation, context)?;
        let control = owner.residual(roles::ResidualRole::Correlation, operation.key)?;
        let token = owner.residual(roles::ResidualRole::PublicationToken, operation.key)?;
        let identity = Arc::new(());
        control.adopt_correlation();
        Ok(DiagnosticPublicationIdentity {
            allocation: loans::IdentityLoan { control: Some(identity.clone()), token },
            identity,
            binding: Mutex::new(binding),
        })
    }
    /// Admit the complete bounded canonical output before its first allocation.
    /// # Errors
    /// Rejects invalid operation/context, capacity exhaustion or allocation failure.
    pub fn record_buffer(
        &mut self,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticRecordBuffer, DiagnosticStorageError> {
        let (owner, _) = self.allocation_binding(operation, context)?;
        let allocation = owner.residual(roles::ResidualRole::Buffer, operation.key)?;
        let reservation = allocation::reservation(&owner, Some(operation.key))?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(DIAGNOSTIC_IO_BYTES).map_err(|cause| reservation.failed(cause))?;
        if bytes.capacity() != DIAGNOSTIC_IO_BYTES {
            return Err(error(operation.key, Code::Capacity));
        }
        Ok(DiagnosticRecordBuffer { bytes, _allocation: allocation })
    }
}
