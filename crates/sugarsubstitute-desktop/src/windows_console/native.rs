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
        System::{
            Com::{CLSCTX_INPROC_SERVER, CoCreateInstance},
            LibraryLoader::{LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW},
        },
        UI::Controls::{
            EM_SETREADONLY,
            RichEdit::{
                EM_AUTOURLDETECT, EM_EXLIMITTEXT, EM_SETUNDOLIMIT, ES_NOOLEDRAGDROP, ES_SAVESEL,
            },
        },
        UI::{
            Accessibility::{
                CAccPropServices, IAccPropServices, Name_Property_GUID, PROPID_ACC_NAME,
            },
            Input::KeyboardAndMouse::{EnableWindow, GetFocus, IsWindowEnabled},
            WindowsAndMessaging::*,
        },
    },
    core::{PCWSTR, w},
};
/// Keep creation hidden with one readonly child style for production and native fixtures.
pub(super) fn peer_style() -> WINDOW_STYLE {
    WS_CHILD
        | WS_TABSTOP
        | WS_VSCROLL
        | WINDOW_STYLE(
            (ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL | ES_NOHIDESEL) as u32
                | ES_NOOLEDRAGDROP
                | ES_SAVESEL,
        )
}
/// A loaded system module stays retained before any following COM/child effect.
pub(super) struct Library {
    module: Cell<Option<HMODULE>>,
}
pub(super) struct Peer {
    pub(super) hwnd: HWND,
    registration: Option<Registration<callback::Callback>>,
    access: IAccPropServices,
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
        Ok(())
    }
}
pub(super) struct Port<'a> {
    pub(super) window: &'a Window,
    pub(super) callback: &'a Rc<callback::Callback>,
    pub(super) label: &'static str,
    pub(super) bounds: Option<crate::ControlBounds>,
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
        // SAFETY: The composing startup owns a live STA; creation returns an owned COM interface.
        let access: IAccPropServices =
            unsafe { CoCreateInstance(&CAccPropServices, None, CLSCTX_INPROC_SERVER) }
                .map_err(|e| DesktopError::new("console_accessibility_create", e))?;
        // SAFETY: Original UI thread owns parent, and loaded class lives in retained Library.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("RICHEDIT50W"),
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
            access,
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
        // SAFETY: Adopted peer and exact Callback precede native reentry and configuration.
        unsafe { install(peer.hwnd, Some(callback::procedure), registration) }?;
        let label = label(self.label)?;
        // SAFETY: Exact retained peer and synchronous bounded label; name never derives from user text.
        for property in [PROPID_ACC_NAME, Name_Property_GUID] {
            peer.live()?;
            // SAFETY: Annotate only this exact retained standard control's name. Its
            // native readonly/text/selection provider stays authoritative for its patterns.
            unsafe {
                peer.access.SetHwndPropStr(
                    peer.hwnd,
                    OBJID_CLIENT.0 as u32,
                    0,
                    property,
                    PCWSTR(label.as_ptr()),
                )
            }
            .map_err(|e| DesktopError::new("console_accessibility_name", e))?;
        }
        peer.live()?;
        // SAFETY: Configure only this retained readonly plain-text input surface. Disable undo,
        // URL activation and OLE drag/drop; streaming accepts Unicode plain text only.
        unsafe {
            if SendMessageW(peer.hwnd, EM_SETREADONLY, Some(WPARAM(1)), None).0 == 0 {
                return Err(DesktopError::message(
                    "console_readonly",
                    "native read-only configuration rejected",
                ));
            }
            SendMessageW(peer.hwnd, EM_SETUNDOLIMIT, Some(WPARAM(0)), None);
            if SendMessageW(peer.hwnd, EM_AUTOURLDETECT, Some(WPARAM(0)), None).0 != 0 {
                return Err(DesktopError::message(
                    "console_links_disable",
                    "native link configuration rejected",
                ));
            }
            SendMessageW(
                peer.hwnd,
                EM_EXLIMITTEXT,
                None,
                Some(LPARAM(
                    (sugarsubstitute_contracts::transcript::MAX_SNAPSHOT_BYTES
                        + sugarsubstitute_contracts::transcript::MAX_SNAPSHOT_RECORDS)
                        as isize,
                )),
            );
        }
        peer.live()
    }
    fn destroyed(&self, peer: &Peer, _parent: Option<&Library>) -> bool {
        peer.registration.as_ref().is_some_and(Registration::destroyed)
    }
    fn remove_peer(&mut self, peer: &mut Peer) -> Result<(), DesktopError> {
        self.callback.active.set(false);
        // SAFETY: Exact owned child remains retained; failed annotation cleanup preserves custody.
        unsafe {
            peer.access.ClearHwndProps(
                peer.hwnd,
                OBJID_CLIENT.0 as u32,
                0,
                &[PROPID_ACC_NAME, Name_Property_GUID],
            )
        }
        .map_err(|e| DesktopError::new("console_accessibility_clear", e))?;
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
    let rect = physical_bounds(
        &AccessibleControl {
            id: 4,
            label: view.label,
            bounds: view.bounds,
            close: false,
            visible: view.visible,
        },
        window.scale_factor(),
    )?;
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

/// Parent CPU blits do not clip transparent button peers. Repaint only this owned
/// visible child afterward so text, caret and selection remain native and current.
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
                    | windows::Win32::Graphics::Gdi::RDW_ERASE
                    | windows::Win32::Graphics::Gdi::RDW_UPDATENOW,
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
    physical_bounds(
        &AccessibleControl {
            id: 4,
            label: view.label,
            bounds: view.bounds,
            close: false,
            visible: view.visible,
        },
        scale,
    )?;
    Ok(())
}
