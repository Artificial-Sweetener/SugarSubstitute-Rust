// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Host the composed browser's native pointer, keyboard focus, cursor and drag-and-drop.
//!
//! Every mouse message is routed against the canvas owner's published input region before
//! WebView2 receives it; gestures assigned to native navigation wait in the routing queue for
//! the main loop, so the UI thread never waits on canvas work. Keyboard and IME messages are
//! never intercepted: they belong to the browser's own focus window.

use super::callbacks::{CallbackFence, retire_when_quiet};
use super::cancellation::{self, Interruption};
use super::cursor::{self, Hands, Pointer, Subscription};
use super::drag_source;
use super::drop_target::{DropTarget, Exporter};
use super::routing::{Button, ClickRule, Message, Routing};
use super::*;
use crate::canvas::{MainCanvasDragOut, MainCanvasGesture, placement::PhysicalRect};
use crate::main_actor::focus::{self, Focus};
use crate::windows_subclass::{self, Registration};
use std::cell::{Cell, RefCell};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_MOUSE_EVENT_KIND, COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS,
    COREWEBVIEW2_NON_CLIENT_REGION_KIND, COREWEBVIEW2_NON_CLIENT_REGION_KIND_CAPTION,
    ICoreWebView2CompositionController4,
};
use windows::Win32::{
    Foundation::{LPARAM, LRESULT, POINT, WPARAM},
    Graphics::Gdi::ScreenToClient,
    UI::{
        Controls::WM_MOUSELEAVE,
        HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi},
        Input::KeyboardAndMouse::{
            GetDoubleClickTime, GetKeyState, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
            VK_LBUTTON,
        },
        Shell::DefSubclassProc,
        WindowsAndMessaging::{
            GetMessageTime, HTCAPTION, HTCLIENT, PostMessageW, SM_CXDOUBLECLK, SM_CXDRAG,
            SM_CYDOUBLECLK, SM_CYDRAG, WM_APP, WM_DPICHANGED, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN,
            WM_LBUTTONUP, WM_MBUTTONDBLCLK, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL,
            WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCDESTROY, WM_NCHITTEST, WM_RBUTTONDBLCLK,
            WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SETCURSOR, WM_SETFOCUS, WM_XBUTTONDBLCLK,
            WM_XBUTTONDOWN, WM_XBUTTONUP,
        },
    },
};

/// WebView2 accepts the seven Win32 mouse-button, shift and control key bits.
const MOUSE_KEYS: u16 = 0x7f;
/// `MK_LBUTTON | MK_RBUTTON | MK_MBUTTON | MK_XBUTTON1 | MK_XBUTTON2`.
const BUTTON_KEYS: u16 = 0x73;
/// Private message that starts the queued drag-out; Tao defines no `WM_APP` messages.
const DRAG_OUT_MESSAGE: u32 = WM_APP + 1;

/// These values remain borrowed from the original native message until synchronous delivery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Mouse {
    kind: u32,
    keys: u16,
    data: u32,
    point: [i32; 2],
    screen: bool,
}

/// Decode only messages understood by WebView2; other window and caption messages stay with Tao.
fn mouse(message: u32, wparam: WPARAM, lparam: LPARAM) -> Option<Mouse> {
    if message == WM_MOUSELEAVE {
        return Some(Mouse { kind: message, keys: 0, data: 0, point: [0, 0], screen: false });
    }
    if !matches!(
        message,
        WM_MOUSEMOVE
            | WM_LBUTTONDOWN
            | WM_LBUTTONUP
            | WM_LBUTTONDBLCLK
            | WM_RBUTTONDOWN
            | WM_RBUTTONUP
            | WM_RBUTTONDBLCLK
            | WM_MBUTTONDOWN
            | WM_MBUTTONUP
            | WM_MBUTTONDBLCLK
            | WM_XBUTTONDOWN
            | WM_XBUTTONUP
            | WM_XBUTTONDBLCLK
            | WM_MOUSEWHEEL
            | WM_MOUSEHWHEEL
    ) {
        return None;
    }
    let keys = wparam.0 as u16;
    if keys & !MOUSE_KEYS != 0 {
        return None;
    }
    let screen = matches!(message, WM_MOUSEWHEEL | WM_MOUSEHWHEEL);
    let high = (wparam.0 >> 16) as u16;
    if matches!(message, WM_XBUTTONDOWN | WM_XBUTTONUP | WM_XBUTTONDBLCLK) && !matches!(high, 1 | 2)
    {
        return None;
    }
    let data = if screen {
        (high as i16 as i32) as u32
    } else if matches!(message, WM_XBUTTONDOWN | WM_XBUTTONUP | WM_XBUTTONDBLCLK) {
        u32::from(high)
    } else {
        0
    };
    Some(Mouse {
        kind: message,
        keys,
        data,
        point: [lparam.0 as u16 as i16 as i32, (lparam.0 >> 16) as u16 as i16 as i32],
        screen,
    })
}

/// Honor only the page's caption: the host draws its own window buttons and frame edges.
fn caption_hit(kind: COREWEBVIEW2_NON_CLIENT_REGION_KIND) -> Option<LRESULT> {
    (kind == COREWEBVIEW2_NON_CLIENT_REGION_KIND_CAPTION).then_some(LRESULT(HTCAPTION as isize))
}

struct State {
    window: HWND,
    controller: ICoreWebView2CompositionController,
    /// The page's `app-region` query, absent on a runtime without non-client region support.
    regions: Option<ICoreWebView2CompositionController4>,
    events: Rc<Events>,
    active: Cell<bool>,
    pointer: Pointer,
    routing: RefCell<Routing>,
    hands: Hands,
    focus: Focus,
    /// Buttons WebView2 last saw held, with the client point of that forwarded message.
    page: Cell<(u16, POINT)>,
    callbacks: CallbackFence,
    /// The drag-out posted for this window's next dispatch, with its export refusal.
    drag_out: RefCell<Option<(MainCanvasDragOut, Exporter)>>,
}

impl State {
    /// Deliver in physical client pixels on the original UI apartment, without retaining a borrow.
    /// A press first moves keyboard focus into a browser that lacks it.
    fn deliver(&self, message: Mouse) -> Result<(), DesktopError> {
        if focus::requests_focus(message.kind, self.focus.browser_focused()) {
            self.focus.enter();
        }
        if message.kind == WM_MOUSEMOVE && self.pointer.enter() {
            let mut tracking = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: self.window,
                dwHoverTime: 0,
            };
            // SAFETY: The callback owns this live HWND and the native call borrows local storage.
            if let Err(cause) = unsafe { TrackMouseEvent(&raw mut tracking) } {
                self.pointer.leave();
                return Err(DesktopError::new("track_composed_browser_mouse", cause));
            }
        } else if message.kind == WM_MOUSELEAVE {
            self.pointer.leave();
        }
        let mut point = POINT { x: message.point[0], y: message.point[1] };
        if message.screen {
            // SAFETY: The original live HWND and local POINT belong to this UI-thread callback.
            if !unsafe { ScreenToClient(self.window, &raw mut point) }.as_bool() {
                return Err(DesktopError::message(
                    "composed_browser_mouse_coordinates",
                    "Native wheel coordinates are unavailable",
                ));
            }
        }
        let forward = self.route(message, [point.x, point.y]);
        self.present_cursor()?;
        if !forward {
            return Ok(());
        }
        // SAFETY: The controller is retained on its creating UI apartment until subclass removal.
        unsafe {
            self.controller.SendMouseInput(
                COREWEBVIEW2_MOUSE_EVENT_KIND(message.kind as i32),
                COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS(i32::from(message.keys)),
                message.data,
                point,
            )
        }
        .map_err(|cause| DesktopError::new("send_composed_browser_mouse", cause))?;
        if message.kind != WM_MOUSELEAVE {
            self.page.set((message.keys & BUTTON_KEYS, point));
        }
        Ok(())
    }

    /// Route one message in client pixels; returns whether WebView2 also receives it.
    fn route(&self, message: Mouse, point: [i32; 2]) -> bool {
        let held = message.keys & BUTTON_KEYS != 0;
        let routed = match message.kind {
            WM_MOUSEMOVE => Message::Move { point, held },
            WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => self.press(Button::Primary, point),
            WM_MBUTTONDOWN | WM_MBUTTONDBLCLK => self.press(Button::Middle, point),
            WM_RBUTTONDOWN | WM_RBUTTONDBLCLK => self.press(Button::Secondary, point),
            WM_XBUTTONDOWN | WM_XBUTTONDBLCLK => self.press(Button::Extra, point),
            WM_LBUTTONUP => Message::Release { button: Button::Primary, point, held },
            WM_MBUTTONUP => Message::Release { button: Button::Middle, point, held },
            WM_RBUTTONUP => Message::Release { button: Button::Secondary, point, held },
            WM_XBUTTONUP => Message::Release { button: Button::Extra, point, held },
            // The decoder stores the signed wheel delta in the low 16 bits.
            WM_MOUSEWHEEL => Message::Wheel { point, delta: message.data as u16 as i16 },
            WM_MOUSELEAVE => Message::Leave,
            _ => Message::Other,
        };
        self.update_routing(|routing| routing.route(routed))
    }

    /// Answer a client-area hit test over the page's caption region with `HTCAPTION`, so
    /// Windows moves, snaps and maximizes the window. A composition-hosted WebView2 owns no
    /// HWND and cannot answer `WM_NCHITTEST` for its own `app-region: drag` elements.
    fn caption(&self, lparam: LPARAM) -> Result<Option<LRESULT>, DesktopError> {
        let Some(regions) = &self.regions else { return Ok(None) };
        let mut point =
            POINT { x: lparam.0 as u16 as i16 as i32, y: (lparam.0 >> 16) as u16 as i16 as i32 };
        // SAFETY: The original live HWND and local POINT belong to this UI-thread callback.
        if !unsafe { ScreenToClient(self.window, &raw mut point) }.as_bool() {
            return Err(DesktopError::message(
                "composed_browser_caption_coordinates",
                "Native hit-test coordinates are unavailable",
            ));
        }
        let mut kind = COREWEBVIEW2_NON_CLIENT_REGION_KIND::default();
        // SAFETY: The controller is retained on its creating UI apartment and the query writes
        // one scalar into live local storage.
        unsafe { regions.GetNonClientRegionAtPoint(point, &raw mut kind) }
            .map_err(|cause| DesktopError::new("query_composed_browser_caption", cause))?;
        Ok(caption_hit(kind))
    }

    /// Mutate routing and wake the main loop when its gesture queue stops being empty.
    fn update_routing<T>(&self, update: impl FnOnce(&mut Routing) -> T) -> T {
        let mut routing = self.routing.borrow_mut();
        let idle = routing.pending() == 0;
        let result = update(&mut routing);
        if idle && routing.pending() != 0 {
            self.events.wake();
        }
        result
    }

    /// Sample the user's double-click rule with the press's own message time and DPI.
    fn press(&self, button: Button, point: [i32; 2]) -> Message {
        // SAFETY: These reads copy this UI thread's message time, the live window's DPI and
        // the matching system metrics without retaining any handle.
        let rule = unsafe {
            let dpi = GetDpiForWindow(self.window);
            ClickRule {
                time: GetMessageTime() as u32,
                bounds: [
                    GetSystemMetricsForDpi(SM_CXDOUBLECLK, dpi),
                    GetSystemMetricsForDpi(SM_CYDOUBLECLK, dpi),
                ],
                limit: GetDoubleClickTime(),
                drag: [
                    GetSystemMetricsForDpi(SM_CXDRAG, dpi),
                    GetSystemMetricsForDpi(SM_CYDRAG, dpi),
                ],
            }
        };
        Message::Press { button, point, rule }
    }

    /// End gestures interrupted outside their own release. A cancellation also releases every
    /// button the page saw held, at the last forwarded point, so no page drag stays captured.
    fn interrupt(&self, interruption: Interruption) -> Result<(), DesktopError> {
        match interruption {
            Interruption::CaptureLost => self.update_routing(Routing::cancel),
            Interruption::Cancelled => {
                self.update_routing(Routing::interrupt);
                let (keys, point) = self.page.get();
                self.page.set((0, point));
                for release in cancellation::releases(keys) {
                    // SAFETY: The controller is retained on its creating UI apartment.
                    unsafe {
                        self.controller.SendMouseInput(
                            COREWEBVIEW2_MOUSE_EVENT_KIND(release.kind as i32),
                            COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS(i32::from(release.keys)),
                            release.data,
                            point,
                        )
                    }
                    .map_err(|cause| DesktopError::new("release_composed_browser_mouse", cause))?;
                }
            }
        }
        self.present_cursor()
    }

    /// Run a queued copy drag while the primary button is still held. This window's drop target
    /// refuses it, so the page never receives its own output, and WebView2 then sees the primary
    /// release OLE consumed. A failure loses only this export; the window stays intact.
    fn export(&self, drag: MainCanvasDragOut, exporter: &Exporter) {
        // SAFETY: Reads this thread's synchronized key state without retaining any handle.
        let held = unsafe { GetKeyState(i32::from(VK_LBUTTON.0)) } < 0;
        if !held || !self.admits() {
            // A drag started after the release would drop wherever the pointer rests.
            tracing::debug!(
                operation = "canvas_drag_out",
                "Drag-out ended before its output was ready"
            );
            return;
        }
        let outcome = self.callbacks.enter(&self.events).and_then(|_scope| {
            // The main loop keeps driving native effects while OLE's modal loop dispatches its
            // events, so only the WebView2 release below runs marked as a native callback.
            let outcome = exporter.export(|| drag_source::run(drag));
            let _callback = self.events.callback();
            // OLE consumed the primary release; end the page's and routing's view of the press.
            let released = self.interrupt(Interruption::Cancelled);
            let effect = outcome??;
            released.map(|()| effect)
        });
        match outcome {
            Ok(effect) => tracing::debug!(
                operation = "canvas_drag_out",
                copied = effect.0 != 0,
                "Canvas drag-out finished"
            ),
            Err(cause) => {
                tracing::warn!(operation = "canvas_drag_out", cause = %cause, "Canvas drag-out failed");
            }
        }
    }

    /// Run one host operation that calls into WebView2 under the callback fence; a failure
    /// closes this window through the main actor's failure path.
    fn guarded(&self, operation: impl FnOnce() -> Result<(), DesktopError>) {
        let _callback = self.events.callback();
        match self.callbacks.enter(&self.events) {
            Ok(_scope) => {
                if let Err(cause) = operation() {
                    self.events.fail(cause);
                }
            }
            Err(cause) => self.events.fail(cause),
        }
    }

    /// Show the routed hand, or the browser cursor once routing no longer names one.
    fn present_cursor(&self) -> Result<(), DesktopError> {
        let native = self.routing.borrow().cursor();
        let present = self.pointer.present(cursor::captured(self.window));
        self.hands.apply(self.window, native, self.pointer.cursor(), present)
    }

    fn admits(&self) -> bool {
        self.active.get() && self.events.accepting()
    }

    /// Resolve the browser's new cursor and select it when Windows will not ask for it.
    fn cursor_changed(&self) {
        if !self.admits() {
            return;
        }
        let _callback = self.events.callback();
        let _scope = match self.callbacks.enter(&self.events) {
            Ok(scope) => scope,
            Err(cause) => return self.events.fail(cause),
        };
        match cursor::resolve(&self.controller) {
            Ok(resolved) => {
                if let Some(now) = self.pointer.change(resolved, cursor::captured(self.window))
                    && self.routing.borrow().cursor().is_none()
                {
                    cursor::select(now);
                }
            }
            Err(cause) => self.events.fail(cause),
        }
    }
}

/// Retain the exact subclass, cursor subscription and drop registration until confirmed
/// removal or final HWND destruction; each is removed before the controller is released.
pub(super) struct Input {
    state: Rc<State>,
    registration: Registration<Rc<State>>,
    cursor: Subscription,
    drop_target: Option<DropTarget>,
}

impl Input {
    /// Install only on the owning main HWND, after its composition controller is ready.
    pub(super) fn new(
        window: HWND,
        controller: ICoreWebView2CompositionController,
        events: Rc<Events>,
    ) -> Result<Self, DesktopError> {
        let browser = controller
            .cast::<ICoreWebView2Controller>()
            .map_err(|cause| DesktopError::new("query_composed_browser_controller", cause))?;
        let regions = controller.cast::<ICoreWebView2CompositionController4>().ok();
        let state = Rc::new(State {
            window,
            controller,
            regions,
            events,
            active: Cell::new(true),
            pointer: Pointer::default(),
            routing: RefCell::new(Routing::new()?),
            hands: Hands::new(window)?,
            focus: Focus::new(browser),
            page: Cell::new((0, POINT::default())),
            callbacks: CallbackFence::default(),
            drag_out: RefCell::new(None),
        });
        let callback = Rc::new(state.clone());
        let registration = Registration::new(&callback);
        // SAFETY: This apartment owns the live HWND and retains the exact callback registration.
        unsafe { windows_subclass::install(window, Some(procedure), &registration)? };
        Ok(Self { state, registration, cursor: Subscription::default(), drop_target: None })
    }

    /// Attach cursor presentation, focus reporting and drag forwarding; a partial attachment
    /// remains retirable.
    pub(super) fn attach(&mut self) -> Result<(), DesktopError> {
        let observed = self.state.clone();
        self.cursor.install(&self.state.controller, move || observed.cursor_changed())?;
        self.state.focus.install()?;
        if self.drop_target.is_none() {
            self.drop_target = Some(DropTarget::register(
                self.state.window,
                &self.state.controller,
                self.state.events.clone(),
            )?);
        }
        Ok(())
    }

    /// Install the canvas owner's current input region, or withdraw routing with `None`.
    /// The region is the shown clip in physical client pixels, its occlusions and whether a
    /// primary drag from it exports the shown output.
    pub(super) fn publish(&self, region: Option<(PhysicalRect, &[PhysicalRect], bool)>) {
        let region = region.filter(|_| self.state.admits());
        self.state.update_routing(|routing| routing.publish(region));
        if let Err(cause) = self.state.present_cursor() {
            self.state.events.fail(cause);
        }
    }

    /// Take the oldest gesture routed to native navigation.
    pub(super) fn take(&self) -> Option<MainCanvasGesture> {
        self.state.routing.borrow_mut().take()
    }

    /// Queue the OS copy drag of the shown output for this window's next message dispatch.
    ///
    /// The drag runs modally from the window procedure rather than from the caller: OLE's
    /// modal loop dispatches paints that Tao delivers to its event handler, which must not be
    /// running already. A newer drag replaces one still waiting.
    /// # Errors
    /// Fails when the queued drag cannot be posted to the window; nothing stays queued.
    pub(super) fn drag_out(&self, drag: MainCanvasDragOut) -> Result<(), DesktopError> {
        let Some(drop_target) = self.drop_target.as_ref().filter(|_| self.state.admits()) else {
            return Ok(());
        };
        self.state.drag_out.replace(Some((drag, drop_target.exporter())));
        // SAFETY: Posts a private message to this thread's live HWND without borrowing memory.
        unsafe { PostMessageW(Some(self.state.window), DRAG_OUT_MESSAGE, WPARAM(0), LPARAM(0)) }
            .map_err(|cause| {
                self.state.drag_out.take();
                DesktopError::new("queue_canvas_drag_out", cause)
            })
    }

    /// Fence further browser input before any asynchronous main-window retirement.
    pub(super) fn suspend(&self) {
        self.state.active.set(false);
        self.state.drag_out.take();
        self.state.routing.borrow_mut().publish(None);
        if let Some(drop_target) = &self.drop_target {
            drop_target.suspend();
        }
    }

    /// Revoke drag forwarding, remove the cursor and focus subscriptions, then the subclass.
    /// Every failure retains its native custody for a later retry.
    pub(super) fn retire(&mut self) -> Result<bool, DesktopError> {
        self.suspend();
        if let Some(drop_target) = &mut self.drop_target {
            if !drop_target.retire()? {
                return Ok(false);
            }
            self.drop_target = None;
        }
        self.cursor.retire(&self.state.controller)?;
        self.state.focus.retire()?;
        let removed = retire_when_quiet(&self.state.callbacks, || {
            windows_subclass::remove(self.state.window, Some(procedure), &self.registration)
        })?;
        if removed {
            self.state.hands.retire();
        }
        Ok(removed)
    }
}

unsafe extern "system" fn procedure(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    // SAFETY: Installation retains this exact callback type for the complete native registration.
    let state = unsafe { windows_subclass::callback::<Rc<State>>(data) }.clone();
    if message == DRAG_OUT_MESSAGE {
        let queued = state.drag_out.take();
        if let Some((drag, exporter)) = queued {
            state.export(drag, &exporter);
        }
        return LRESULT(0);
    }
    if message == WM_NCDESTROY {
        state.active.set(false);
        state.drag_out.take();
        state.hands.retire();
        // SAFETY: The final callback returns this exact native registration after chaining.
        return unsafe {
            windows_subclass::destroy_callback::<Rc<State>>(
                window,
                Some(procedure),
                wparam,
                lparam,
                data,
            )
        };
    }
    if message == WM_SETCURSOR && state.admits() && cursor::claims(window, wparam, lparam) {
        let native = state.routing.borrow().cursor();
        let selected = match cursor::shown(native, state.pointer.cursor()) {
            Some(cursor::Shown::Native(_)) => state.hands.routed(),
            Some(cursor::Shown::Browser(current)) => Some(current),
            None => None,
        };
        if let Some(current) = selected {
            cursor::select(current);
            return LRESULT(1);
        }
    }
    if message == WM_NCHITTEST && state.admits() {
        // SAFETY: Tao answers its frame edges first; only its client answer may become caption.
        let hit = unsafe { DefSubclassProc(window, message, wparam, lparam) };
        let mut caption = None;
        if hit.0 == HTCLIENT as isize {
            state.guarded(|| {
                caption = state.caption(lparam)?;
                Ok(())
            });
        }
        return caption.unwrap_or(hit);
    }
    if state.admits()
        && let Some(interruption) = cancellation::interruption(message, wparam, lparam, window)
    {
        state.guarded(|| state.interrupt(interruption));
    }
    if state.admits()
        && let Some(message) = mouse(message, wparam, lparam)
    {
        state.guarded(|| state.deliver(message));
    }
    // SAFETY: Preserve Tao's own capture, focus, keyboard, IME, window and caption processing.
    let result = unsafe { DefSubclassProc(window, message, wparam, lparam) };
    // Activation gives this top-level window focus; pass it on once Tao has observed it.
    if message == WM_SETFOCUS && state.admits() {
        state.guarded(|| {
            state.focus.enter();
            Ok(())
        });
    }
    // A routed hand loaded at the old DPI is rebuilt and reselected without waiting for movement.
    if message == WM_DPICHANGED && state.admits() {
        state.guarded(|| state.present_cursor());
    }
    result
}

#[cfg(test)]
mod tests;
