// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one already-admitted immutable original without copying its allocation or charge.
use crate::ImportError;
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};

/// Data retires before its transferred allocation charge and shared metadata admission.
struct MemoryBytes {
    bytes: Vec<u8>,
    _original: MemoryClaim,
    _metadata: MemoryClaim,
}

/// Every clone shares the same immutable bytes and original execution reservation.
/// No Weak observer escapes; final extraction frees Arc control before returning claims.
pub(super) struct MemoryOriginal(Option<Arc<MemoryBytes>>);
impl MemoryOriginal {
    /// Validate existing admission and obtain shared metadata credit before transferring custody.
    pub(super) fn new(
        input: &mut Option<(Vec<u8>, MemoryClaim)>,
        resident: &MemoryLease,
    ) -> Result<Self, ImportError> {
        let (bytes, claim) =
            input.as_ref().ok_or(ImportError::Invalid("Memory original is unavailable"))?;
        let required = bytes
            .capacity()
            .checked_add(128)
            .ok_or(ImportError::Invalid("Memory original capacity overflow"))?;
        if bytes.is_empty()
            || bytes.len() as u64 > crate::MAX_ENCODED_SOURCE_BYTES
            || claim.bytes() < required as u64
        {
            return Err(ImportError::Invalid(
                "Memory original requires admitted complete encoded storage",
            ));
        }
        let metadata = resident.claim(size_of::<MemoryBytes>() as u64 + 128)?;
        let (bytes, original) =
            input.take().ok_or(ImportError::Invalid("Memory original is unavailable"))?;
        Ok(Self(Some(Arc::new(MemoryBytes { bytes, _original: original, _metadata: metadata }))))
    }
    /// Borrow immutable bytes while this original's reservation remains retained.
    pub(super) fn bytes(&self) -> Result<&[u8], ImportError> {
        self.0
            .as_ref()
            .map(|original| original.bytes.as_slice())
            .ok_or(ImportError::Invalid("Memory original has retired"))
    }
}
impl Clone for MemoryOriginal {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl Drop for MemoryOriginal {
    fn drop(&mut self) {
        // Arc::into_inner frees the final Arc allocation before the extracted fields release credit.
        // Exactly one simultaneous final clone receives the contents; no Weak observer is exported.
        drop(self.0.take().and_then(Arc::into_inner));
    }
}
