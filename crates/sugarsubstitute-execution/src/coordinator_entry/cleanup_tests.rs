// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject renewal and arithmetic retry at the cleanup decision owner.
use super::*;
#[test]
fn root_serving_never_creates_a_private_shutdown_deadline() -> Result<(), EntryTransportError> {
    let mut cleanup = Cleanup::Pending;
    cleanup.begin(true, Moment::from_nanos(u64::MAX), true)?;
    cleanup.begin(false, Moment::from_nanos(0), true)?;
    assert!(matches!(cleanup, Cleanup::RootServing));
    assert_eq!(cleanup.deadline(Moment::from_nanos(0), true), None);
    assert!(!cleanup.take_expiry(Moment::from_nanos(u64::MAX), true));
    Ok(())
}
#[test]
fn first_private_budget_expiry_is_reported_once_without_renewal() -> Result<(), EntryTransportError>
{
    let mut cleanup = Cleanup::Pending;
    cleanup.begin(false, Moment::from_nanos(7), true)?;
    cleanup.begin(false, Moment::from_nanos(9_000_000_007), true)?;
    assert_eq!(
        cleanup.deadline(Moment::from_nanos(7), true),
        Some(Moment::from_nanos(10_000_000_007))
    );
    assert_eq!(
        cleanup.deadline(Moment::from_nanos(10_000_000_007), true),
        Some(Moment::from_nanos(15_000_000_007))
    );
    assert!(cleanup.take_expiry(Moment::from_nanos(15_000_000_007), true));
    assert!(!cleanup.take_expiry(Moment::from_nanos(30_000_000_007), true));
    cleanup.begin(false, Moment::from_nanos(30_000_000_007), true)?;
    assert_eq!(cleanup.deadline(Moment::from_nanos(30_000_000_007), true), None);
    Ok(())
}
#[test]
fn failed_first_budget_or_clock_cannot_retry_arithmetic() -> Result<(), EntryTransportError> {
    let mut cleanup = Cleanup::Pending;
    assert!(cleanup.begin(false, Moment::from_nanos(u64::MAX), true).is_err());
    cleanup.begin(false, Moment::from_nanos(0), true)?;
    assert!(matches!(cleanup, Cleanup::Failed));
    let mut unavailable = Cleanup::Pending;
    unavailable.begin(false, Moment::from_nanos(0), false)?;
    unavailable.begin(false, Moment::from_nanos(0), true)?;
    assert!(matches!(unavailable, Cleanup::Failed));
    assert_eq!(unavailable.deadline(Moment::from_nanos(0), true), None);
    Ok(())
}
