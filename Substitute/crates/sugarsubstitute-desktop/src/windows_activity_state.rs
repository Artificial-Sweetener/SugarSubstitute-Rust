// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain bounded native eligibility history and overlapping acknowledged suspension facts.

use crate::DesktopError;
use sugarsubstitute_contracts::lifecycle::UiWatchdogActivity;

#[derive(Clone, Copy, Default)]
pub(super) struct SurfaceFacts {
    pub(super) visible: bool,
    pub(super) minimized: bool,
    pub(super) active: bool,
    menu: bool,
    moving: bool,
}
impl SurfaceFacts {
    pub(super) fn observed(visible: bool, minimized: bool, active: bool) -> Self {
        Self { visible, minimized, active, ..Self::default() }
    }
}
#[derive(Clone, Copy)]
pub(super) enum Suspension {
    System(bool),
    Menu(bool),
    Move(bool),
}
/// Fixed two-surface inventory and sixteen transitions; overload is an observed owner failure.
pub(super) struct Signals {
    surfaces: [SurfaceFacts; 2],
    sleeping: bool,
    last: Option<UiWatchdogActivity>,
    pending: [Option<UiWatchdogActivity>; 16],
    read: usize,
    count: usize,
    closing: bool,
    pub(super) failure: Option<DesktopError>,
}
impl Signals {
    pub(super) const fn new() -> Self {
        Self {
            surfaces: [SurfaceFacts {
                visible: false,
                minimized: false,
                active: false,
                menu: false,
                moving: false,
            }; 2],
            sleeping: false,
            last: None,
            pending: [None; 16],
            read: 0,
            count: 0,
            closing: false,
            failure: None,
        }
    }
    pub(super) fn surface(&mut self, slot: usize, facts: SurfaceFacts) {
        let current = &mut self.surfaces[slot];
        current.visible = facts.visible;
        current.minimized = facts.minimized;
        current.active = facts.active;
        self.record();
    }
    pub(super) fn suspend(&mut self, slot: usize, reason: Suspension) {
        match reason {
            Suspension::System(value) => self.sleeping = value,
            Suspension::Menu(value) => self.surfaces[slot].menu = value,
            Suspension::Move(value) => self.surfaces[slot].moving = value,
        }
        self.record();
    }
    pub(super) fn remove(&mut self, slot: usize) {
        self.surfaces[slot] = SurfaceFacts::default();
        self.record();
    }
    fn record(&mut self) {
        if self.closing {
            return;
        }
        let activity =
            if self.sleeping || self.surfaces.iter().any(|facts| facts.menu || facts.moving) {
                UiWatchdogActivity::Suspended
            } else if self
                .surfaces
                .iter()
                .any(|facts| facts.visible && !facts.minimized && facts.active)
            {
                UiWatchdogActivity::VisibleActive
            } else {
                UiWatchdogActivity::Inactive
            };
        if self.last == Some(activity) {
            return;
        }
        if self.count == self.pending.len() {
            DesktopError::retain_first(
                &mut self.failure,
                DesktopError::message(
                    "native_activity_history",
                    "bounded native activity history exhausted",
                ),
            );
            return;
        }
        self.last = Some(activity);
        let write = (self.read + self.count) % self.pending.len();
        self.pending[write] = Some(activity);
        self.count += 1;
    }
    pub(super) fn take(&mut self) -> Option<UiWatchdogActivity> {
        if self.count == 0 {
            return None;
        }
        let value = self.pending[self.read].take();
        self.read = (self.read + 1) % self.pending.len();
        self.count -= 1;
        value
    }
    pub(super) fn has_pending(&self) -> bool {
        self.count != 0 || self.failure.is_some()
    }
    pub(super) fn close(&mut self) {
        self.closing = true;
        self.pending.fill(None);
        self.count = 0;
    }
}

#[cfg(test)]
#[path = "windows_activity_state_tests.rs"]
mod tests;
