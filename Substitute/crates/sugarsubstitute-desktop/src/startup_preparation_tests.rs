// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify original deadline selection and cleanup wake cadence without an OS event loop.
use super::*;
use std::time::Duration;

#[test]
fn preparation_preserves_the_earliest_original_deadline_including_expired_equality() {
    let now = Instant::now();
    let later = now + Duration::from_secs(30);
    let earlier = now + Duration::from_secs(2);
    assert_eq!(
        preparation_wait(false, Some(later), Some(earlier), now),
        ControlFlow::WaitUntil(earlier)
    );
    assert_eq!(
        preparation_wait(false, Some(earlier), Some(later), now),
        ControlFlow::WaitUntil(earlier)
    );
    assert_eq!(preparation_wait(false, Some(now), Some(later), now), ControlFlow::WaitUntil(now));
    assert_eq!(preparation_wait(false, None, None, now), ControlFlow::Wait);
}

#[test]
fn closing_ignores_expired_wake_deadlines_without_renewing_future_owner_deadlines() {
    let now = Instant::now();
    let original = now + Duration::from_millis(1);
    assert_eq!(preparation_wait(true, Some(original), None, now), ControlFlow::WaitUntil(original));
    assert_eq!(preparation_wait(true, Some(now), None, now), crate::cadence::closing_wait(now));
    assert_eq!(preparation_wait(true, None, None, now), crate::cadence::closing_wait(now));
}
