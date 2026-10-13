// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one admitted transient phase and its genuine error destinations.
use super::*;
use crate::protected_storage::{native_file::NativeCauseDestinations, native_phase::NativePhase};

/// Keep scratch and outside original custody alive through each producing scope.
pub(super) struct Session {
    causes: NativeCauseDestinations,
    scratch: DiagnosticNativeScratchLoan,
    original: DiagnosticNativeGrant,
}
impl Session {
    /// Build the sole path allocation only after its complete destination is held.
    pub(super) fn name(
        &mut self,
        directory: &crate::protected_storage::native_name::NativeName,
        leaf: &str,
        current: &JobContext,
    ) -> Result<crate::protected_storage::native_name::NativeName, ClaimError> {
        let mut phase = self.phase(current);
        let destination = phase.name_destination()?;
        let directory = directory.path()?;
        let length = directory
            .as_os_str()
            .len()
            .checked_add(1)
            .and_then(|length| length.checked_add(leaf.len()))
            .filter(|length| *length <= 192 * 1024)
            .ok_or_else(|| phase.invalid("diagnostic_name_capacity"))?;
        if leaf.is_empty() || leaf.bytes().any(|byte| matches!(byte, b'/' | b'\\' | 0)) {
            return Err(phase.invalid("diagnostic_leaf"));
        }
        let mut path = PathBuf::new();
        path.try_reserve_exact(length).map_err(|cause| {
            phase.error(
                "diagnostic_name_reservation",
                crate::claim_error::BoundedNativeCause::Reserve(cause),
            )
        })?;
        path.push(directory);
        path.push(leaf);
        if path.capacity() > 192 * 1024 {
            return Err(phase.invalid("diagnostic_name_capacity"));
        }
        phase.check()?;
        destination.finish(path)
    }
    pub(super) fn path(
        original: &DiagnosticNativeGrant,
        current: &JobContext,
    ) -> Result<Self, ClaimError> {
        let causes = NativeCauseDestinations::diagnostic(original)?;
        let scratch = original.path_scratch(current).map_err(ClaimError::diagnostic_allocation)?;
        Ok(Self { causes, scratch, original: original.clone() })
    }
    pub(super) fn scan(
        original: &DiagnosticNativeGrant,
        current: &JobContext,
    ) -> Result<Self, ClaimError> {
        let causes = NativeCauseDestinations::diagnostic(original)?;
        let scratch = original.scan_page(current).map_err(ClaimError::diagnostic_allocation)?;
        Ok(Self { causes, scratch, original: original.clone() })
    }
    pub(super) fn phase<'a>(&'a mut self, current: &'a JobContext) -> NativePhase<'a> {
        NativePhase::Diagnostic {
            original: &self.original,
            current,
            scratch: &mut self.scratch,
            causes: &mut self.causes,
        }
    }
}
