// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep original execution charges attached to actual image import allocations.
use crate::ImportError;
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};

pub(crate) struct Buffer<T> {
    pub(crate) data: Vec<T>,
    _memory: MemoryClaim,
}
impl<T: Clone> Buffer<T> {
    pub(crate) fn new(length: usize, value: T, memory: &MemoryLease) -> Result<Self, ImportError> {
        let bytes = length
            .checked_mul(size_of::<T>())
            .and_then(|bytes| bytes.checked_add(128))
            .ok_or(ImportError::Invalid("Image import allocation size overflow"))?;
        let claim = memory.claim(bytes as u64)?;
        let mut data = Vec::new();
        data.try_reserve_exact(length)?;
        if data.capacity() != length {
            return Err(ImportError::Invalid("Image import allocation exceeds admission"));
        }
        data.resize(length, value);
        Ok(Self { data, _memory: claim })
    }
}
