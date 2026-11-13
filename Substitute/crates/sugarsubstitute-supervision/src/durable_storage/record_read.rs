// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own bounded record byte accounting and returned I/O under distinct allocation controls.
use std::{
    collections::TryReserveError,
    io::{self, Read},
};
const CHUNK: usize = 8 * 1024;

/// Keep actual I/O and control outcomes distinct at the external read boundary.
pub(super) enum ReadFailure<C> {
    Control(C),
    Read { source: io::Error, after: Option<C> },
    Capacity(TryReserveError),
    TooLarge,
}

/// Preparation reserves its full extent; ordinary records use bounded incremental chunks.
#[derive(Clone, Copy)]
pub(super) enum Allocation {
    Exact,
    Incremental { observed: usize },
}

/// Preserve the caller's existing interpretation of an interrupted native read.
#[derive(Clone, Copy)]
pub(super) enum InterruptedReads {
    Propagate,
    Resume,
}

/// The returned outcome stays owned while the mandatory observer runs lazily.
pub(super) fn observe<T, C>(
    outcome: io::Result<T>,
    after: impl FnOnce() -> Result<(), C>,
) -> Result<T, ReadFailure<C>> {
    let post = after();
    match (outcome, post) {
        (Err(source), after) => Err(ReadFailure::Read { source, after: after.err() }),
        (Ok(value), Ok(())) => Ok(value),
        (Ok(_), Err(cause)) => Err(ReadFailure::Control(cause)),
    }
}

/// Read through one sentinel bound, owning every outcome before its control observation.
pub(super) fn bounded<C>(
    mut source: impl Read,
    maximum: usize,
    allocation: Allocation,
    interrupted: InterruptedReads,
    mut before: impl FnMut() -> Result<(), C>,
    mut after: impl FnMut() -> Result<(), C>,
) -> Result<Vec<u8>, ReadFailure<C>> {
    before().map_err(ReadFailure::Control)?;
    let limit = maximum.checked_add(1).ok_or(ReadFailure::TooLarge)?;
    let capacity = match allocation {
        Allocation::Exact => limit,
        Allocation::Incremental { observed } if observed <= maximum => observed.min(CHUNK - 1) + 1,
        Allocation::Incremental { .. } => return Err(ReadFailure::TooLarge),
    };
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(capacity).map_err(ReadFailure::Capacity)?;
    if bytes.capacity() != capacity {
        return Err(ReadFailure::TooLarge);
    }
    bytes.resize(capacity, 0);
    let mut used = 0;
    loop {
        before().map_err(ReadFailure::Control)?;
        if used == bytes.len() {
            let next = used.saturating_mul(2).max(used.saturating_add(CHUNK));
            let capacity = limit.min(next);
            bytes.try_reserve_exact(capacity - used).map_err(ReadFailure::Capacity)?;
            if bytes.capacity() != capacity {
                return Err(ReadFailure::TooLarge);
            }
            bytes.resize(capacity, 0);
        }
        let available = bytes.len() - used;
        let actual = match observe(source.read(&mut bytes[used..]), &mut after) {
            Err(ReadFailure::Read { source, after: None })
                if source.kind() == io::ErrorKind::Interrupted
                    && matches!(interrupted, InterruptedReads::Resume) =>
            {
                continue;
            }
            other => other?,
        };
        if actual > available {
            return Err(ReadFailure::TooLarge);
        }
        if actual == 0 {
            bytes.truncate(used);
            return Ok(bytes);
        }
        used += actual;
        if used > maximum {
            return Err(ReadFailure::TooLarge);
        }
    }
}

#[cfg(test)]
mod tests;
