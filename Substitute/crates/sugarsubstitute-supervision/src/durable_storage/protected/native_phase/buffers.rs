// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep producing buffers and the same exclusive native phase alive until adoption.
use super::*;
/// Keep ALL producing buffers alive until the caller adopts actual native output.
pub(crate) struct NativeOpenBuffers<'phase, 'origin> {
    pub(super) units: Vec<u16>,
    pub(super) descriptor: Option<security::Descriptor>,
    pub(super) _session: &'phase mut NativePhase<'origin>,
}
impl NativeOpenBuffers<'_, '_> {
    /// Check the existing actual job while ALL producing allocations remain live.
    pub(crate) fn context(&mut self, context: &JobContext) -> Result<(), ClaimError> {
        self._session.context(context)
    }
    /// Revalidate actual original control immediately before the finite call.
    pub(crate) fn check(&self) -> Result<(), ClaimError> {
        self._session.check()
    }
    /// Borrow bounded NUL units while this producing scope remains live.
    pub(crate) fn units(&self) -> &[u16] {
        &self.units
    }
    /// Borrow security attributes pointing into this same retained descriptor.
    pub(crate) fn attributes(&self) -> Option<windows::Win32::Security::SECURITY_ATTRIBUTES> {
        self.descriptor.as_ref().map(security::Descriptor::attributes)
    }
}

/// Own the complete producing path allocation before relinquishing its phase.
pub(crate) struct NativeWidePath<'phase, 'origin> {
    pub(super) units: Vec<u16>,
    pub(super) session: &'phase mut NativePhase<'origin>,
}
impl NativeWidePath<'_, '_> {
    /// Borrow the same bounded NUL destination for the finite native call.
    pub(crate) fn units(&self) -> &[u16] {
        &self.units
    }
    /// Revalidate original control immediately before an effect.
    pub(crate) fn check(&self) -> Result<(), ClaimError> {
        self.session.check()
    }
    /// Preserve an allocation failure while the same producing phase remains borrowed.
    pub(crate) fn error(
        &mut self,
        operation: &'static str,
        cause: BoundedNativeCause,
    ) -> ClaimError {
        self.session.error(operation, cause)
    }
}
