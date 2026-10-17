// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual focus policy with hidden and available splash-owned controls.
use super::*;
use crate::ControlBounds;
fn controls(retry: bool) -> [AccessibleControl; 3] {
    [
        AccessibleControl {
            id: 1,
            label: "Show output",
            bounds: ControlBounds { x: 466.0, y: 0.0, width: 46.0, height: 32.0 },
            close: false,
            visible: true,
        },
        AccessibleControl {
            id: 2,
            label: "Cancel",
            bounds: ControlBounds { x: 512.0, y: 0.0, width: 46.0, height: 32.0 },
            close: true,
            visible: true,
        },
        AccessibleControl {
            id: 3,
            label: "Retry",
            bounds: ControlBounds { x: 24.0, y: 486.0, width: 510.0, height: 48.0 },
            close: false,
            visible: retry,
        },
    ]
}
#[test]
fn tab_and_reverse_tab_skip_hidden_peers_and_do_not_activate_disclosure_or_close() {
    let controls = controls(false);
    let mut current = None;
    let mut observed = Vec::new();
    for _ in 0..4 {
        current = next(controls.iter().copied(), None, current, false);
        observed.push(current);
    }
    assert_eq!(
        observed,
        [
            Some(Target::Control(1)),
            Some(Target::Control(2)),
            Some(Target::Control(1)),
            Some(Target::Control(2))
        ]
    );
    assert_eq!(
        next(controls.iter().copied(), None, Some(Target::Control(1)), true),
        Some(Target::Control(2))
    );
    assert_eq!(
        next(controls.iter().copied(), None, Some(Target::Control(99)), false),
        Some(Target::Control(1))
    );
    assert_eq!(
        next(controls.iter().copied(), None, Some(Target::Control(99)), true),
        Some(Target::Control(2))
    );
}
#[test]
fn expanded_output_precedes_lower_retry_and_collapse_removes_only_the_console_target() {
    let controls = controls(true);
    let order = ordered(controls.iter().copied(), Some(362.0)).collect::<Vec<_>>();
    assert_eq!(
        order,
        [Target::Control(1), Target::Control(2), Target::Console, Target::Control(3)]
    );
    assert_eq!(
        next(controls.iter().copied(), Some(362.0), Some(Target::Console), true),
        Some(Target::Control(2))
    );
    assert_eq!(
        next(controls.iter().copied(), None, Some(Target::Console), false),
        Some(Target::Control(1))
    );
    let hidden = controls.map(|control| AccessibleControl { visible: false, ..control });
    assert_eq!(next(hidden.iter().copied(), None, None, false), None);
    assert_eq!(next(hidden.iter().copied(), Some(362.0), None, true), Some(Target::Console));
}
