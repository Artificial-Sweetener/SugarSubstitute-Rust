// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve common native pipe causes and fatal thread identity restoration facts.
use crate::native::NativeObservationError;
use std::fmt;
#[derive(Clone)]
pub(crate) struct NativePipeError {
    pub(crate) cause: NativeObservationError,
    pub(crate) identity_unrestored: bool,
}
impl NativePipeError {
    /// Preserve an ordinary native failure without declaring fatal identity contamination.
    pub(crate) fn native(cause: NativeObservationError) -> Self {
        Self { cause, identity_unrestored: false }
    }
}
impl fmt::Debug for NativePipeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativePipeError")
            .field("stage", &self.cause.stage())
            .field("identity_unrestored", &self.identity_unrestored)
            .finish_non_exhaustive()
    }
}
impl fmt::Display for NativePipeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Native pipe operation failed")
    }
}
impl std::error::Error for NativePipeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}
