// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Open the exact historical generation only after its canonical origin journal was checked.
use super::*;
impl DiagnosticStorage {
    /// Prepare fresh local pins from the genuine current Root and retained historical journal proof.
    /// The old Root epoch is not current operation authority. Unknown directories are untouched.
    /// # Errors
    /// All seven handle slots and partial outputs remain owned before native effects.
    pub fn prepare_historical_recovery_capture_read(
        &mut self,
        source: &mut crate::ProtectedStorageRoot,
        checked: &CheckedRecoveryCaptureOrigin,
        output: &mut Option<ProtectedCaptureReadLease>,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<(), DiagnosticStorageError> {
        checked.current(self, source, operation, context)?;
        let mut state = self.check(operation, context)?;
        if output.is_some()
            || state.data_mut()?.reads.len() + state.data_mut()?.capture_reads
                >= DIAGNOSTIC_READ_BOUND
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
        let origin =
            sugarsubstitute_contracts::crash::CaptureGenerationOrigin::Recovery(checked.origin());
        let owner = source
            .admit_recorded_capture_read(original, origin, context)
            .map_err(|cause| native(operation.key, Code::Admission, cause))?;
        checked.current(self, source, operation, context)?;
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
                origin,
                context,
            )
            .map_err(|cause| native(operation.key, Code::Read, cause))?;
        checked.current(self, source, operation, context)
    }
}
