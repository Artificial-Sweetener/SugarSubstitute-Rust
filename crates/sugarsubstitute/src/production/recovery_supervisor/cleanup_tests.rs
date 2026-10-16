// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Busy scheduling and repeated losses cannot renew capture or containment.
use super::*;
use std::error::Error;
#[test]
fn root_loss_preserves_sixty_seconds_before_original_fifteen_second_containment()
-> Result<(), Box<dyn Error>> {
    let mut owner = Cleanup::new();
    owner.capture(Moment::from_nanos(100))?;
    let cutoff = Moment::from_nanos(60_000_000_100);
    assert_eq!(owner.until(), Some(cutoff));
    owner.capture(Moment::from_nanos(50_000_000_100))?;
    assert!(owner.advance(Moment::from_nanos(60_000_000_099))?.is_none());
    let budget = owner.advance(Moment::from_nanos(90_000_000_100))?.ok_or("Containment absent")?;
    assert_eq!(budget.started, cutoff);
    assert_eq!(budget.settle_until, Moment::from_nanos(75_000_000_100));
    owner.capture(Moment::from_nanos(100_000_000_100))?;
    assert_eq!(owner.advance(Moment::from_nanos(110_000_000_100))?, Some(budget));
    Ok(())
}
#[test]
fn authenticated_root_stop_shortens_once_without_renewing_after_repeated_frames()
-> Result<(), Box<dyn Error>> {
    let mut owner = Cleanup::new();
    owner.capture(Moment::from_nanos(1))?;
    owner.root_stop(Moment::from_nanos(10), Moment::from_nanos(100))?;
    let first = owner.containment().ok_or("Original Root budget absent")?;
    owner.root_stop(Moment::from_nanos(50), Moment::from_nanos(1_000))?;
    assert_eq!(owner.containment(), Some(first));
    owner.root_stop(Moment::from_nanos(60), Moment::from_nanos(80))?;
    assert_eq!(owner.containment().map(|budget| budget.settle_until), Some(Moment::from_nanos(80)));
    assert_eq!(owner.containment().map(|budget| budget.started), Some(Moment::from_nanos(10)));
    Ok(())
}
#[test]
fn overflow_preserves_dormant_state_and_does_not_authorize_cleanup() {
    let mut owner = Cleanup::new();
    assert!(owner.capture(Moment::from_nanos(u64::MAX)).is_err());
    assert!(!owner.requested());
    assert!(owner.until().is_none());
}
