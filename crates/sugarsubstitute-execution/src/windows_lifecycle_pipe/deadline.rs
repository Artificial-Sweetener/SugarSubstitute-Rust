// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Convert original handshake deadlines conservatively with checked integer flooring.
use crate::{lifecycle_pipe::LifecyclePipeDeadline, native::NativeObservationError};
use sugarsubstitute_contracts::execution::Moment;
pub(crate) fn child(
    before: Moment,
    counter: (u64, u64),
    original: LifecyclePipeDeadline,
) -> Result<Moment, NativeObservationError> {
    let (ticks, frequency) = counter;
    if frequency == 0
        || frequency != original.frequency as u64
        || ticks == 0
        || ticks >= original.ticks as u64
    {
        return Err(invalid());
    }
    let remaining =
        (original.ticks as u128 - ticks as u128).checked_mul(1_000_000_000).ok_or_else(invalid)?
            / frequency as u128;
    let remaining = u64::try_from(remaining).map_err(|_| invalid())?;
    if remaining == 0 {
        return Err(invalid());
    }
    before.checked_add(std::time::Duration::from_nanos(remaining)).map_err(|_| invalid())
}
pub(crate) fn parent(
    counter: (u64, u64),
    remaining: std::time::Duration,
) -> Result<LifecyclePipeDeadline, NativeObservationError> {
    let (ticks, frequency) = counter;
    if ticks == 0 || frequency == 0 || frequency > i64::MAX as u64 || ticks > i64::MAX as u64 {
        return Err(invalid());
    }
    let delta =
        remaining.as_nanos().checked_mul(frequency as u128).ok_or_else(invalid)? / 1_000_000_000;
    if delta == 0 {
        return Err(invalid());
    }
    let until = (ticks as u128)
        .checked_add(delta)
        .and_then(|value| i64::try_from(value).ok())
        .ok_or_else(invalid)?;
    Ok(LifecyclePipeDeadline { ticks: until, frequency: frequency as i64 })
}
fn invalid() -> NativeObservationError {
    NativeObservationError::new(
        "lifecycle_deadline_conversion",
        std::io::Error::other("Original handshake deadline or counter is invalid"),
    )
}
#[cfg(test)]
#[path = "deadline_tests.rs"]
mod tests;
