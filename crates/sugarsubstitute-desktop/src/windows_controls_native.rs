// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Create and retire all native form peers through the shared checked transaction.
use super::*;
use sugarsubstitute_ui::form::Role;
use windows::Win32::Foundation::HMODULE;
/// Borrow the exact retained parent/callback and bounded planned metadata for native effects.
pub(super) struct ControlPort<'a> {
    pub(super) window: &'a Window,
    pub(super) callback: &'a Rc<Callback>,
    pub(super) planned: &'a [Specification],
    pub(super) paint_mode: PaintMode,
    pub(super) module: Option<HMODULE>,
    pub(super) font: Option<windows::Win32::Graphics::Gdi::HFONT>,
}
impl NativePort for ControlPort<'_> {
    type Parent = Registration<Callback>;
    type Peer = Peer;
    fn parent(&mut self) -> Result<Self::Parent, DesktopError> {
        Ok(Registration::new(self.callback))
    }
    fn install_parent(&mut self, parent: &Self::Parent) -> Result<(), DesktopError> {
        // SAFETY: This thread owns the retained parent and exact Callback registration.
        unsafe { install(HWND(self.window.hwnd() as *mut c_void), Some(parent_proc), parent) }
    }
    fn peer(&mut self, index: usize) -> Result<Self::Peer, DesktopError> {
        let specification = self.planned.get(index).ok_or_else(|| {
            DesktopError::message("create_accessible_control", "planned identity missing")
        })?;
        let control = specification.control;
        let (class, style) = values::native_style(self.paint_mode, specification);
        let label: Vec<u16> = control.label.encode_utf16().chain(Some(0)).collect();
        let rect = values::peer_rect(control, &specification.role, self.window.scale_factor())?;
        // SAFETY: The owned parent belongs to this thread; BUTTON copies the bounded label.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class,
                PCWSTR(label.as_ptr()),
                style,
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                Some(HWND(self.window.hwnd() as *mut c_void)),
                Some(HMENU(usize::from(control.id) as *mut c_void)),
                None,
                None,
            )
        }
        .map_err(|cause| DesktopError::new("create_accessible_control", cause))?;
        Ok(Peer {
            hwnd,
            snapshot: control,
            rect,
            registration: Some(Registration::new(self.callback)),
            role: specification.role.clone(),
            named: std::cell::Cell::new(false),
            engine: matches!(specification.role, Role::TextField(_)).then(|| {
                crate::windows_text::Engine::new(
                    hwnd,
                    sugarsubstitute_ui::windows::text_style::TextStyle::Field,
                )
            }),
        })
    }
    fn register_peer(&mut self, peer: &Self::Peer) -> Result<(), DesktopError> {
        self.callback.register(
            peer.hwnd,
            peer.snapshot.id,
            action(self.paint_mode, peer.snapshot),
            values::input(&peer.role),
        )
    }
    fn install_peer(&mut self, peer: &Self::Peer) -> Result<(), DesktopError> {
        let registration = peer.registration.as_ref().ok_or_else(|| {
            DesktopError::message("register_accessible_child", "retained registration missing")
        })?;
        if let Some(engine) = &peer.engine {
            let module = self.module.ok_or_else(|| {
                DesktopError::message("native_field_module", "text library absent")
            })?;
            engine.initialize(module)?;
            engine.configure_accessibility(module, peer.snapshot.label)?;
            engine.install_input()?;
            engine.activate()?;
        }
        values::initialize(peer, self.callback)?;
        if matches!(peer.role, Role::Select { .. }) {
            sugarsubstitute_ui::windows::select::configure(
                peer.hwnd,
                self.font.ok_or_else(|| {
                    DesktopError::message("native_select_font", "Original font absent")
                })?,
                peer.rect.bottom - peer.rect.top,
            )?;
        }
        // SAFETY: The transaction retains this exact child and Callback before native reentry.
        unsafe { install(peer.hwnd, Some(child_proc), registration) }
    }
    fn destroyed(&self, peer: &Self::Peer, parent: Option<&Self::Parent>) -> bool {
        peer.engine.as_ref().is_none_or(|engine| engine.retired())
            && (peer.registration.as_ref().is_some_and(Registration::destroyed)
                || parent.is_some_and(Registration::destroyed))
    }
    fn remove_peer(&mut self, peer: &mut Self::Peer) -> Result<(), DesktopError> {
        accessibility::clear(peer)?;
        if let Some(engine) = &peer.engine {
            engine.close()?;
        }
        if let Some(registration) = &peer.registration {
            if registration.installed() {
                remove(peer.hwnd, Some(child_proc), registration)?;
            }
            peer.registration = None;
        }
        Ok(())
    }
    fn destroy_peer(&mut self, peer: &Self::Peer) -> Result<(), DesktopError> {
        // SAFETY: The transaction observed successful callback removal before this effect.
        unsafe { DestroyWindow(peer.hwnd) }
            .map_err(|cause| DesktopError::new("destroy_accessible_control", cause))
    }
    fn retire_parent(&mut self, parent: &Self::Parent) -> Result<(), DesktopError> {
        if parent.destroyed() || !parent.installed() {
            return Ok(());
        }
        remove(HWND(self.window.hwnd() as *mut c_void), Some(parent_proc), parent)
    }
    fn callbacks_retired(&self) -> bool {
        Rc::strong_count(self.callback) == 1
    }
}
