// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decide, per composed-window mouse message, whether WebView2, native navigation or both see it.
//!
//! The canvas owner publishes the native input region: the shown final's clip minus the
//! page's occlusions. WebView2 receives every move and button message, so focus, hover and
//! context menus keep working; only wheel input routed to the canvas is withheld from it.
//! A primary or middle press inside the region starts a native gesture that stays captured
//! until its release or a cancellation. A gesture that starts elsewhere stays with WebView2.
//! Without a region every message belongs to WebView2 alone.
//!
//! This owner performs no native calls; the Win32 adapter samples message facts for it and
//! drains the bounded gesture queue on the main loop without waiting on the UI thread.

use crate::canvas::{
    MAX_INPUT_OCCLUSIONS, MainCanvasButton, MainCanvasGesture,
    click::{Click, double_click},
    placement::PhysicalRect,
};
use sugarsubstitute_canvas::CursorIntent;

/// Routed gestures retained between UI-thread messages and the next main-loop drain.
pub(super) const QUEUE_CAPACITY: usize = 64;

/// Physical Win32 hardware units per wheel notch (`WHEEL_DELTA`).
const WHEEL_DELTA: f64 = 120.0;

/// The mouse button named by a Win32 button message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Button {
    /// `WM_LBUTTON*`.
    Primary,
    /// `WM_MBUTTON*`.
    Middle,
    /// `WM_RBUTTON*`; always WebView2's, including its context menu.
    Secondary,
    /// `WM_XBUTTON*`; always WebView2's, including browser navigation buttons.
    Extra,
}

impl Button {
    const fn navigation(self) -> Option<MainCanvasButton> {
        match self {
            Self::Primary => Some(MainCanvasButton::Primary),
            Self::Middle => Some(MainCanvasButton::Middle),
            Self::Secondary | Self::Extra => None,
        }
    }
}

/// The user's Windows double-click rule, sampled with the press it applies to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ClickRule {
    /// `GetMessageTime` of the press.
    pub(super) time: u32,
    /// `SM_CXDOUBLECLK` and `SM_CYDOUBLECLK` for the window's current DPI.
    pub(super) bounds: [i32; 2],
    /// `GetDoubleClickTime` in milliseconds.
    pub(super) limit: u32,
}

/// One decoded mouse message in physical client pixels; `held` reports whether any mouse
/// button remains down after it, from the message's own key state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Message {
    /// `WM_MOUSEMOVE`.
    Move { point: [i32; 2], held: bool },
    /// A button-down or button-double-click message.
    Press { button: Button, point: [i32; 2], rule: ClickRule },
    /// A button-up message.
    Release { button: Button, point: [i32; 2], held: bool },
    /// `WM_MOUSEWHEEL`, already converted from screen to client coordinates.
    Wheel { point: [i32; 2], delta: i16 },
    /// `WM_MOUSELEAVE`.
    Leave,
    /// Horizontal wheel or any other message WebView2 alone interprets.
    Other,
}

/// Route state confined to the composed window's UI thread.
pub(super) struct Routing {
    clip: Option<PhysicalRect>,
    occlusions: Vec<PhysicalRect>,
    native: Option<MainCanvasButton>,
    web: bool,
    hover: bool,
    last: Option<[i32; 2]>,
    click: Option<Click>,
    queue: [Option<MainCanvasGesture>; QUEUE_CAPACITY],
    first: usize,
    count: usize,
}

impl Routing {
    /// Preallocate the occlusion table so publication and routing never allocate.
    ///
    /// # Errors
    /// Reports allocation failure for the bounded occlusion table.
    pub(super) fn new() -> Result<Self, crate::DesktopError> {
        let mut occlusions = Vec::new();
        occlusions
            .try_reserve_exact(MAX_INPUT_OCCLUSIONS)
            .map_err(|cause| crate::DesktopError::new("allocate_canvas_input_region", cause))?;
        Ok(Self {
            clip: None,
            occlusions,
            native: None,
            web: false,
            hover: false,
            last: None,
            click: None,
            queue: [None; QUEUE_CAPACITY],
            first: 0,
            count: 0,
        })
    }

    /// Install the current region, or withdraw it with `None`.
    ///
    /// A replaced region keeps a captured native gesture; withdrawal cancels it. A region with
    /// more than [`MAX_INPUT_OCCLUSIONS`] occlusions is invalid and withdraws routing.
    pub(super) fn publish(&mut self, region: Option<(PhysicalRect, &[PhysicalRect])>) {
        match region {
            Some((clip, occlusions)) if occlusions.len() <= MAX_INPUT_OCCLUSIONS => {
                if self.clip != Some(clip) || self.occlusions != occlusions {
                    self.clip = Some(clip);
                    self.occlusions.clear();
                    self.occlusions.extend_from_slice(occlusions);
                }
            }
            _ => {
                if self.clip.take().is_some() {
                    self.occlusions.clear();
                }
                self.cancel();
            }
        }
        self.hover = self.native.is_some()
            || (!self.web && self.last.is_some_and(|point| self.inside(point)));
    }

    /// Route one message, queueing native gestures; returns whether WebView2 receives it.
    pub(super) fn route(&mut self, message: Message) -> bool {
        match message {
            Message::Move { point, held } => {
                self.last = Some(point);
                if !held {
                    self.web = false;
                }
                self.hover = self.native.is_some() || (!self.web && self.inside(point));
                if self.hover {
                    self.push(MainCanvasGesture::Pointer(position(point)));
                }
                true
            }
            Message::Press { button, point, rule } => {
                self.last = Some(point);
                match (self.native, button.navigation()) {
                    (None, Some(routed)) if !self.web && self.inside(point) => {
                        self.press(routed, point, rule);
                    }
                    (None, _) => {
                        self.web = true;
                        self.hover = false;
                        self.click = None;
                    }
                    (Some(_), _) => {}
                }
                true
            }
            Message::Release { button, point, held } => {
                self.last = Some(point);
                if let Some(routed) =
                    self.native.filter(|routed| Some(*routed) == button.navigation())
                {
                    self.push(MainCanvasGesture::Pointer(position(point)));
                    self.push(MainCanvasGesture::Button { button: routed, pressed: false });
                    self.native = None;
                }
                if !held {
                    self.web = false;
                }
                self.hover = self.native.is_some() || (!self.web && self.inside(point));
                true
            }
            Message::Wheel { point, delta } => {
                if self.native.is_none() && (self.web || !self.inside(point)) {
                    return true;
                }
                self.push(MainCanvasGesture::Pointer(position(point)));
                self.push(MainCanvasGesture::Wheel(f64::from(delta) / WHEEL_DELTA));
                false
            }
            Message::Leave => {
                if self.native.is_none() {
                    self.hover = false;
                    self.last = None;
                }
                true
            }
            Message::Other => true,
        }
    }

    /// End a captured native gesture after capture, activation or region loss.
    pub(super) fn cancel(&mut self) {
        self.click = None;
        if self.native.take().is_some() {
            self.push(MainCanvasGesture::Cancel);
        }
    }

    /// Native cursor that wins over the browser's: closed hand while a native gesture holds
    /// capture, open hand over the region, otherwise none so WebView2's cursor applies.
    pub(super) const fn cursor(&self) -> Option<CursorIntent> {
        if self.native.is_some() {
            Some(CursorIntent::ClosedHand)
        } else if self.hover {
            Some(CursorIntent::OpenHand)
        } else {
            None
        }
    }

    /// Take the oldest routed gesture for delivery to the canvas owner.
    pub(super) fn take(&mut self) -> Option<MainCanvasGesture> {
        if self.count == 0 {
            return None;
        }
        let gesture = self.queue[self.first].take();
        self.first = (self.first + 1) % QUEUE_CAPACITY;
        self.count -= 1;
        gesture
    }

    /// Number of gestures awaiting the main-loop drain.
    pub(super) const fn pending(&self) -> usize {
        self.count
    }

    fn press(&mut self, routed: MainCanvasButton, point: [i32; 2], rule: ClickRule) {
        self.push(MainCanvasGesture::Pointer(position(point)));
        let current = Click { time: rule.time, point };
        let doubled = routed == MainCanvasButton::Primary
            && self
                .click
                .is_some_and(|previous| double_click(previous, current, rule.bounds, rule.limit));
        self.click = (routed == MainCanvasButton::Primary && !doubled).then_some(current);
        if doubled {
            self.push(MainCanvasGesture::DoubleClick(position(point)));
        }
        self.push(MainCanvasGesture::Button { button: routed, pressed: true });
        self.native = Some(routed);
        self.hover = true;
    }

    fn inside(&self, [x, y]: [i32; 2]) -> bool {
        self.clip.is_some_and(|clip| contains(clip, x, y))
            && !self.occlusions.iter().any(|occlusion| contains(*occlusion, x, y))
    }

    /// Coalesce consecutive moves; on overflow, replace the backlog with one cancellation.
    fn push(&mut self, gesture: MainCanvasGesture) {
        if let MainCanvasGesture::Pointer(_) = gesture
            && self.count != 0
        {
            let last = (self.first + self.count - 1) % QUEUE_CAPACITY;
            if let Some(MainCanvasGesture::Pointer(_)) = self.queue[last] {
                self.queue[last] = Some(gesture);
                return;
            }
        }
        if self.count == QUEUE_CAPACITY {
            self.queue = [None; QUEUE_CAPACITY];
            self.first = 0;
            self.count = 0;
            self.native = None;
            self.click = None;
            self.queue[0] = Some(MainCanvasGesture::Cancel);
            self.count = 1;
            return;
        }
        self.queue[(self.first + self.count) % QUEUE_CAPACITY] = Some(gesture);
        self.count += 1;
    }
}

fn contains(rect: PhysicalRect, x: i32, y: i32) -> bool {
    let (x, y) = (i64::from(x), i64::from(y));
    x >= i64::from(rect.x)
        && y >= i64::from(rect.y)
        && x < i64::from(rect.x) + i64::from(rect.width)
        && y < i64::from(rect.y) + i64::from(rect.height)
}

fn position(point: [i32; 2]) -> [f64; 2] {
    point.map(f64::from)
}

#[cfg(test)]
#[path = "routing/tests.rs"]
mod tests;
