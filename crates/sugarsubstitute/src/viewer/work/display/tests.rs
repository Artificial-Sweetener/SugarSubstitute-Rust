// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise proof-readback retry decisions without native windows or GPU allocation.

use super::*;
use sugarsubstitute_contracts::execution::MemoryClaimError;

fn pressure() -> MemoryClaimError {
    MemoryClaimError { requested_bytes: 65_792, available_bytes: 60_680 }
}

#[test]
fn readback_memory_pressure_retries_only_after_reclaim_progress() -> Result<(), Box<dyn Error>> {
    let mut attempts = 0;
    let mut reclaims = 0;
    let outcome = retry_after_cache_reclaim(
        || {
            attempts += 1;
            if attempts == 1 { Err(pressure()) } else { Ok(37) }
        },
        || {
            reclaims += 1;
            Ok(CacheReclaim::Progress)
        },
        |_| true,
    )?;
    assert!(matches!(outcome, ReadbackOutcome::Complete(37)));
    assert_eq!(attempts, 2);
    assert_eq!(reclaims, 1);
    Ok(())
}

#[test]
fn busy_readback_reclaim_defers_without_repeating_the_allocation() -> Result<(), Box<dyn Error>> {
    let mut attempts = 0;
    let outcome = retry_after_cache_reclaim(
        || {
            attempts += 1;
            Err::<(), _>(pressure())
        },
        || Ok(CacheReclaim::Busy),
        |_| true,
    )?;
    assert!(matches!(outcome, ReadbackOutcome::Deferred));
    assert_eq!(attempts, 1);
    Ok(())
}

#[test]
fn exhausted_readback_reclaim_preserves_the_admission_failure() -> Result<(), Box<dyn Error>> {
    let error = retry_after_cache_reclaim(
        || Err::<(), _>(pressure()),
        || Ok(CacheReclaim::Exhausted),
        |_| true,
    )
    .err()
    .ok_or("exhausted cache accepted a failed readback")?;
    let cause = error
        .source()
        .and_then(|cause| cause.downcast_ref::<MemoryClaimError>())
        .ok_or("missing original admission failure")?;
    assert_eq!(cause.requested_bytes, 65_792);
    assert_eq!(cause.available_bytes, 60_680);
    Ok(())
}

#[test]
fn nonmemory_readback_failure_is_not_retried_or_replaced() -> Result<(), Box<dyn Error>> {
    let mut reclaims = 0;
    let error = retry_after_cache_reclaim(
        || Err::<(), _>(std::io::Error::from_raw_os_error(123)),
        || {
            reclaims += 1;
            Ok(CacheReclaim::Progress)
        },
        |_| false,
    )
    .err()
    .ok_or("driver failure was accepted")?;
    let cause = error
        .source()
        .and_then(|cause| cause.downcast_ref::<std::io::Error>())
        .ok_or("missing original driver failure")?;
    assert_eq!(cause.raw_os_error(), Some(123));
    assert_eq!(reclaims, 0);
    Ok(())
}
