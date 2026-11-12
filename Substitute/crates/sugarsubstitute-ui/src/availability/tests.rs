// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Replace only native effects; exercise the production admission fence through reentry/failure.
use super::*;

const STATES: [Availability; 4] = [
    Availability { visible: false, enabled: false },
    Availability { visible: false, enabled: true },
    Availability { visible: true, enabled: false },
    Availability { visible: true, enabled: true },
];

struct Peer {
    live: bool,
    visible: bool,
    enabled: bool,
    action: bool,
    fail_visible: bool,
    retire_on_enable: bool,
    effects: Vec<(&'static str, bool)>,
}
impl Peer {
    fn new(state: Availability) -> Self {
        Self {
            live: true,
            visible: state.visible,
            enabled: state.actionable(),
            action: state.actionable(),
            fail_visible: false,
            retire_on_enable: false,
            effects: Vec::new(),
        }
    }
}
impl AvailabilityPort for Peer {
    type Error = &'static str;
    fn live(&self) -> Result<(), Self::Error> {
        if self.live { Ok(()) } else { Err("original peer retired") }
    }
    fn action(&mut self, value: bool) -> Result<(), Self::Error> {
        if value {
            assert!(self.visible && self.enabled && self.live);
        }
        self.action = value;
        self.effects.push(("action", value));
        Ok(())
    }
    fn enabled(&mut self, value: bool) -> Result<(), Self::Error> {
        assert!(!self.action);
        if value {
            assert!(self.visible);
        }
        self.enabled = value;
        self.effects.push(("enabled", value));
        if value && self.retire_on_enable {
            self.live = false;
        }
        Ok(())
    }
    fn visible(&mut self, value: bool) -> Result<(), Self::Error> {
        assert!(!self.action && !self.enabled);
        if self.fail_visible {
            return Err("visibility failed");
        }
        self.visible = value;
        self.effects.push(("visible", value));
        Ok(())
    }
}

#[test]
fn all_availability_transitions_keep_hidden_and_disabled_peers_inert() -> Result<(), &'static str> {
    for previous in STATES {
        for current in STATES {
            let mut peer = Peer::new(previous);
            reconcile(previous, current, &mut peer)?;
            assert_eq!(peer.visible, current.visible);
            assert_eq!(peer.enabled, current.actionable());
            assert_eq!(peer.action, current.actionable());
            assert_eq!(peer.effects.is_empty(), previous == current);
        }
    }
    Ok(())
}

#[test]
fn visible_busy_control_is_disabled_before_any_reentrant_visibility_call()
-> Result<(), &'static str> {
    let mut peer = Peer::new(STATES[3]);
    reconcile(STATES[3], STATES[2], &mut peer)?;
    assert_eq!(peer.effects, [("action", false), ("enabled", false), ("visible", true)]);
    assert!(peer.visible && !peer.enabled && !peer.action);
    Ok(())
}

#[test]
fn enable_restores_callback_only_after_native_effects_verify() -> Result<(), &'static str> {
    let mut peer = Peer::new(STATES[2]);
    apply(STATES[3], &mut peer)?;
    assert_eq!(
        peer.effects,
        [
            ("action", false),
            ("enabled", false),
            ("visible", true),
            ("enabled", true),
            ("action", true)
        ]
    );
    Ok(())
}

#[test]
fn visibility_failure_retains_original_cause_and_fences_explicit_activation() {
    let mut peer = Peer::new(STATES[3]);
    peer.fail_visible = true;
    assert_eq!(apply(STATES[2], &mut peer), Err("visibility failed"));
    assert!(!peer.action && !peer.enabled);
    assert_eq!(peer.effects, [("action", false), ("enabled", false)]);
}

#[test]
fn native_retirement_during_enabling_cannot_restore_callback() {
    let mut peer = Peer::new(STATES[2]);
    peer.retire_on_enable = true;
    assert_eq!(apply(STATES[3], &mut peer), Err("original peer retired"));
    assert!(!peer.action);
    assert_eq!(peer.effects.last(), Some(&("enabled", true)));
}

#[test]
fn unchanged_snapshot_preserves_an_independently_owned_recovery_action() -> Result<(), &'static str>
{
    let mut peer = Peer::new(STATES[2]);
    reconcile(STATES[3], STATES[3], &mut peer)?;
    assert!(peer.effects.is_empty());
    assert!(!peer.enabled && !peer.action);
    Ok(())
}
