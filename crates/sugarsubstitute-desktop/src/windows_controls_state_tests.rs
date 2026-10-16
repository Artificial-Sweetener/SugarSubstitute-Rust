// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose fixed content snapshots with the actual native update validation and activation fence.
use super::*;
use crate::{
    ControlBounds,
    native_action::NativeAction,
    windows_controls::{PaintMode, action},
};
use std::cell::Cell;

/// Replace only external native visibility/enabling effects; explicit messages use retained action.
struct Native {
    live: Cell<bool>,
    visible: bool,
    enabled: bool,
    action: bool,
    fail_visibility: bool,
    effects: Vec<(&'static str, bool)>,
}
impl Native {
    fn new() -> Self {
        Self {
            live: Cell::new(true),
            visible: false,
            enabled: false,
            action: false,
            fail_visibility: false,
            effects: Vec::new(),
        }
    }
    fn message_dispatches(&self) -> bool {
        self.action
    }
}
impl VisibilityPort for Native {
    fn live(&self) -> Result<(), DesktopError> {
        if self.live.get() { Ok(()) } else { Err(DesktopError::message("native_peer", "retired")) }
    }
    fn action(&mut self, action: bool) -> Result<(), DesktopError> {
        if action {
            assert!(self.visible && self.enabled);
        }
        self.action = action;
        self.effects.push(("action", action));
        Ok(())
    }
    fn enabled(&mut self, enabled: bool) -> Result<(), DesktopError> {
        assert!(!self.message_dispatches());
        if enabled {
            assert!(self.visible);
        }
        self.enabled = enabled;
        self.effects.push(("enabled", enabled));
        Ok(())
    }
    fn visible(&mut self, visible: bool) -> Result<(), DesktopError> {
        assert!(!self.message_dispatches() && !self.enabled);
        if self.fail_visibility {
            return Err(DesktopError::message("native_visibility", "original visibility failure"));
        }
        self.visible = visible;
        self.effects.push(("visible", visible));
        Ok(())
    }
}

/// Same typed native content contract as startup: original IDs and phase-owned Retry availability.
fn snapshots(retry: bool) -> [AccessibleControl; 3] {
    std::array::from_fn(|index| AccessibleControl {
        id: index as u16 + 1,
        label: ["Output", "Cancel", "Retry"][index],
        bounds: ControlBounds { x: 1.0, y: 1.0, width: 10.0, height: 10.0 },
        close: index == 1,
        visible: index != 2 || retry,
    })
}

#[test]
fn control_snapshots_reproduce_original_variable_inventory_rejection() {
    let loading = snapshots(false);
    let ready = snapshots(true);
    let cause = validate_inventory(&loading[..2], &ready);
    assert!(cause.is_err());
    assert!(validate_inventory(&loading, &ready).is_ok());
    assert!(validate_inventory(&loading, &loading).is_ok());
    let mut changed = ready;
    changed[2].id = 4;
    assert!(validate_inventory(&loading, &changed).is_err());
    changed = ready;
    changed[2].close = true;
    assert!(validate_inventory(&loading, &changed).is_err());
}

#[test]
fn control_snapshots_keep_retry_hidden_disabled_and_callback_inert_until_settlement()
-> Result<(), DesktopError> {
    let loading = snapshots(false);
    let ready = snapshots(true);
    validate_inventory(&loading, &ready)?;
    assert!(action(PaintMode::PreparedPixels, loading[2]).is_none());
    assert!(matches!(action(PaintMode::PreparedPixels, ready[2]), Some(NativeAction::Activate(3))));
    let style = crate::windows_controls::retry::initial_style(PaintMode::PreparedPixels, 3, false);
    assert_eq!(style & crate::windows_controls::WS_VISIBLE, Default::default());
    assert_eq!(style & crate::windows_controls::WS_DISABLED, crate::windows_controls::WS_DISABLED);
    let mut native = Native::new();
    apply_visibility(&mut native, loading[2].visible)?;
    assert!(!native.visible && !native.enabled && !native.message_dispatches());
    native.effects.clear();
    apply_visibility(&mut native, ready[2].visible)?;
    assert_eq!(
        native.effects,
        [
            ("action", false),
            ("enabled", false),
            ("visible", true),
            ("enabled", true),
            ("action", true)
        ]
    );
    assert!(native.visible && native.enabled && native.message_dispatches());
    native.effects.clear();
    apply_visibility(&mut native, loading[2].visible)?;
    assert_eq!(native.effects, [("action", false), ("enabled", false), ("visible", false)]);
    assert!(!native.visible && !native.enabled && !native.message_dispatches());
    Ok(())
}

#[test]
fn control_snapshots_visibility_failure_preserves_cause_and_fenced_callback() {
    let mut native = Native::new();
    native.fail_visibility = true;
    let cause = apply_visibility(&mut native, true).err();
    assert!(cause.is_some_and(|cause| cause.to_string().contains("native_visibility")));
    assert!(!native.enabled && !native.message_dispatches());
    assert_eq!(native.effects, [("action", false), ("enabled", false)]);
}
