// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Conservatively translate a fixed parent duration after its enrolled QPC sample.
use super::{FixtureDeadline, NativeObservationError};
use std::time::Duration;

pub(super) fn sampled(
    counter: impl FnOnce() -> Result<(u64, u64), NativeObservationError>,
    remaining: impl FnOnce() -> Option<Duration>,
) -> Result<FixtureDeadline, NativeObservationError> {
    let (ticks, frequency) = counter()?;
    let remaining = remaining().ok_or_else(invalid)?;
    if ticks == 0 || frequency == 0 || ticks > i64::MAX as u64 || frequency > i64::MAX as u64 {
        return Err(invalid());
    }
    let delta = remaining.as_nanos().checked_mul(u128::from(frequency)).ok_or_else(invalid)?
        / 1_000_000_000;
    let delta = u64::try_from(delta).map_err(|_| invalid())?;
    if delta == 0 {
        return Err(invalid());
    }
    let deadline = ticks.checked_add(delta).ok_or_else(invalid)?;
    FixtureDeadline::new(deadline, frequency).map_err(|_| invalid())
}
fn invalid() -> NativeObservationError {
    NativeObservationError::new(
        "fixture_parent_deadline",
        std::io::Error::other("Fixed parent deadline is expired or unrepresentable"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, error::Error};
    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn counter_is_sampled_before_remaining_original_duration() -> TestResult {
        let sampled_counter = Cell::new(false);
        let deadline = sampled(
            || {
                sampled_counter.set(true);
                Ok((100, 3))
            },
            || {
                assert!(sampled_counter.get());
                Some(Duration::from_millis(500))
            },
        )?;
        assert_eq!((deadline.ticks(), deadline.frequency()), (101, 3));
        Ok(())
    }

    #[test]
    fn flooring_never_extends_original_and_requires_one_whole_tick() -> TestResult {
        assert!(sampled(|| Ok((10, 3)), || Some(Duration::from_nanos(333_333_333))).is_err());
        assert_eq!(
            sampled(|| Ok((10, 3)), || Some(Duration::from_nanos(333_333_334)))?.ticks(),
            11
        );
        assert_eq!(
            sampled(|| Ok((10, 3)), || Some(Duration::from_nanos(666_666_666)))?.ticks(),
            11
        );
        Ok(())
    }

    #[test]
    fn expired_zero_invalid_and_overflow_are_rejected_without_deadline() {
        assert!(sampled(|| Ok((10, 3)), || None).is_err());
        assert!(sampled(|| Ok((10, 3)), || Some(Duration::ZERO)).is_err());
        for counter in [(0, 3), (10, 0), (u64::MAX, 3), (10, u64::MAX)] {
            assert!(sampled(|| Ok(counter), || Some(Duration::from_secs(1))).is_err());
        }
        assert!(sampled(|| Ok((i64::MAX as u64, 1)), || Some(Duration::from_secs(1))).is_err());
        assert_eq!(
            sampled(|| Ok((i64::MAX as u64 - 1, 1)), || Some(Duration::from_secs(1)))
                .map(|deadline| deadline.ticks())
                .ok(),
            Some(i64::MAX as u64)
        );
        assert!(sampled(|| Ok((10, 3)), || Some(Duration::MAX)).is_err());
        assert!(sampled(|| Ok((10, i64::MAX as u64)), || Some(Duration::MAX)).is_err());
    }

    #[test]
    fn native_counter_failure_keeps_original_cause_and_never_reads_remaining() -> TestResult {
        let remaining_calls = Cell::new(0);
        let result = sampled(
            || {
                Err(NativeObservationError::new(
                    "fixture_qpc_counter",
                    std::io::Error::from_raw_os_error(5),
                ))
            },
            || {
                remaining_calls.set(remaining_calls.get() + 1);
                Some(Duration::from_secs(1))
            },
        );
        let Err(error) = result else { return Err("Counter failure produced deadline".into()) };
        assert_eq!(remaining_calls.get(), 0);
        assert_eq!(error.stage(), "fixture_qpc_counter");
        assert_eq!(
            error
                .source()
                .and_then(|cause| cause.downcast_ref::<std::io::Error>())
                .and_then(std::io::Error::raw_os_error),
            Some(5)
        );
        Ok(())
    }
}
