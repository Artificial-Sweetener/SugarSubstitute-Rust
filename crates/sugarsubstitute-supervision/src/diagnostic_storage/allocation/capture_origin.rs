// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain exact produced origin metadata under the original diagnostic residual owner.
use super::*;
/// Keep the actual original diagnostic allocation alive through an escaped origin owner.
/// No caller-provided size, replacement credit or native authority is accepted.
pub struct DiagnosticRecoveryCaptureOriginLoan {
    _allocation: DiagnosticAllocationLoan,
}
impl DiagnosticStorage {
    /// Admit the complete concrete origin commit before constructing its retained cells.
    /// # Errors
    /// Wrong current operation or unavailable original residual capacity rejects admission.
    pub fn recovery_capture_origin(
        &mut self,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticRecoveryCaptureOriginLoan, DiagnosticStorageError> {
        let (owner, _) = self.allocation_binding(operation, context)?;
        Ok(DiagnosticRecoveryCaptureOriginLoan {
            _allocation: owner.residual(roles::ResidualRole::CaptureOrigin, operation.key)?,
        })
    }
    pub(in crate::diagnostic_storage) fn recovery_capture_origin_read(
        &mut self,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticRecoveryCaptureOriginLoan, DiagnosticStorageError> {
        let (owner, _) = self.allocation_binding(operation, context)?;
        Ok(DiagnosticRecoveryCaptureOriginLoan {
            _allocation: owner.residual(roles::ResidualRole::CaptureOriginRead, operation.key)?,
        })
    }
}
#[cfg(test)]
mod tests;
