// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Seal exact historical origin only from a genuine retained protected journal read.
use super::*;
use sugarsubstitute_contracts::{crash::RecoveryCaptureIdentity, incident::BuildIdentity};
/// Pure decoder result; this value cannot open a path or construct checked origin custody.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordedRecoveryCaptureOrigin {
    operation: LogicalStoreOperationKey,
    origin: RecoveryCaptureIdentity,
    root_identity: [u8; 24],
    build: BuildIdentity,
}
impl RecordedRecoveryCaptureOrigin {
    /// Validate exact historical preparation relationship without granting native authority.
    /// # Errors
    /// Rejects foreign original Root or absent protected-root identity.
    pub fn new(
        operation: LogicalStoreOperationKey,
        origin: RecoveryCaptureIdentity,
        root_identity: [u8; 24],
        build: BuildIdentity,
    ) -> Result<Self, DiagnosticRecoveryValidationError> {
        if operation.owner() != origin.run().owner || root_identity == [0; 24] {
            return Err(DiagnosticRecoveryValidationError::rejected(
                DiagnosticRecoveryValidationCode::InvalidIdentity,
            ));
        }
        Ok(Self { operation, origin, root_identity, build })
    }
    /// Observe complete original preparation identity.
    pub const fn operation(&self) -> LogicalStoreOperationKey {
        self.operation
    }
    /// Observe exact historical origin facts.
    pub const fn origin(&self) -> RecoveryCaptureIdentity {
        self.origin
    }
    /// Borrow exact recorded producer build data, never a current-version substitution.
    pub const fn build(&self) -> &BuildIdentity {
        &self.build
    }
    /// Observe the historical protected-root native identity.
    pub const fn root_identity(&self) -> [u8; 24] {
        self.root_identity
    }
}
/// Real strict incident-codec boundary; matching decisions never supply native authority.
pub trait RecoveryCaptureOriginValidator {
    /// Decode canonical bounded bytes; preserve actual decoder failure through its admitted loan.
    /// # Errors
    /// Rejects unknown schemas, noncanonical bytes, mixed identities and changed journal facts.
    fn validate(
        &self,
        decode: &mut DiagnosticRecoveryDecodeLoan,
        source: &DiagnosticRecoverySource,
    ) -> Result<RecordedRecoveryCaptureOrigin, DiagnosticRecoveryValidationError>;
}
/// Constructor-private actual source custody under one genuine current Root operation.
/// This retains the real read slot, current operation and protected file facts. Historical
/// Root/run/grant are matching data; a successor epoch does not reuse old authority.
pub struct CheckedRecoveryCaptureOrigin {
    read: DiagnosticReadLease,
    record: RecordedRecoveryCaptureOrigin,
    retired: bool,
    _custody: DiagnosticRecoveryCaptureOriginLoan,
}
impl CheckedRecoveryCaptureOrigin {
    /// Observe exact historical matching data without issuing another proof.
    pub const fn origin(&self) -> RecoveryCaptureIdentity {
        self.record.origin
    }
    /// Observe original durable preparation operation, independently of current authority.
    pub const fn historical_operation(&self) -> LogicalStoreOperationKey {
        self.record.operation
    }
    /// Borrow exact historical build only after canonical native read validation.
    pub const fn build(&self) -> &BuildIdentity {
        &self.record.build
    }
    /// Observe the current operation which checked the actual retained journal source.
    pub const fn current_operation(&self) -> LogicalStoreOperationKey {
        self.read.operation
    }
    /// Observe complete actual journal identity/digest/length without granting another read.
    pub const fn journal_facts(&self) -> DiagnosticReadFacts {
        self.read.facts
    }
    /// Observe actual protected-root identity, never a path or native handle.
    pub const fn root_identity(&self) -> [u8; 24] {
        self.record.root_identity
    }
    /// Checked-close actual origin read custody; failure preserves the original slot.
    /// # Errors
    /// Wrong context, cancellation, deadline or native close failure never grants retirement.
    pub fn retire(&mut self, context: &JobContext) -> Result<(), DiagnosticStorageError> {
        self.read.retire(context)?;
        self.retired = true;
        Ok(())
    }
    pub(super) fn current(
        &self,
        storage: &DiagnosticStorage,
        source: &crate::ProtectedStorageRoot,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<(), DiagnosticStorageError> {
        let mut state = storage.check(operation, context)?;
        if self.retired
            || !Shared::ptr_eq(&self.read.shared, &storage.shared)
            || self.read.operation != operation.key
            || !matches_current_root(&self.record, operation.key.owner(), source.root_identity())
            || !state.data_mut()?.reads.iter().any(|read| {
                read.id == self.read.id
                    && read.native.is_some()
                    && read.facts == Some(self.read.facts)
                    && read.object == Some(DiagnosticObject::Journal(self.record.operation))
            })
        {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        Ok(())
    }
}
impl DiagnosticStorage {
    /// Seal a strict decoder decision only after actual canonical read binding and current checks.
    /// No decoded payload, namespace or supplied historical key can independently issue this proof.
    /// # Errors
    /// All failed reads remain retained by storage; occupied output and foreign root are rejected.
    pub fn check_recovery_capture_origin(
        &mut self,
        source: &crate::ProtectedStorageRoot,
        mut read: DiagnosticReadLease,
        validator: &impl RecoveryCaptureOriginValidator,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<CheckedRecoveryCaptureOrigin, DiagnosticStorageError> {
        drop(self.check(operation, context)?);
        if !Shared::ptr_eq(&read.shared, &self.shared) || read.operation != operation.key {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        let custody = self.recovery_capture_origin_read(operation, context)?;
        let bytes = read.whole_source(context)?;
        let mut decode = self.recovery_decode(operation, context)?;
        let record = validator.validate(&mut decode, &bytes).map_err(|cause| {
            native(operation.key, Code::Read, ClaimError::new("capture_origin_codec", cause))
        })?;
        drop(decode);
        drop(bytes);
        read.bind_object(DiagnosticObject::Journal(record.operation), context)?;
        let checked =
            CheckedRecoveryCaptureOrigin { read, record, retired: false, _custody: custody };
        checked.current(self, source, operation, context)?;
        Ok(checked)
    }
}

impl DiagnosticStorageError {
    /// Preserve an actual registered-grant failure without rendering its private source text.
    pub fn from_recovery_grant(
        cause: sugarsubstitute_contracts::execution::AdmissionError,
    ) -> Self {
        Self::native(None, Code::Admission, ClaimError::new("capture_origin_grant", cause))
    }
}

fn matches_current_root(
    record: &RecordedRecoveryCaptureOrigin,
    current: CoordinatorKey,
    identity: Option<[u8; 24]>,
) -> bool {
    record.origin.run().owner.namespace == current.namespace
        && identity == Some(record.root_identity)
}
#[cfg(test)]
mod tests;

mod recognition;
pub(in crate::diagnostic_storage) use recognition::{
    ORIGIN_BOUND, RecognizedOrigin, recognized_origin,
};
