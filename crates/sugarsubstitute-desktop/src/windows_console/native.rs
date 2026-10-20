// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Implement only the checked Win32 boundary behind the retained Creation owner.
use super::*;
use crate::{
    AccessibleControl,
    control_geometry::physical_bounds,
    windows_subclass::{Registration, install, remove},
};
use std::{cell::Cell, ffi::c_void};
use tao::platform::windows::WindowExtWindows;
use windows::{
    Win32::{
        Foundation::{FreeLibrary, HMODULE, HWND, LPARAM, WPARAM},
        System::LibraryLoader::{LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW},
        UI::Controls::RichEdit::{EM_AUTOURLDETECT, EM_EXLIMITTEXT},
        UI::{
            Input::KeyboardAndMouse::{EnableWindow, GetFocus, IsWindowEnabled},
            WindowsAndMessaging::*,
        },
    },
    core::w,
};
/// Keep the native input child hidden until presentation enables this focusable surface.
pub(super) fn peer_style() -> WINDOW_STYLE {
    WS_CHILD | WS_TABSTOP
}
/// A loaded system module stays retained before any following COM/child effect.
pub(super) struct Library {
    module: Cell<Option<HMODULE>>,
}
impl Library {
    /// Borrow the exact system module whose retained owner outlives every text engine.
    pub(super) fn module(&self) -> Option<HMODULE> {
        self.module.get()
    }
}
pub(super) struct Peer {
    pub(super) hwnd: HWND,
    registration: Option<Registration<callback::Callback>>,
    pub(super) engine: Rc<super::engine::Engine>,
    module: HMODULE,
    parent: HWND,
}
impl Peer {
    /// A reused HWND cannot substitute for the exact still-installed owned callback.
    pub(super) fn live(&self) -> Result<(), DesktopError> {
        let registration = self
            .registration
            .as_ref()
            .ok_or_else(|| DesktopError::message("console_identity", "callback custody absent"))?;
        let mut data = 0;
        if !registration.installed()
            || registration.destroyed()
            // SAFETY: Query the owned subclass identity without dereferencing callback data.
            || !unsafe {
                windows::Win32::UI::Shell::GetWindowSubclass(
                    self.hwnd,
                    Some(callback::procedure),
                    1,
                    Some(&raw mut data),
                )
            }
            .as_bool()
            || data != registration.data()
            // SAFETY: Read the parent identity of the checked UI-thread child without retaining it.
            || unsafe { GetAncestor(self.hwnd, GA_PARENT) } != self.parent
        {
            return Err(DesktopError::message(
                "console_identity",
                "original native child registration or parent changed",
            ));
        }
        self.engine.check()
    }
}
pub(super) struct Port<'a> {
    pub(super) window: &'a Window,
    pub(super) callback: &'a Rc<callback::Callback>,
    pub(super) label: &'static str,
    pub(super) bounds: Option<crate::ControlBounds>,
    pub(super) module: Option<HMODULE>,
}
impl NativePort for Port<'_> {
    type Parent = Library;
    type Peer = Peer;
    fn parent(&mut self) -> Result<Library, DesktopError> {
        Ok(Library { module: Cell::new(None) })
    }
    fn install_parent(&mut self, parent: &Library) -> Result<(), DesktopError> {
        // SAFETY: Load only the OS-supplied RichEdit library, never a working-directory DLL.
        let module =
            unsafe { LoadLibraryExW(w!("Msftedit.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32) }
                .map_err(|e| DesktopError::new("console_library_load", e))?;
        parent.module.set(Some(module));
        self.module = Some(module);
        Ok(())
    }
    fn peer(&mut self, _index: usize) -> Result<Peer, DesktopError> {
        let bounds = self.bounds.ok_or_else(|| {
            DesktopError::message("console_create_bounds", "creation geometry absent")
        })?;
        let rect = physical_bounds(
            &AccessibleControl { id: 4, label: self.label, bounds, close: false, visible: false },
            self.window.scale_factor(),
        )?;
        let module = self.module.ok_or_else(|| {
            DesktopError::message("console_library_identity", "retained system module absent")
        })?;
        // SAFETY: Original UI thread owns parent, and loaded class lives in retained Library.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!(""),
                peer_style(),
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                Some(HWND(self.window.hwnd() as *mut c_void)),
                Some(HMENU(4usize as *mut c_void)),
                None,
                None,
            )
        }
        .map_err(|e| DesktopError::new("console_window_create", e))?;
        Ok(Peer {
            hwnd,
            registration: Some(Registration::new(self.callback)),
            engine: super::engine::Engine::new(hwnd),
            module,
            parent: HWND(self.window.hwnd() as *mut c_void),
        })
    }
    fn register_peer(&mut self, peer: &Peer) -> Result<(), DesktopError> {
        if self.callback.peer.get().is_some() {
            return Err(DesktopError::message("console_bind", "callback is already bound"));
        }
        self.callback.peer.set(Some(peer.hwnd));
        Ok(())
    }
    fn install_peer(&mut self, peer: &Peer) -> Result<(), DesktopError> {
        let registration = peer
            .registration
            .as_ref()
            .ok_or_else(|| DesktopError::message("console_register", "registration absent"))?;
        peer.engine.initialize(peer.module)?;
        peer.engine.install_input()?;
        // SAFETY: The engine's input subclass is installed first, leaving application shortcuts outermost.
        unsafe { install(peer.hwnd, Some(callback::procedure), registration) }?;
        peer.engine.configure_accessibility(peer.module, self.label)?;
        peer.live()?;
        // TXTBIT_READONLY is supplied by the windowless host; HWND-only EM_SETREADONLY is
        // unprocessed by this native engine. Configure supported operations directly and check
        // their HRESULTs instead of losing S_FALSE through the window's LRESULT boundary.
        if peer.engine.message(EM_AUTOURLDETECT, WPARAM(0), LPARAM(0))?.0 != 0 {
            return Err(DesktopError::message(
                "console_links_disable",
                "native link configuration rejected",
            ));
        }
        peer.engine.message(
            EM_EXLIMITTEXT,
            WPARAM(0),
            LPARAM(
                (sugarsubstitute_contracts::transcript::MAX_SNAPSHOT_BYTES
                    + sugarsubstitute_contracts::transcript::MAX_SNAPSHOT_RECORDS)
                    as isize,
            ),
        )?;
        peer.engine.activate()?;
        peer.live()
    }
    fn destroyed(&self, peer: &Peer, _parent: Option<&Library>) -> bool {
        peer.engine.retired() && peer.registration.as_ref().is_some_and(Registration::destroyed)
    }
    fn remove_peer(&mut self, peer: &mut Peer) -> Result<(), DesktopError> {
        self.callback.active.set(false);
        peer.engine.close()?;
        if let Some(registration) = peer.registration.as_ref()
            && registration.installed()
        {
            remove(peer.hwnd, Some(callback::procedure), registration)?;
        }
        peer.registration = None;
        Ok(())
    }
    fn destroy_peer(&mut self, peer: &Peer) -> Result<(), DesktopError> {
        // SAFETY: Creation confirmed actual callback removal before this exact child destruction.
        unsafe { DestroyWindow(peer.hwnd) }
            .map_err(|e| DesktopError::new("console_window_destroy", e))
    }
    fn retire_parent(&mut self, parent: &Library) -> Result<(), DesktopError> {
        if let Some(module) = parent.module.get() {
            // SAFETY: Every original child and callback retired before unloading its class module.
            unsafe { FreeLibrary(module) }
                .map_err(|e| DesktopError::new("console_library_free", e))?;
            parent.module.set(None);
        }
        Ok(())
    }
    fn callbacks_retired(&self) -> bool {
        Rc::strong_count(self.callback) == 1
    }
}
fn label(text: &str) -> Result<[u16; 129], DesktopError> {
    let mut label = [0u16; 129];
    let mut count = 0;
    for unit in text.encode_utf16() {
        if unit == 0 || count == 128 {
            return Err(DesktopError::message(
                "console_accessibility_label",
                "invalid bounded label",
            ));
        }
        label[count] = unit;
        count += 1;
    }
    if count == 0 {
        return Err(DesktopError::message("console_accessibility_label", "empty label"));
    }
    Ok(label)
}
/// Geometry/availability changes retain the same control, native selection and document.
pub(super) fn present(
    window: &Window,
    hwnd: HWND,
    callback: &callback::Callback,
    view: &ConsoleView<'_>,
) -> Result<(), DesktopError> {
    let rect = physical_bounds(&view.native_control(), window.scale_factor())?;
    callback.active.set(false);
    // SAFETY: Parent and original readonly child stay on their creating UI thread.
    unsafe {
        if !view.visible && GetFocus() == hwnd {
            super::super::focus_window(HWND(window.hwnd() as *mut c_void))?;
        }
        let _previous = EnableWindow(hwnd, view.visible);
        SetWindowPos(
            hwnd,
            None,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_NOACTIVATE
                | SWP_NOZORDER
                | if view.visible { SWP_SHOWWINDOW } else { SWP_HIDEWINDOW },
        )
        .map_err(|e| DesktopError::new("console_bounds_visibility", e))?;
        if IsWindowEnabled(hwnd).as_bool() != view.visible
            || (GetWindowLongPtrW(hwnd, GWL_STYLE) as u32 & WS_VISIBLE.0 != 0) != view.visible
        {
            return Err(DesktopError::message(
                "console_availability",
                "native console availability differs",
            ));
        }
    }
    callback.active.set(view.visible);
    Ok(())
}

/// Queue a completed document/layout paint so native input can run before expensive drawing.
pub(super) fn repaint(hwnd: HWND, visible: bool) -> Result<(), DesktopError> {
    if visible {
        // SAFETY: Exact retained child is painted on its creating UI thread; no other
        // window or desktop is queried, invalidated, activated or painted.
        unsafe {
            windows::Win32::Graphics::Gdi::RedrawWindow(
                Some(hwnd),
                None,
                None,
                windows::Win32::Graphics::Gdi::RDW_INVALIDATE
                    | windows::Win32::Graphics::Gdi::RDW_FRAME,
            )
        }
        .ok()
        .map_err(|e| DesktopError::new("console_repaint", e))?;
    }
    Ok(())
}

/// Validate localized identity and native geometry before first module/child effects.
pub(super) fn validate(view: &ConsoleView<'_>, scale: f64) -> Result<(), DesktopError> {
    label(view.label)?;
    physical_bounds(&view.native_control(), scale)?;
    Ok(())
}
