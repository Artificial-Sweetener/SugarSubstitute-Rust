// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Prove sampling order, original-budget flooring and retained conversion causes.
use super::*;
use std::cell::Cell;
type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn diagnostic_expiry_is_a_miss_but_invalid_clock_facts_remain_errors() -> TestResult {
    let original = CounterDeadline::new(102, 3)?;
    assert_eq!(decode_if_live(Moment::from_nanos(50), (102, 3), original)?, None);
    assert_eq!(decode_if_live(Moment::from_nanos(50), (103, 3), original)?, None);
    assert_eq!(
        decode_if_live(Moment::from_nanos(50), (100, 3), original)?,
        Some(Moment::from_nanos(666_666_716))
    );
    assert!(decode_if_live(Moment::from_nanos(50), (103, 2), original).is_err());
    assert!(decode_if_live(Moment::from_nanos(50), (0, 3), original).is_err());
    Ok(())
}

#[test]
fn encode_samples_counter_before_the_central_clock() -> TestResult {
    let phase = Cell::new(0);
    let deadline = encode_sampled(
        || {
            assert_eq!(phase.replace(1), 0);
            Ok((100, 3))
        },
        || {
            assert_eq!(phase.replace(2), 1);
            Ok(Moment::from_nanos(20))
        },
        Moment::from_nanos(500_000_020),
    )?;
    assert_eq!(phase.get(), 2);
    assert_eq!((deadline.ticks(), deadline.frequency()), (101, 3));
    Ok(())
}
#[test]
fn decode_samples_central_clock_before_the_counter() -> TestResult {
    let phase = Cell::new(0);
    let until = decode_sampled(
        || {
            assert_eq!(phase.replace(1), 0);
            Ok(Moment::from_nanos(20))
        },
        || {
            assert_eq!(phase.replace(2), 1);
            Ok((100, 3))
        },
        CounterDeadline::new(102, 3)?,
    )?;
    assert_eq!(phase.get(), 2);
    assert_eq!(until, Moment::from_nanos(666_666_686));
    Ok(())
}
#[test]
fn parent_flooring_never_extends_remaining_time() -> TestResult {
    let deadline = encode((100, 3), Duration::from_nanos(666_666_667))?;
    assert_eq!(deadline.ticks(), 102);
    assert_eq!(deadline.frequency(), 3);
    assert!(encode((100, 3), Duration::from_nanos(333_333_333)).is_err());
    assert!(encode((i64::MAX as u64, 1), Duration::from_secs(1)).is_err());
    assert!(encode((1, u64::MAX), Duration::from_secs(1)).is_err());
    Ok(())
}
#[test]
fn child_uses_before_sample_epoch_exact_frequency_and_floor() -> TestResult {
    let original = CounterDeadline::new(102, 3)?;
    assert_eq!(
        decode(Moment::from_nanos(50), (100, 3), original)?,
        Moment::from_nanos(666_666_716)
    );
    for counter in [(102, 3), (103, 3), (0, 3), (100, 2), (100, 0)] {
        assert!(decode(Moment::from_nanos(50), counter, original).is_err());
    }
    let error = decode(Moment::from_nanos(u64::MAX), (100, 3), original)
        .err()
        .ok_or("Overflow accepted")?;
    assert_eq!(
        error.source().and_then(|cause| cause.downcast_ref::<TimeError>()),
        Some(&TimeError::Overflow)
    );
    Ok(())
}
#[test]
fn deadline_shapes_reject_nonpositive_and_unrepresentable_remaining_time() -> TestResult {
    for (ticks, frequency) in [(0, 1), (1, 0), (-1, 1), (1, -1)] {
        assert!(CounterDeadline::new(ticks, frequency).is_err());
    }
    for counter in [(0, 1), (1, 0), (u64::MAX, 1), (1, u64::MAX)] {
        assert!(encode(counter, Duration::from_secs(1)).is_err());
        assert!(decode(Moment::from_nanos(0), counter, CounterDeadline::new(2, 1)?).is_err());
    }
    assert!(decode(Moment::from_nanos(0), (1, 1), CounterDeadline::new(i64::MAX, 1)?).is_err());
    Ok(())
}
#[test]
fn equality_and_past_local_deadlines_do_not_create_a_new_budget() -> TestResult {
    for original in [Moment::from_nanos(40), Moment::from_nanos(39)] {
        assert!(encode_sampled(|| Ok((100, 3)), || Ok(Moment::from_nanos(40)), original).is_err());
    }
    let error =
        encode_sampled(|| Ok((100, 3)), || Ok(Moment::from_nanos(40)), Moment::from_nanos(39))
            .err()
            .ok_or("Past deadline accepted")?;
    assert_eq!(
        error.source().and_then(|cause| cause.downcast_ref::<TimeError>()),
        Some(&TimeError::Regression)
    );
    Ok(())
}
#[test]
fn decode_rejects_sub_nanosecond_remaining_interval_instead_of_rounding_up() -> TestResult {
    assert!(
        decode(
            Moment::from_nanos(1),
            (100, 2_000_000_000),
            CounterDeadline::new(101, 2_000_000_000)?
        )
        .is_err()
    );
    assert_eq!(
        decode(
            Moment::from_nanos(1),
            (100, 2_000_000_000),
            CounterDeadline::new(102, 2_000_000_000)?
        )?,
        Moment::from_nanos(2)
    );
    Ok(())
}
#[test]
fn counter_failure_keeps_original_os_cause_and_skips_central_sample() -> TestResult {
    let central_calls = Cell::new(0);
    let error = encode_sampled(
        || Err(NativeObservationError::new("actual_counter", std::io::Error::from_raw_os_error(5))),
        || {
            central_calls.set(central_calls.get() + 1);
            Ok(Moment::from_nanos(0))
        },
        Moment::from_nanos(1_000_000_000),
    )
    .err()
    .ok_or("Counter failure accepted")?;
    assert_eq!(central_calls.get(), 0);
    assert_eq!(error.stage(), "actual_counter");
    assert_eq!(
        error
            .source()
            .and_then(|cause| cause.downcast_ref::<std::io::Error>())
            .and_then(std::io::Error::raw_os_error),
        Some(5)
    );
    Ok(())
}
#[test]
fn central_failure_keeps_cause_and_decode_does_not_sample_counter() -> TestResult {
    let counter_calls = Cell::new(0);
    let error = decode_sampled(
        || Err(NativeObservationError::new("actual_central_clock", TimeError::Regression)),
        || {
            counter_calls.set(counter_calls.get() + 1);
            Ok((100, 3))
        },
        CounterDeadline::new(102, 3)?,
    )
    .err()
    .ok_or("Central failure accepted")?;
    assert_eq!(counter_calls.get(), 0);
    assert_eq!(error.stage(), "actual_central_clock");
    assert_eq!(
        error.source().and_then(|cause| cause.downcast_ref::<TimeError>()),
        Some(&TimeError::Regression)
    );
    let error = encode_sampled(
        || Ok((100, 3)),
        || Err(NativeObservationError::new("actual_central_clock", TimeError::Regression)),
        Moment::from_nanos(1_000_000_000),
    )
    .err()
    .ok_or("Central encode failure accepted")?;
    assert_eq!(error.stage(), "actual_central_clock");
    Ok(())
}
#[test]
fn arithmetic_overflow_retains_typed_cause_and_never_wraps_ticks() -> TestResult {
    for (counter, remaining) in
        [((1, i64::MAX as u64), Duration::MAX), ((i64::MAX as u64, 1), Duration::from_secs(1))]
    {
        let error = encode(counter, remaining).err().ok_or("Overflow accepted")?;
        assert_eq!(
            error.source().and_then(|cause| cause.downcast_ref::<TimeError>()),
            Some(&TimeError::Overflow)
        );
    }
    let latest = encode((i64::MAX as u64 - 1, 1), Duration::from_secs(1))?;
    assert_eq!(latest.ticks(), i64::MAX);
    Ok(())
}
