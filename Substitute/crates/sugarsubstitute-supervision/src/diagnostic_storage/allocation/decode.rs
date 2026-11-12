// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the original exclusive decode phase through an escaping real cause.
use super::*;

pub(super) struct DecodeCustody {
    pub(super) binding: Mutex<loans::Binding>,
    pub(super) _phase: ledger::PhaseLoan,
}
/// Borrow one exclusive bounded decode phase for parsing and canonical callbacks.
pub struct DiagnosticRecoveryDecodeLoan {
    pub(super) original: Option<Arc<DecodeCustody>>,
    pub(super) error_issued: bool,
    pub(super) validation_issued: bool,
}
/// Retain the same decode phase with a genuine escaping codec error.
pub struct DiagnosticRecoveryDecodeErrorLoan {
    original: Option<Arc<DecodeCustody>>,
}
impl Drop for DiagnosticRecoveryDecodeLoan {
    fn drop(&mut self) {
        if let Some(original) = self.original.take() {
            drop(Arc::into_inner(original));
        }
    }
}
impl Drop for DiagnosticRecoveryDecodeErrorLoan {
    fn drop(&mut self) {
        if let Some(original) = self.original.take() {
            drop(Arc::into_inner(original));
        }
    }
}
impl DiagnosticRecoveryDecodeLoan {
    pub(in crate::diagnostic_storage) fn check(
        &mut self,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<(), DiagnosticStorageError> {
        let original = self.original.as_ref().ok_or_else(|| error(operation.key, Code::Closed))?;
        if !original._phase.owner.same(&operation.shared._allocation) {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        let snapshot =
            original.binding.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
        let observation = snapshot.observe(context);
        let mut binding =
            original.binding.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if binding.key() != operation.key {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        binding.validate(&original._phase.owner, observation)
    }
    /// Keep the already admitted phase until its retained cause actually drops.
    /// # Errors
    /// Rejects issuing more than one escaping error owner from this decode session.
    pub fn retain_error(
        &mut self,
    ) -> Result<DiagnosticRecoveryDecodeErrorLoan, DiagnosticStorageError> {
        if self.error_issued {
            return Err(DiagnosticStorageError::rejected(None, Code::Capacity));
        }
        self.error_issued = true;
        Ok(DiagnosticRecoveryDecodeErrorLoan { original: self.original.clone() })
    }
}
