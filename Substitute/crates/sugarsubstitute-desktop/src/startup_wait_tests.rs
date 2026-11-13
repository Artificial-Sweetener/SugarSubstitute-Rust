// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep native incomplete-sample wakes independent of presentation and original cutoffs.
use super::*;
use std::time::Duration;
use sugarsubstitute_execution::qualification_time::anchor;

#[test]
fn incomplete_probe_wakes_inactive_and_retired_splash_at_its_native_deadline() {
    let now = anchor();
    let probe = now + Duration::from_millis(100);
    let original = now + Duration::from_secs(30);
    let deadline = wait_deadline(Some(original), None, Some(probe));
    for (visible, retired, frame) in [
        (false, false, None),
        (false, true, None),
        (true, true, Some(now + Duration::from_millis(16))),
        (true, false, None),
    ] {
        assert_eq!(
            presentation_wait(visible, retired, frame, deadline),
            ControlFlow::WaitUntil(probe)
        );
    }
    assert_eq!(wait_deadline(Some(original), None, Some(original)), Some(original));
    assert_eq!(
        wait_deadline(Some(original), None, Some(original + Duration::from_millis(1))),
        Some(original)
    );
    assert_eq!(wait_deadline(Some(original), Some(now), Some(probe)), Some(now));
    assert_eq!(wait_deadline(Some(now), None, Some(probe)), Some(now));
}

#[test]
fn pending_or_completed_probe_adds_no_native_polling_wake() {
    let now = anchor();
    let original = now + Duration::from_secs(30);
    assert_eq!(wait_deadline(Some(original), None, None), Some(original));
    assert_eq!(
        presentation_wait(false, true, None, wait_deadline(None, None, None)),
        ControlFlow::Wait
    );
    assert_eq!(
        presentation_wait(false, true, None, wait_deadline(Some(original), None, None)),
        ControlFlow::WaitUntil(original)
    );
}
