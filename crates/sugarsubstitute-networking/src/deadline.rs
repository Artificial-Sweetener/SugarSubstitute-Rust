// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Convert the original finite job budget without selecting native infinite timeouts.
use sugarsubstitute_contracts::execution::Moment;

/// Invalid or elapsed original deadline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeadlineError {
    /// The caller supplied no deadline or a past deadline.
    Invalid,
    /// No complete millisecond remains; rounding upward would extend the deadline.
    Elapsed,
}
impl std::fmt::Display for DeadlineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP deadline {self:?}")
    }
}
impl std::error::Error for DeadlineError {}

/// Floor remaining nanoseconds to positive native milliseconds, optionally capped by idle.
/// # Errors
/// Rejects absent/past deadlines, zero idle caps and submillisecond remainders.
pub fn milliseconds(
    deadline: Option<Moment>,
    now: Moment,
    idle: Option<u32>,
) -> Result<i32, DeadlineError> {
    let remaining = deadline
        .ok_or(DeadlineError::Invalid)?
        .as_nanos()
        .checked_sub(now.as_nanos())
        .ok_or(DeadlineError::Invalid)?
        / 1_000_000;
    let value = remaining.min(u64::from(idle.unwrap_or(i32::MAX as u32)));
    if value == 0 {
        return Err(DeadlineError::Elapsed);
    }
    Ok(value.min(i32::MAX as u64) as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn floors_original_budget_and_preserves_full_readiness_phase() -> Result<(), DeadlineError> {
        let now = Moment::from_nanos(1_123_000_000);
        let until = Some(Moment::from_nanos(3_000_000_000));
        assert_eq!(milliseconds(until, now, None)?, 1877);
        assert_eq!(milliseconds(until, now, Some(50))?, 50);
        assert_eq!(
            milliseconds(Some(Moment::from_nanos(1_999_999)), Moment::from_nanos(0), None)?,
            1
        );
        Ok(())
    }
    #[test]
    fn missing_past_submillisecond_and_zero_idle_never_select_infinite() {
        assert_eq!(milliseconds(None, Moment::from_nanos(0), None), Err(DeadlineError::Invalid));
        assert_eq!(
            milliseconds(Some(Moment::from_nanos(9)), Moment::from_nanos(10), None),
            Err(DeadlineError::Invalid)
        );
        for nanos in [0, 1, 999_999] {
            assert_eq!(
                milliseconds(Some(Moment::from_nanos(nanos)), Moment::from_nanos(0), None),
                Err(DeadlineError::Elapsed)
            );
        }
        assert_eq!(
            milliseconds(Some(Moment::from_nanos(u64::MAX)), Moment::from_nanos(0), Some(0)),
            Err(DeadlineError::Elapsed)
        );
    }
    #[test]
    fn maximum_clock_and_integer_never_wrap() -> Result<(), DeadlineError> {
        for nanos in [i32::MAX as u64 * 1_000_000, (i32::MAX as u64 + 1) * 1_000_000, u64::MAX] {
            assert_eq!(
                milliseconds(Some(Moment::from_nanos(nanos)), Moment::from_nanos(0), None)?,
                i32::MAX
            );
        }
        assert_eq!(
            milliseconds(
                Some(Moment::from_nanos(u64::MAX)),
                Moment::from_nanos(u64::MAX - 1_000_000),
                None
            )?,
            1
        );
        Ok(())
    }
}
