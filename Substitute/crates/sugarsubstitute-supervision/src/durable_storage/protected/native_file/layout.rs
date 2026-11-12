// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Describe concrete physical file and bounded hash storage before native effects.
use super::*;
impl NativeFile {
    /// Describe the actual fields Box; its inline wrapper belongs to its container.
    pub(crate) fn diagnostic_owner_layout() -> Result<
        crate::native_claims::files::NativeRoleLayout,
        crate::native_claims::files::NativeLayoutFailure,
    > {
        let bytes = std::alloc::Layout::new::<Fields>().size();
        Ok(crate::native_claims::files::NativeRoleLayout { retained: bytes, peak: bytes })
    }
    /// Describe the complete bounded hash phase, including its real controls.
    pub(crate) fn diagnostic_hash_layout() -> Result<
        crate::native_claims::files::NativeRoleLayout,
        crate::native_claims::files::NativeLayoutFailure,
    > {
        use crate::{
            diagnostic_storage::DiagnosticNativeScratchLoan,
            native_claims::files::{NativeLayoutFailure, NativeRoleLayout},
        };
        let controls = size_of::<Vec<u8>>()
            .checked_add(size_of::<ring::digest::Context>())
            .and_then(|value| value.checked_add(size_of::<ring::digest::Digest>()))
            .and_then(|value| value.checked_add(size_of::<DiagnosticNativeScratchLoan>()))
            .ok_or(NativeLayoutFailure::Overflow)?;
        let peak =
            DIAGNOSTIC_IO_BYTES.checked_add(controls).ok_or(NativeLayoutFailure::Overflow)?;
        Ok(NativeRoleLayout { retained: 0, peak })
    }
}
