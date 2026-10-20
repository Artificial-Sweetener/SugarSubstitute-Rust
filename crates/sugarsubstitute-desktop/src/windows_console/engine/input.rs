// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep one checked native input subclass attached to the original single document.
use super::Engine;
use crate::{
    DesktopError,
    windows_subclass::{Registration, callback, destroy_callback, install, remove},
};
use std::rc::{Rc, Weak};
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::{
        Controls::{
            EM_CANUNDO, EM_CHARFROMPOS, EM_EMPTYUNDOBUFFER, EM_GETFIRSTVISIBLELINE, EM_GETLINE,
            EM_GETLINECOUNT, EM_GETMARGINS, EM_GETMODIFY, EM_GETRECT, EM_GETSEL, EM_LIMITTEXT,
            EM_LINEFROMCHAR, EM_LINEINDEX, EM_LINELENGTH, EM_LINESCROLL, EM_POSFROMCHAR,
            EM_REPLACESEL, EM_SCROLL, EM_SCROLLCARET, EM_SETMARGINS, EM_SETMODIFY, EM_SETREADONLY,
            EM_SETRECT, EM_SETRECTNP, EM_SETSEL, EM_UNDO,
        },
        Shell::{DefSubclassProc, GetWindowSubclass},
        WindowsAndMessaging::*,
    },
};

/// The native registration holds a weak engine identity and cannot keep a retired document alive.
struct Context {
    engine: Weak<Engine>,
    hwnd: HWND,
}
/// Record exact native callback custody before any reentrant installation effect.
pub(super) struct Input {
    registration: Registration<Context>,
    hwnd: HWND,
    engine: Weak<Engine>,
    scroll: super::scroll_input::ScrollInput,
}
impl Input {
    /// Construct inert metadata before its containing engine adopts the registration owner.
    pub(super) fn new(engine: &Rc<Engine>) -> Self {
        let context = Rc::new(Context { engine: Rc::downgrade(engine), hwnd: engine.state.hwnd });
        Self {
            registration: Registration::new(&context),
            hwnd: engine.state.hwnd,
            engine: Rc::downgrade(engine),
            scroll: super::scroll_input::ScrollInput::new(),
        }
    }
    /// Attach the affine message adapter only after partial custody is retained.
    pub(super) fn install(&self) -> Result<(), DesktopError> {
        if usize::BITS < 64 {
            return Err(DesktopError::message(
                "console_input_platform",
                "native input requires a 64-bit Windows host",
            ));
        }
        // SAFETY: The engine retains this exact native child and registration on its creating thread.
        unsafe { install(self.hwnd, Some(procedure), &self.registration) }
    }
    /// Stop native delivery before the engine's COM interfaces or system DLL can retire.
    pub(super) fn close(&self) -> Result<(), DesktopError> {
        if let Some(engine) = self.engine.upgrade() {
            self.scroll.cancel(&engine, true)?;
            engine.state.release_capture()?;
        }
        if self.registration.installed() {
            remove(self.hwnd, Some(procedure), &self.registration)?;
        }
        Ok(())
    }
}
/// Borrow only an actual still-registered document for a typed native observation.
pub(in super::super) fn find(hwnd: HWND) -> Result<Rc<Engine>, DesktopError> {
    let mut data = 0;
    // SAFETY: Inspect the exact private procedure/ID pair without interpreting foreign callback data.
    if !unsafe { GetWindowSubclass(hwnd, Some(procedure), 1, Some(&mut data)) }.as_bool()
        || data == 0
    {
        return Err(DesktopError::message(
            "console_input_identity",
            "native text engine is not registered",
        ));
    }
    // SAFETY: Only this private procedure installs Context references under this exact pair.
    let context = unsafe { callback::<Context>(data) };
    if context.hwnd != hwnd {
        return Err(DesktopError::message(
            "console_input_identity",
            "native child identity differs",
        ));
    }
    context.engine.upgrade().ok_or_else(|| {
        DesktopError::message("console_input_identity", "native text engine retired")
    })
}
/// # Safety
/// Installed only with the original retained Context registration on its creating UI thread.
unsafe extern "system" fn procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    if message == WM_NCDESTROY {
        // SAFETY: Return exactly this registration's native reference through its final callback.
        return unsafe { destroy_callback::<Context>(hwnd, Some(procedure), wparam, lparam, data) };
    }
    // SAFETY: This original callback remains registered until removal or final destruction.
    let context = unsafe { callback::<Context>(data) };
    let Some(engine) = context.engine.upgrade().filter(|engine| engine.state.hwnd == hwnd) else {
        // SAFETY: Preserve the chain when this exact document is no longer available.
        return unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    };
    match engine.dispatch(message, wparam, lparam) {
        Ok(Some(value)) => value,
        Ok(None) => {
            // SAFETY: Non-text window messages retain the original native child procedure.
            unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
        }
        Err(cause) => {
            engine.state.record(cause);
            LRESULT(0)
        }
    }
}

impl Engine {
    /// Install input after this exact engine is adopted by its containing native peer.
    pub(in super::super) fn install_input(self: &Rc<Self>) -> Result<(), DesktopError> {
        if self.input.borrow().is_some() {
            return Err(DesktopError::message(
                "console_input_install",
                "native input already registered",
            ));
        }
        *self.input.borrow_mut() = Some(Input::new(self));
        self.input
            .borrow()
            .as_ref()
            .ok_or_else(|| {
                DesktopError::message("console_input_install", "native input custody absent")
            })?
            .install()
    }
    /// Forward only messages whose meaning belongs to this exact native document/input owner.
    fn dispatch(
        &self,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Result<Option<LRESULT>, DesktopError> {
        if self.state.closing.get() || self.state.painting.get() {
            return Ok(Some(LRESULT(0)));
        }
        if let Some(input) = self.input.borrow().as_ref()
            && input.scroll.message(self, message, wparam, lparam)?
        {
            return Ok(Some(LRESULT(0)));
        }
        match message {
            // The backing STATIC procedure otherwise returns HTTRANSPARENT, preventing
            // Windows from routing pointer input to this text-services host.
            WM_NCHITTEST => Ok(Some(LRESULT(HTCLIENT as isize))),
            WM_PAINT => {
                self.paint()?;
                Ok(Some(LRESULT(0)))
            }
            WM_GETOBJECT => match self.accessibility.borrow().as_ref() {
                Some(owner) => Ok(Some(owner.message(wparam, lparam)?)),
                None => Ok(Some(LRESULT(0))),
            },
            WM_SETTINGCHANGE => {
                self.refresh_system_theme()?;
                self.state.invalidate();
                Ok(None)
            }
            WM_SETREDRAW => {
                self.state.redraw.set(wparam.0 != 0);
                self.state
                    .observing_blocked
                    .set(!self.state.redraw.get() || self.state.painting.get());
                if self.state.redraw.get() {
                    self.state.invalidate();
                }
                Ok(Some(LRESULT(0)))
            }
            WM_ERASEBKGND => Ok(Some(LRESULT(1))),
            WM_SIZE => {
                self.state.refresh_bounds()?;
                // SAFETY: Layout changes belong to this same retained affine document.
                unsafe {
                    self.services()?.OnTxPropertyBitsChange(
                        windows::Win32::UI::Controls::RichEdit::TXTBIT_CLIENTRECTCHANGE,
                        0,
                    )
                }
                .map_err(|cause| DesktopError::new("console_layout", cause))?;
                Ok(None)
            }
            WM_SETFOCUS => {
                // SAFETY: The real native focus event activates only its original retained document.
                unsafe { self.services()?.OnTxUIActivate() }
                    .map_err(|cause| DesktopError::new("console_focus_activate", cause))?;
                self.state.ui_active.set(true);
                self.forward(message, wparam, lparam)
            }
            WM_KILLFOCUS => {
                let result = self.forward(message, wparam, lparam);
                // SAFETY: Focus retirement is attempted even when the original native message failed.
                let deactivated = unsafe { self.services()?.OnTxUIDeactivate() }
                    .map_err(|cause| DesktopError::new("console_focus_deactivate", cause));
                if deactivated.is_ok() {
                    self.state.ui_active.set(false);
                }
                match result {
                    Ok(value) => deactivated.map(|_| value),
                    Err(cause) => {
                        if let Err(ending) = deactivated {
                            tracing::error!(operation="console_focus_deactivate",error=%ending);
                        }
                        Err(cause)
                    }
                }
            }
            WM_TIMER if !self.state.owns_timer(wparam.0) => Ok(None),
            EM_GETSEL
            | EM_SETSEL
            | EM_GETRECT
            | EM_SETRECT
            | EM_SETRECTNP
            | EM_SCROLL
            | EM_LINESCROLL
            | EM_SCROLLCARET
            | EM_GETMODIFY
            | EM_SETMODIFY
            | EM_GETLINECOUNT
            | EM_LINEINDEX
            | EM_LINELENGTH
            | EM_REPLACESEL
            | EM_GETLINE
            | EM_LIMITTEXT
            | EM_CANUNDO
            | EM_UNDO
            | EM_LINEFROMCHAR
            | EM_EMPTYUNDOBUFFER
            | EM_GETFIRSTVISIBLELINE
            | EM_SETREADONLY
            | EM_SETMARGINS
            | EM_GETMARGINS
            | EM_POSFROMCHAR
            | EM_CHARFROMPOS => self.forward(message, wparam, lparam),
            WM_GETDLGCODE | WM_GETTEXT | WM_GETTEXTLENGTH | WM_SETTEXT | WM_COPY | WM_CUT
            | WM_PASTE | WM_CLEAR | WM_UNDO | WM_CHAR | WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN
            | WM_SYSKEYUP | WM_TIMER | WM_MOUSEACTIVATE | WM_SETCURSOR | WM_CAPTURECHANGED
            | WM_CANCELMODE | WM_CONTEXTMENU | WM_VSCROLL | WM_HSCROLL => {
                self.forward(message, wparam, lparam)
            }
            value if (WM_MOUSEFIRST..=WM_MOUSELAST).contains(&value) || value >= WM_USER => {
                self.forward(message, wparam, lparam)
            }
            _ => Ok(None),
        }
    }
}

impl Input {
    /// Derive scrollbar pixels from the original native engine's range and this input owner's hover.
    pub(super) fn scroll_model(
        engine: &Engine,
    ) -> Result<Option<super::scrollbar::Model>, DesktopError> {
        match engine.input.borrow().as_ref() {
            Some(input) => input.scroll.model(engine),
            None => Ok(None),
        }
    }
}
impl Engine {
    /// Validate one exact native paint lifetime, including EndPaint after a renderer failure.
    fn paint(&self) -> Result<(), DesktopError> {
        use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, PAINTSTRUCT};
        let mut paint = PAINTSTRUCT::default();
        // SAFETY: The original UI-thread child owns this WM_PAINT and writable paint structure.
        let dc = unsafe { BeginPaint(self.state.hwnd, &mut paint) };
        let result = if dc.is_invalid() {
            Err(DesktopError::message("console_begin_paint", "native paint DC absent"))
        } else if !self.state.redraw.get() {
            Ok(())
        } else {
            match self.renderer.try_borrow_mut() {
                Ok(mut renderer) => match renderer.as_mut() {
                    Some(renderer) => renderer.paint(self, dc),
                    None => Ok(()),
                },
                Err(cause) => Err(DesktopError::new("console_paint_reentry", cause)),
            }
        };
        // SAFETY: This balances the exact BeginPaint even if no renderer or a rejected draw exists.
        let ended = unsafe { EndPaint(self.state.hwnd, &paint) }
            .ok()
            .map_err(|cause| DesktopError::new("console_end_paint", cause));
        match (result, ended) {
            (Err(cause), Err(ending)) => {
                tracing::error!(operation="console_end_paint",error=%ending);
                Err(cause)
            }
            (Err(cause), _) | (_, Err(cause)) => Err(cause),
            _ => Ok(()),
        }
    }
}
