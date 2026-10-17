// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound native timeout conversion by the exact remaining admitted job budget.
use super::timeout_milliseconds;
use crate::readiness_transport::ProbeRetryDisposition;
use std::error::Error as _;
use sugarsubstitute_contracts::execution::Moment;

#[test]
fn full_probe_and_remaining_phase_budget_are_not_capped_at_one_hundred_milliseconds()
-> Result<(), Box<dyn std::error::Error>> {
    let until = Some(Moment::from_nanos(3_000_000_000));
    assert_eq!(timeout_milliseconds(until, Moment::from_nanos(1_000_000_000))?, 2000);
    assert_eq!(timeout_milliseconds(until, Moment::from_nanos(1_123_000_000))?, 1877);
    Ok(())
}
#[test]
fn milliseconds_floor_without_widening_or_infinite_zero() -> Result<(), Box<dyn std::error::Error>>
{
    assert_eq!(
        timeout_milliseconds(Some(Moment::from_nanos(1_999_999)), Moment::from_nanos(0))?,
        1
    );
    assert_eq!(
        timeout_milliseconds(Some(Moment::from_nanos(1_000_000)), Moment::from_nanos(0))?,
        1
    );
    for nanos in [0, 1, 999_999] {
        let error = timeout_milliseconds(Some(Moment::from_nanos(nanos)), Moment::from_nanos(0))
            .err()
            .ok_or("Nonpositive native timeout accepted")?;
        assert_eq!(error.stage(), "http_submillisecond_deadline");
        assert_eq!(error.retry_disposition(), ProbeRetryDisposition::ProbeIntervalElapsed);
        assert_eq!(
            error
                .source()
                .and_then(|cause| cause.downcast_ref::<std::io::Error>())
                .map(std::io::Error::kind),
            Some(std::io::ErrorKind::TimedOut)
        );
    }
    Ok(())
}
#[test]
fn missing_or_past_original_deadline_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    for until in [None, Some(Moment::from_nanos(9))] {
        let error = timeout_milliseconds(until, Moment::from_nanos(10))
            .err()
            .ok_or("Invalid original deadline accepted")?;
        assert_eq!(error.stage(), "http_deadline");
        assert_eq!(error.retry_disposition(), ProbeRetryDisposition::Terminal);
    }
    Ok(())
}
#[test]
fn maximum_clock_and_native_integer_bound_never_wrap_or_select_infinite()
-> Result<(), Box<dyn std::error::Error>> {
    for nanos in [(i32::MAX as u64) * 1_000_000, (i32::MAX as u64 + 1) * 1_000_000, u64::MAX] {
        assert_eq!(
            timeout_milliseconds(Some(Moment::from_nanos(nanos)), Moment::from_nanos(0))?,
            i32::MAX
        );
    }
    assert_eq!(
        timeout_milliseconds(
            Some(Moment::from_nanos(u64::MAX)),
            Moment::from_nanos(u64::MAX - 1_000_000)
        )?,
        1
    );
    Ok(())
}
