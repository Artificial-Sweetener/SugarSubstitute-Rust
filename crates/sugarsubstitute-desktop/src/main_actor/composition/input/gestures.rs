// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hand routed canvas gestures to the main loop.
//!
//! Keyboard messages belong to the browser's focus window, so the host never sees Shift, Ctrl
//! or Alt change. Each drain therefore samples the live modifiers first and lets routing
//! re-deliver a held gesture's latest sample when they changed. A press the canvas captured
//! never gives the page a focus change of its own, so it hands the page's focus to the body.

use super::{modifiers, *};

impl State {
    /// Take the oldest gesture routed to the canvas after re-delivering a held gesture's sample
    /// with changed modifiers; a captured press first moves the page's focus to its body.
    pub(super) fn take_gesture(&self) -> Option<MainCanvasGesture> {
        let gesture = {
            let mut routing = self.routing.borrow_mut();
            routing.refresh_modifiers(modifiers::live());
            routing.take()
        }?;
        if matches!(gesture, MainCanvasGesture::Button { pressed: true, .. }) {
            self.focus.focus_page();
        }
        Some(gesture)
    }
}
