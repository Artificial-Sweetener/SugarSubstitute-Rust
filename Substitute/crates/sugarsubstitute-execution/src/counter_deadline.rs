// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Translate immutable counter deadlines through the existing execution clock.
use crate::native::NativeObservationError;
use std::{error::Error, fmt, time::Duration};
use sugarsubstitute_contracts::execution::{CounterDeadline, Moment, TimeError};

pub(crate) fn encode_sampled(
    counter: impl FnOnce() -> Result<(u64, u64), NativeObservationError>,
    now: impl FnOnce() -> Result<Moment, NativeObservationError>,
    original: Moment,
) -> Result<CounterDeadline, NativeObservationError> {
    let counter = counter()?;
    let remaining = original.checked_duration_since(now()?).map_err(arithmetic)?;
    encode(counter, remaining)
}
pub(crate) fn decode_sampled(
    now: impl FnOnce() -> Result<Moment, NativeObservationError>,
    counter: impl FnOnce() -> Result<(u64, u64), NativeObservationError>,
    original: CounterDeadline,
) -> Result<Moment, NativeObservationError> {
    let before = now()?;
    decode(before, counter()?, original)
}
pub(crate) fn encode(
    counter: (u64, u64),
    remaining: Duration,
) -> Result<CounterDeadline, NativeObservationError> {
    let (ticks, frequency) = positive(counter)?;
    let delta = remaining
        .as_nanos()
        .checked_mul(u128::from(frequency))
        .ok_or_else(|| arithmetic(TimeError::Overflow))?
        / 1_000_000_000;
    if delta == 0 {
        return Err(invalid(ConversionError::Expired));
    }
    let until = u128::from(ticks)
        .checked_add(delta)
        .and_then(|value| i64::try_from(value).ok())
        .ok_or_else(|| arithmetic(TimeError::Overflow))?;
    CounterDeadline::new(until, frequency as i64)
        .map_err(|cause| NativeObservationError::new("counter_deadline_shape", cause))
}
pub(crate) fn decode(
    before: Moment,
    counter: (u64, u64),
    original: CounterDeadline,
) -> Result<Moment, NativeObservationError> {
    decode_if_live(before, counter, original)?.ok_or_else(|| invalid(ConversionError::Expired))
}
/// Distinguish an expired diagnostic cutoff from malformed clock facts.
pub(crate) fn decode_if_live(
    before: Moment,
    counter: (u64, u64),
    original: CounterDeadline,
) -> Result<Option<Moment>, NativeObservationError> {
    let (ticks, frequency) = positive(counter)?;
    if frequency != original.frequency() as u64 {
        return Err(invalid(ConversionError::FrequencyMismatch));
    }
    if ticks >= original.ticks() as u64 {
        return Ok(None);
    }
    let remaining = u128::from(original.ticks() as u64 - ticks)
        .checked_mul(1_000_000_000)
        .ok_or_else(|| arithmetic(TimeError::Overflow))?
        / u128::from(frequency);
    let remaining = u64::try_from(remaining).map_err(|_| arithmetic(TimeError::Overflow))?;
    if remaining == 0 {
        return Ok(None);
    }
    before.checked_add(Duration::from_nanos(remaining)).map(Some).map_err(arithmetic)
}
fn positive(counter: (u64, u64)) -> Result<(u64, u64), NativeObservationError> {
    let (ticks, frequency) = counter;
    if ticks == 0 || frequency == 0 || ticks > i64::MAX as u64 || frequency > i64::MAX as u64 {
        return Err(invalid(ConversionError::InvalidCounter));
    }
    Ok(counter)
}
#[derive(Debug)]
enum ConversionError {
    InvalidCounter,
    FrequencyMismatch,
    Expired,
}
impl fmt::Display for ConversionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidCounter => "Native counter facts must be positive signed values",
            Self::FrequencyMismatch => {
                "Native counter frequency differs from the enrolled deadline"
            }
            Self::Expired => "Original deadline has no representable positive remaining interval",
        })
    }
}
impl Error for ConversionError {}
fn invalid(cause: ConversionError) -> NativeObservationError {
    NativeObservationError::new("counter_deadline_conversion", cause)
}
fn arithmetic(cause: TimeError) -> NativeObservationError {
    NativeObservationError::new("counter_deadline_arithmetic", cause)
}
#[cfg(test)]
#[path = "counter_deadline_tests.rs"]
mod tests;
