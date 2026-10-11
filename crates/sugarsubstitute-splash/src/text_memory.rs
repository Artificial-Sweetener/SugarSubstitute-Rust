// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound owned text storage separately from opaque platform font allocations.

use std::{io, num::NonZeroUsize};
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};

pub(super) const MAX_GLYPHS: usize = 4096;
pub(super) const MAX_RUNS: usize = 256;
pub(super) const METADATA_BYTES: usize = 1024 * 1024;
pub(super) const TEXTURE_BYTES: usize = 4 * 1024 * 1024;

/// Keep immutable paint coordinates and destination identity together.
#[derive(Clone, Copy)]
pub(super) struct PaintSpec {
    pub(super) theme: crate::Theme,
    pub(super) scale: f32,
    pub(super) width: u32,
    pub(super) height: u32,
}

/// Distinguish admitted production allocations from synchronous qualification.
pub(super) struct Allocation(pub(super) Option<MemoryLease>);

impl Allocation {
    /// Claim a conservative allocation capacity before requesting allocator storage.
    pub(super) fn claim(&self, bytes: usize) -> io::Result<Option<MemoryClaim>> {
        self.0.as_ref().map(|lease| lease.claim(bytes as u64).map_err(io::Error::other)).transpose()
    }
}

/// Reserve exact vector capacity without filling it or hiding allocator failures.
pub(super) fn reserve<T>(count: usize, byte_limit: usize) -> io::Result<Vec<T>> {
    let requested = count
        .checked_mul(size_of::<T>())
        .ok_or_else(|| io::Error::other("Status allocation capacity overflow"))?;
    if requested > byte_limit {
        return Err(io::Error::other("Status allocation exceeds claimed capacity"));
    }
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(io::Error::other)?;
    let bytes = values
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or_else(|| io::Error::other("Status allocation capacity overflow"))?;
    if bytes > requested {
        return Err(io::Error::other("Status allocation exceeds claimed capacity"));
    }
    Ok(values)
}

/// Reject external inputs before any destination mutation or shaping allocation.
pub(super) fn validate(message: &str, scale: f32, width: u32, height: u32) -> io::Result<()> {
    if !scale.is_finite()
        || scale <= 0.0
        || scale > 4.0
        || width == 0
        || height == 0
        || width > 4096
        || height > 4096
        || message.encode_utf16().take(MAX_GLYPHS + 1).count() > MAX_GLYPHS
    {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid bounded status input"));
    }
    Ok(())
}

/// Clamp authored initialization/compositing work to one cooperative pixel budget.
pub(super) fn budget(max: NonZeroUsize) -> usize {
    max.get().min(65_536)
}
