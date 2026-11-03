// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route Windows pen pointer messages to the canvas and forward the rest to WebView2.
//!
//! Windows delivers pen input as `WM_POINTER*` messages and promotes them to mouse messages
//! only when they reach the default window procedure. The composed window consumes every pen
//! message it decodes: routing claims samples over the canvas region, and the rest reach
//! WebView2 through `SendPointerInput` with the original pointer facts, so the page sees a real
//! pen. No decoded pen message reaches the default procedure, so Windows promotes none of them;
//! a promoted pen mouse message that still arrives is recognized by its message signature and
//! dropped, so no pen observation is delivered twice. Touch keeps its existing path.
//!
//! Pointer facts are copied on the UI thread while Windows still holds the message's frame;
//! nothing here retains a pointer identity beyond the message.

use super::State;
use crate::DesktopError;
use crate::main_actor::composition::{
    modifiers,
    routing::{Button, Message, Routing},
};
use std::cell::Cell;
use sugarsubstitute_canvas::{PointerKind, PointerSample, Pressure, ToolModifiers};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_POINTER_EVENT_KIND, ICoreWebView2_2, ICoreWebView2CompositionController,
    ICoreWebView2Controller, ICoreWebView2Environment3,
};
use windows::{
    Win32::{
        Foundation::{POINT, RECT, WPARAM},
        Graphics::Gdi::ClientToScreen,
        UI::{
            Input::Pointer::{
                GetPointerDeviceRects, GetPointerPenInfo, GetPointerType,
                POINTER_BUTTON_CHANGE_TYPE, POINTER_CHANGE_FIFTHBUTTON_DOWN,
                POINTER_CHANGE_FIFTHBUTTON_UP, POINTER_CHANGE_FIRSTBUTTON_DOWN,
                POINTER_CHANGE_FIRSTBUTTON_UP, POINTER_CHANGE_FOURTHBUTTON_DOWN,
                POINTER_CHANGE_FOURTHBUTTON_UP, POINTER_CHANGE_SECONDBUTTON_DOWN,
                POINTER_CHANGE_SECONDBUTTON_UP, POINTER_CHANGE_THIRDBUTTON_DOWN,
                POINTER_CHANGE_THIRDBUTTON_UP, POINTER_FLAG_INCONTACT, POINTER_FLAG_SECONDBUTTON,
                POINTER_FLAGS, POINTER_PEN_INFO,
            },
            WindowsAndMessaging::{
                GetMessageExtraInfo, PEN_FLAG_ERASER, PEN_FLAG_INVERTED, PEN_MASK_PRESSURE,
                POINTER_INPUT_TYPE, PT_PEN, WM_POINTERCAPTURECHANGED, WM_POINTERDOWN,
                WM_POINTERENTER, WM_POINTERLEAVE, WM_POINTERUP, WM_POINTERUPDATE,
            },
        },
    },
    core::Interface,
};

/// `POINTER_PEN_INFO.pressure` is normalized by Windows to `0..=1024`.
const PEN_PRESSURE_RANGE: u32 = 1024;
/// `MI_WP_SIGNATURE`: the `GetMessageExtraInfo` stamp of mouse messages promoted from pen or
/// touch input; touch additionally sets bit 7.
const PROMOTED_SIGNATURE: u32 = 0xff51_5700;
const PROMOTED_PEN_MASK: u32 = 0xffff_ff80;

/// What one pen message means for routing, before its sample is attached.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Transition {
    /// Hover in range, or movement while in contact (`held`).
    Move {
        /// Whether the tip or eraser touches the surface.
        held: bool,
    },
    /// The tip or eraser touched the surface, or a barrel button contacted it.
    Press(Button),
    /// The contact ended.
    Release(Button),
    /// The pen left detection range.
    Leave,
    /// The pen entered detection range; only WebView2 interprets it.
    Enter,
}

/// Pen forwarding owner for one composed window: the browser environment that creates pointer
/// descriptions and whether WebView2 currently tracks the pen in range.
pub(super) struct Pen {
    environment: Option<ICoreWebView2Environment3>,
    page: Cell<bool>,
}

impl Pen {
    /// Take pen messages only when the browser can receive forwarded pointer input; otherwise
    /// they keep the platform's default processing.
    pub(super) fn new(browser: &ICoreWebView2Controller) -> Self {
        // SAFETY: The controller belongs to this UI apartment; the queries retain only owned
        // COM references.
        let environment = unsafe { browser.CoreWebView2() }
            .and_then(|view| view.cast::<ICoreWebView2_2>())
            // SAFETY: As above, on the same apartment.
            .and_then(|view| unsafe { view.Environment() })
            .and_then(|environment| environment.cast::<ICoreWebView2Environment3>());
        let environment = environment
            .inspect_err(|cause| {
                tracing::warn!(operation = "composed_browser_pen_forwarding", cause = %cause,
                    "Pen input keeps default processing; the browser cannot receive pointer input");
            })
            .ok();
        Self { environment, page: Cell::new(false) }
    }
}

impl State {
    /// Copy the pen facts of a pointer message this window routes itself, or `None` for other
    /// messages, other devices, or a browser without pointer forwarding.
    pub(super) fn pen_message(&self, message: u32, wparam: WPARAM) -> Option<POINTER_PEN_INFO> {
        self.pen.environment.as_ref()?;
        read(message, wparam)
    }

    /// Route one pen message, then forward what WebView2 keeps with matching range transitions.
    /// A pen contact first moves keyboard focus into a browser that lacks it.
    pub(super) fn deliver_pen(
        &self,
        message: u32,
        pen: &POINTER_PEN_INFO,
    ) -> Result<(), DesktopError> {
        let Some(transition) = transition(message, pen) else {
            self.update_routing(Routing::cancel);
            return self.present_cursor();
        };
        let origin = self.client_origin()?;
        let sample =
            sample(pen, client(pen.pointerInfo.ptPixelLocation, origin), modifiers::current());
        let routed = match transition {
            Transition::Move { held } => Message::Move { sample, held },
            Transition::Press(button) => {
                if !self.focus.browser_focused() {
                    self.focus.enter();
                }
                self.press(button, sample)
            }
            Transition::Release(button) => Message::Release { button, sample, held: false },
            Transition::Leave => Message::Leave,
            Transition::Enter => Message::Other,
        };
        let forward = self.update_routing(|routing| routing.route_pen(routed));
        self.present_cursor()?;
        let (events, tracked) = page_events(message, forward, self.pen.page.get());
        self.pen.page.set(tracked);
        if events.iter().all(Option::is_none) {
            return Ok(());
        }
        let page = page_pointer(*pen, origin, self.device_rects(pen)?);
        for kind in events.into_iter().flatten() {
            self.send_pen(kind, &page)?;
        }
        Ok(())
    }

    /// Whether the mouse message being processed was promoted from a pen this window already
    /// routes through its pointer messages.
    pub(super) fn promoted_pen(&self) -> bool {
        // SAFETY: Reads this UI thread's extra information for the message being processed.
        self.pen.environment.is_some() && promoted_from_pen(unsafe { GetMessageExtraInfo() }.0)
    }

    fn client_origin(&self) -> Result<POINT, DesktopError> {
        let mut origin = POINT::default();
        // SAFETY: The original live HWND and local POINT belong to this UI-thread callback.
        if !unsafe { ClientToScreen(self.window, &raw mut origin) }.as_bool() {
            return Err(DesktopError::message(
                "composed_browser_pen_coordinates",
                "Native client origin is unavailable",
            ));
        }
        Ok(origin)
    }

    /// The pen device's himetric extent and the screen rectangle it maps to.
    fn device_rects(&self, pen: &POINTER_PEN_INFO) -> Result<[RECT; 2], DesktopError> {
        let (mut device, mut display) = (RECT::default(), RECT::default());
        // SAFETY: The source device belongs to the live message; both rectangles are local.
        unsafe {
            GetPointerDeviceRects(pen.pointerInfo.sourceDevice, &raw mut device, &raw mut display)
        }
        .map_err(|cause| DesktopError::new("query_composed_browser_pen_device", cause))?;
        Ok([device, display])
    }

    fn send_pen(&self, kind: u32, page: &PagePointer) -> Result<(), DesktopError> {
        let environment = self.pen.environment.as_ref().ok_or_else(|| {
            DesktopError::message("send_composed_browser_pen", "Pointer forwarding is unavailable")
        })?;
        forward(&self.controller, environment, kind, page)
            .map_err(|cause| DesktopError::new("send_composed_browser_pen", cause))
    }
}

/// Describe one pen frame to WebView2 and deliver it as `kind`, a `WM_POINTER*` message value;
/// WebView2's pointer event kinds share those values.
fn forward(
    controller: &ICoreWebView2CompositionController,
    environment: &ICoreWebView2Environment3,
    kind: u32,
    page: &PagePointer,
) -> windows::core::Result<()> {
    let pen = &page.pen;
    let pointer = &pen.pointerInfo;
    // SAFETY: The controller and environment are retained on their creating UI apartment; the
    // created description is owned locally and every setter copies scalar values.
    unsafe {
        let info = environment.CreateCoreWebView2PointerInfo()?;
        info.SetPointerKind(PT_PEN.0 as u32)?;
        info.SetPointerId(pointer.pointerId)?;
        info.SetFrameId(pointer.frameId)?;
        info.SetPointerFlags(pointer.pointerFlags.0)?;
        info.SetPointerDeviceRect(page.device)?;
        info.SetDisplayRect(page.display)?;
        info.SetPixelLocation(pointer.ptPixelLocation)?;
        info.SetHimetricLocation(pointer.ptHimetricLocation)?;
        info.SetPixelLocationRaw(pointer.ptPixelLocationRaw)?;
        info.SetHimetricLocationRaw(pointer.ptHimetricLocationRaw)?;
        info.SetTime(pointer.dwTime)?;
        info.SetHistoryCount(pointer.historyCount)?;
        info.SetInputData(pointer.InputData)?;
        info.SetKeyStates(pointer.dwKeyStates)?;
        info.SetPerformanceCount(pointer.PerformanceCount)?;
        info.SetButtonChangeKind(pointer.ButtonChangeType.0)?;
        info.SetPenFlags(pen.penFlags)?;
        info.SetPenMask(pen.penMask)?;
        info.SetPenPressure(pen.pressure)?;
        info.SetPenRotation(pen.rotation)?;
        info.SetPenTiltX(pen.tiltX)?;
        info.SetPenTiltY(pen.tiltY)?;
        controller.SendPointerInput(COREWEBVIEW2_POINTER_EVENT_KIND(kind as i32), &info)
    }
}

/// Copy the pen frame of a pointer message on the receiving UI thread, or `None` for other
/// messages and devices. Only valid inside the window procedure that received the message.
fn read(message: u32, wparam: WPARAM) -> Option<POINTER_PEN_INFO> {
    if !matches!(
        message,
        WM_POINTERUPDATE
            | WM_POINTERDOWN
            | WM_POINTERUP
            | WM_POINTERENTER
            | WM_POINTERLEAVE
            | WM_POINTERCAPTURECHANGED
    ) {
        return None;
    }
    // `GET_POINTERID_WPARAM`.
    let id = u32::from(wparam.0 as u16);
    let frame = || -> windows::core::Result<Option<POINTER_PEN_INFO>> {
        let mut kind = POINTER_INPUT_TYPE::default();
        // SAFETY: Reads the live message's pointer type into local storage.
        unsafe { GetPointerType(id, &raw mut kind) }?;
        if kind != PT_PEN {
            return Ok(None);
        }
        let mut pen = POINTER_PEN_INFO::default();
        // SAFETY: Copies the pen frame Windows holds for this message into local storage.
        unsafe { GetPointerPenInfo(id, &raw mut pen) }?;
        Ok(Some(pen))
    };
    frame()
        .inspect_err(|cause| {
            tracing::debug!(operation = "composed_browser_pen", cause = %cause,
                "Pointer frame is unavailable; the message keeps default processing");
        })
        .ok()
        .flatten()
}

/// Whether a decoded pen message is consumed; a capture change keeps default processing after
/// it ends any routed contact.
pub(super) const fn consumed(message: u32) -> bool {
    message != WM_POINTERCAPTURECHANGED
}

/// Whether a mouse message's `GetMessageExtraInfo` marks it as promoted from pen input.
pub(super) const fn promoted_from_pen(extra: isize) -> bool {
    (extra as u32) & PROMOTED_PEN_MASK == PROMOTED_SIGNATURE
}

/// Classify a pen message; `None` is a pointer capture change, which ends any routed contact.
pub(super) fn transition(message: u32, pen: &POINTER_PEN_INFO) -> Option<Transition> {
    let pointer = &pen.pointerInfo;
    match message {
        WM_POINTERUPDATE => {
            Some(Transition::Move { held: pointer.pointerFlags.contains(POINTER_FLAG_INCONTACT) })
        }
        WM_POINTERDOWN => {
            Some(Transition::Press(button(pointer.ButtonChangeType, pointer.pointerFlags)))
        }
        WM_POINTERUP => {
            Some(Transition::Release(button(pointer.ButtonChangeType, pointer.pointerFlags)))
        }
        WM_POINTERLEAVE => Some(Transition::Leave),
        WM_POINTERENTER => Some(Transition::Enter),
        _ => None,
    }
}

/// The tip and eraser are the primary button; the barrel button is secondary, so it keeps the
/// page's context menu exactly as a right click does.
fn button(change: POINTER_BUTTON_CHANGE_TYPE, flags: POINTER_FLAGS) -> Button {
    match change {
        POINTER_CHANGE_FIRSTBUTTON_DOWN | POINTER_CHANGE_FIRSTBUTTON_UP => Button::Primary,
        POINTER_CHANGE_SECONDBUTTON_DOWN | POINTER_CHANGE_SECONDBUTTON_UP => Button::Secondary,
        POINTER_CHANGE_THIRDBUTTON_DOWN
        | POINTER_CHANGE_THIRDBUTTON_UP
        | POINTER_CHANGE_FOURTHBUTTON_DOWN
        | POINTER_CHANGE_FOURTHBUTTON_UP
        | POINTER_CHANGE_FIFTHBUTTON_DOWN
        | POINTER_CHANGE_FIFTHBUTTON_UP => Button::Extra,
        _ if flags.contains(POINTER_FLAG_SECONDBUTTON) => Button::Secondary,
        _ => Button::Primary,
    }
}

/// Translate a screen pixel into client pixels whose screen origin is `origin`.
pub(super) const fn client(point: POINT, origin: POINT) -> POINT {
    POINT { x: point.x - origin.x, y: point.y - origin.y }
}

/// Convert one pen frame into a tool sample at a client pixel. Pressure is normalized from the
/// Windows pen range when the device reports it and is full otherwise; the eraser end and an
/// inverted pen both count as erasing, while the barrel button does not.
pub(super) fn sample(
    pen: &POINTER_PEN_INFO,
    client: POINT,
    modifiers: ToolModifiers,
) -> PointerSample {
    let pressure = if pen.penMask & PEN_MASK_PRESSURE != 0 {
        Pressure::from_range(pen.pressure, PEN_PRESSURE_RANGE)
    } else {
        Pressure::FULL
    };
    PointerSample {
        position: [f64::from(client.x), f64::from(client.y)],
        kind: PointerKind::Pen,
        pressure,
        eraser: pen.penFlags & (PEN_FLAG_ERASER | PEN_FLAG_INVERTED) != 0,
        modifiers,
    }
}

/// Decide which pointer events WebView2 receives for one routed pen message and whether it
/// tracks the pen afterwards. A pen the canvas claims leaves the page; a pen the page regains
/// enters it again before the forwarded message, so page hover never sticks.
pub(super) fn page_events(message: u32, forward: bool, tracked: bool) -> ([Option<u32>; 2], bool) {
    match (forward, message) {
        (true, WM_POINTERLEAVE) => ([tracked.then_some(WM_POINTERLEAVE), None], false),
        (true, WM_POINTERENTER) => ([(!tracked).then_some(WM_POINTERENTER), None], true),
        (true, _) => ([(!tracked).then_some(WM_POINTERENTER), Some(message)], true),
        (false, _) => ([tracked.then_some(WM_POINTERLEAVE), None], false),
    }
}

/// One pen frame in WebView2 coordinates: the client area's physical pixels. The display
/// rectangle moves with the pixel locations, so the unchanged himetric locations and device
/// rectangle keep mapping onto the same pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PagePointer {
    pen: POINTER_PEN_INFO,
    device: RECT,
    display: RECT,
}

/// Translate a pen frame from screen to client coordinates for WebView2.
pub(super) fn page_pointer(
    mut pen: POINTER_PEN_INFO,
    origin: POINT,
    [device, display]: [RECT; 2],
) -> PagePointer {
    let pointer = &mut pen.pointerInfo;
    pointer.ptPixelLocation = client(pointer.ptPixelLocation, origin);
    pointer.ptPixelLocationRaw = client(pointer.ptPixelLocationRaw, origin);
    let display = RECT {
        left: display.left - origin.x,
        top: display.top - origin.y,
        right: display.right - origin.x,
        bottom: display.bottom - origin.y,
    };
    PagePointer { pen, device, display }
}

#[cfg(test)]
#[path = "pen/tests.rs"]
mod tests;
