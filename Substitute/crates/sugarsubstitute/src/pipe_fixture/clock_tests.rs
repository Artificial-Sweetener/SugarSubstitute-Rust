// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Protect conservative translation of the single original campaign deadline.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use sugarsubstitute_contracts::execution::WaitFuture;

struct OrderedClock(AtomicBool);
impl Clock for OrderedClock {
    fn now(&self) -> Moment {
        self.0.store(true, Ordering::Release);
        Moment::from_nanos(100)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}

#[test]
fn remaining_time_is_added_to_the_epoch_sample_before_native_sampling() -> Result<()> {
    let clock = OrderedClock(AtomicBool::new(false));
    let native = Instant::now();
    let deadline = native.checked_add(Duration::from_secs(10)).ok_or("Test deadline overflow")?;
    let sampled = native.checked_add(Duration::from_secs(3)).ok_or("Test sample overflow")?;
    let result = original_deadline(&clock, deadline, || {
        assert!(clock.0.load(Ordering::Acquire));
        sampled
    })?;
    assert_eq!(result, Moment::from_nanos(7_000_000_100));
    assert_eq!(deadline.checked_duration_since(native), Some(Duration::from_secs(10)));
    Ok(())
}

#[test]
fn an_expired_original_deadline_is_rejected_after_ordered_sampling() -> Result<()> {
    let clock = OrderedClock(AtomicBool::new(false));
    let deadline = Instant::now();
    let late = deadline.checked_add(Duration::from_secs(1)).ok_or("Test sample overflow")?;
    assert!(
        original_deadline(&clock, deadline, || {
            assert!(clock.0.load(Ordering::Acquire));
            late
        })
        .is_err()
    );
    Ok(())
}
