// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit a new Root-local read source without receiving any subordinate native token.
use super::*;
use sugarsubstitute_execution::RegisteredRecoveryGrant;
impl DiagnosticStorage {
    /// Open only the canonical physical generation of the genuine once-only Root grant.
    /// The original registered Root source supplies the real account/root pins; this
    /// process creates fresh local directory handles under the current diagnostic owner.
    /// No child path, C++ lease/address or serialized native capability is accepted.
    /// # Errors
    /// Pre-effect rejection leaves output empty; after admission every partial output,
    /// source fence, original cause and all seven reserved handle slots remain retained.
    pub fn prepare_recovery_capture_read(
        &mut self,
        source: &mut crate::ProtectedStorageRoot,
        grant: &RegisteredRecoveryGrant,
        output: &mut Option<ProtectedCaptureReadLease>,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<(), DiagnosticStorageError> {
        grant.check_context(context).map_err(|cause| {
            native(operation.key, Code::Admission, ClaimError::new("capture_recovery_grant", cause))
        })?;
        if grant.key().run().owner != operation.key.owner()
            || source.capture_origin()
                != Some(sugarsubstitute_contracts::crash::CaptureGenerationOrigin::Coordinator(
                    grant.key().run().owner,
                ))
        {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
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
        let owner = source
            .admit_recorded_capture_read(
                original,
                sugarsubstitute_contracts::crash::CaptureGenerationOrigin::Recovery(
                    sugarsubstitute_contracts::crash::RecoveryCaptureIdentity::from_grant(
                        grant.key(),
                    ),
                ),
                context,
            )
            .map_err(|cause| native(operation.key, Code::Admission, cause))?;
        let mut state = self.check(operation, context)?;
        grant.check_context(context).map_err(|cause| {
            native(operation.key, Code::Admission, ClaimError::new("capture_recovery_grant", cause))
        })?;
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
                sugarsubstitute_contracts::crash::CaptureGenerationOrigin::Recovery(
                    sugarsubstitute_contracts::crash::RecoveryCaptureIdentity::from_grant(
                        grant.key(),
                    ),
                ),
                context,
            )
            .map_err(|cause| native(operation.key, Code::Read, cause))?;
        drop(self.check(operation, context)?);
        grant.check_context(context).map_err(|cause| {
            native(operation.key, Code::Admission, ClaimError::new("capture_recovery_grant", cause))
        })
    }
}
