// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Characterize exact two-surface eligibility and acknowledged overlapping suspension.
use super::*;
#[test]
fn visible_nonminimized_active_owned_surface_is_required() {
    let mut signals = Signals::new();
    for facts in [
        SurfaceFacts::observed(false, false, true),
        SurfaceFacts::observed(true, true, true),
        SurfaceFacts::observed(true, false, false),
    ] {
        signals.surface(0, facts);
        assert!(signals.take().is_none_or(|value| value == UiWatchdogActivity::Inactive));
    }
    signals.surface(1, SurfaceFacts::observed(true, false, true));
    assert_eq!(signals.take(), Some(UiWatchdogActivity::VisibleActive));
    signals.remove(1);
    assert_eq!(signals.take(), Some(UiWatchdogActivity::Inactive));
}
#[test]
fn overlapping_system_menu_and_move_facts_cannot_clear_each_other() {
    let mut signals = Signals::new();
    signals.surface(0, SurfaceFacts::observed(true, false, true));
    assert_eq!(signals.take(), Some(UiWatchdogActivity::VisibleActive));
    signals.suspend(0, Suspension::Menu(true));
    signals.suspend(0, Suspension::Move(true));
    signals.suspend(1, Suspension::Menu(true));
    signals.suspend(0, Suspension::System(true));
    assert_eq!(signals.take(), Some(UiWatchdogActivity::Suspended));
    for (slot, fact) in
        [(0, Suspension::Menu(false)), (0, Suspension::System(false)), (0, Suspension::Move(false))]
    {
        signals.suspend(slot, fact);
        assert!(signals.take().is_none());
    }
    signals.suspend(1, Suspension::Menu(false));
    assert_eq!(signals.take(), Some(UiWatchdogActivity::VisibleActive));
}
#[test]
fn pending_native_enter_exit_is_preserved_even_before_ui_drain() {
    let mut signals = Signals::new();
    signals.surface(0, SurfaceFacts::observed(true, false, true));
    signals.suspend(0, Suspension::Move(true));
    signals.suspend(0, Suspension::Move(false));
    assert_eq!(signals.take(), Some(UiWatchdogActivity::VisibleActive));
    assert_eq!(signals.take(), Some(UiWatchdogActivity::Suspended));
    assert_eq!(signals.take(), Some(UiWatchdogActivity::VisibleActive));
    assert!(signals.take().is_none());
}
#[test]
fn closing_discards_queued_activity_and_blocks_further_native_renewals() {
    let mut signals = Signals::new();
    signals.surface(0, SurfaceFacts::observed(true, false, true));
    signals.close();
    signals.suspend(0, Suspension::System(true));
    signals.surface(0, SurfaceFacts::observed(false, false, false));
    assert!(signals.take().is_none());
}
#[test]
fn bounded_history_exhaustion_retains_first_cause_and_existing_order() {
    let mut signals = Signals::new();
    for index in 0..18 {
        signals.surface(0, SurfaceFacts::observed(true, false, index % 2 == 0));
    }
    assert_eq!(signals.count, 16);
    assert!(
        signals
            .failure
            .as_ref()
            .is_some_and(|cause| cause.to_string().contains("history exhausted"))
    );
    for index in 0..16 {
        assert_eq!(
            signals.take(),
            Some(if index % 2 == 0 {
                UiWatchdogActivity::VisibleActive
            } else {
                UiWatchdogActivity::Inactive
            })
        );
    }
    assert!(signals.take().is_none());
}
