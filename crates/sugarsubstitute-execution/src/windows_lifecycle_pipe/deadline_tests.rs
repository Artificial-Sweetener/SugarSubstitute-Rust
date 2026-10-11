// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove fixed parent/child deadlines use conservative flooring across clock epochs.
use super::*;
use std::time::Duration;
#[test]
fn parent_flooring_never_extends_remaining_time() -> Result<(), Box<dyn std::error::Error>> {
    let deadline = parent((100, 3), Duration::from_nanos(666_666_667))?;
    assert_eq!(deadline.ticks(), 102);
    assert_eq!(deadline.frequency(), 3);
    assert!(parent((100, 3), Duration::from_nanos(333_333_333)).is_err());
    assert!(parent((i64::MAX as u64, 1), Duration::from_secs(1)).is_err());
    assert!(parent((1, u64::MAX), Duration::from_secs(1)).is_err());
    Ok(())
}
#[test]
fn child_uses_before_sample_epoch_exact_frequency_and_floor()
-> Result<(), Box<dyn std::error::Error>> {
    let original = LifecyclePipeDeadline::new(102, 3)?;
    assert_eq!(child(Moment::from_nanos(50), (100, 3), original)?, Moment::from_nanos(666_666_716));
    for counter in [(102, 3), (103, 3), (0, 3), (100, 2), (100, 0)] {
        assert!(child(Moment::from_nanos(50), counter, original).is_err());
    }
    assert!(child(Moment::from_nanos(u64::MAX), (100, 3), original).is_err());
    Ok(())
}
#[test]
fn deadline_shapes_reject_nonpositive_and_unrepresentable_remaining_time()
-> Result<(), Box<dyn std::error::Error>> {
    for (ticks, frequency) in [(0, 1), (1, 0), (-1, 1), (1, -1)] {
        assert!(LifecyclePipeDeadline::new(ticks, frequency).is_err());
    }
    assert!(parent((0, 1), Duration::from_secs(1)).is_err());
    assert!(parent((1, 0), Duration::from_secs(1)).is_err());
    assert!(
        child(Moment::from_nanos(0), (1, 1), LifecyclePipeDeadline::new(i64::MAX, 1)?).is_err()
    );
    Ok(())
}
