// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve stable control identity and native callback eligibility for shared availability.
use super::*;
use crate::{
    ControlBounds,
    native_action::NativeAction,
    windows_controls::{PaintMode, action},
};

/// Same typed native contract as startup; hidden Retry keeps its original identity.
fn snapshots(retry: bool) -> [AccessibleControl; 3] {
    std::array::from_fn(|index| AccessibleControl {
        id: index as u16 + 1,
        label: ["Output", "Cancel", "Retry"][index],
        bounds: ControlBounds { x: 1.0, y: 1.0, width: 10.0, height: 10.0 },
        close: index == 1,
        visible: index != 2 || retry,
        enabled: true,
    })
}

#[test]
fn control_snapshots_reject_inventory_changes_but_accept_availability() {
    let loading = snapshots(false);
    let ready = snapshots(true);
    assert!(validate_inventory(&loading[..2], &ready).is_err());
    assert!(validate_inventory(&loading, &ready).is_ok());
    let mut changed = ready;
    changed[2].id = 4;
    assert!(validate_inventory(&loading, &changed).is_err());
    changed = ready;
    changed[2].close = true;
    assert!(validate_inventory(&loading, &changed).is_err());
    changed = ready;
    changed[2].enabled = false;
    assert!(validate_inventory(&loading, &changed).is_ok());
}

#[test]
fn hidden_or_busy_controls_cannot_dispatch_explicit_native_activation() {
    let loading = snapshots(false);
    let mut ready = snapshots(true);
    assert!(action(PaintMode::PreparedPixels, loading[2]).is_none());
    assert!(matches!(action(PaintMode::PreparedPixels, ready[2]), Some(NativeAction::Activate(3))));
    ready[2].enabled = false;
    assert!(action(PaintMode::PreparedPixels, ready[2]).is_none());
    ready[1].enabled = false;
    assert!(action(PaintMode::PreparedPixels, ready[1]).is_none());
}
