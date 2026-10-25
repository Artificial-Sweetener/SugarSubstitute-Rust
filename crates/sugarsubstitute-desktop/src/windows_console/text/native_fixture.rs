// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own hidden RichEdit resources and the outer qualification's MTA cookie on one STA.
use crate::windows_text::Engine;
use crate::{DesktopError, console::index::DocumentIndex, windows_ui_apartment::UiApartment};
use std::rc::Rc;
use windows::{
    Win32::{
        Foundation::{FreeLibrary, HMODULE, HWND},
        System::{
            Com::{
                APTTYPE, APTTYPE_CURRENT, APTTYPEQUALIFIER, APTTYPEQUALIFIER_NONE,
                CoGetApartmentType,
            },
            LibraryLoader::{LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW},
        },
        UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WINDOW_STYLE},
    },
    core::w,
};

/// Independently observe both native apartment facts without sharing the admission decision.
pub(super) fn apartment_fact(
    operation: &'static str,
) -> Result<(APTTYPE, APTTYPEQUALIFIER), DesktopError> {
    let mut kind = APTTYPE_CURRENT;
    let mut qualifier = APTTYPEQUALIFIER_NONE;
    // SAFETY: Writable scalar outputs belong to this existing qualification test thread.
    unsafe { CoGetApartmentType(&mut kind, &mut qualifier) }
        .map_err(|cause| DesktopError::new(operation, cause))?;
    Ok((kind, qualifier))
}

/// Retain each hidden native peer beneath the one borrowed outer qualification STA.
pub(super) struct Hidden<'a> {
    pub(super) hwnd: Option<HWND>,
    parent: Option<HWND>,
    module: Option<HMODULE>,
    engine: Option<Rc<Engine>>,
    _apartment: &'a UiApartment,
    pub(super) document: Option<DocumentIndex>,
}
impl<'a> Hidden<'a> {
    /// Create the production-style hidden child on the already initialized outer STA.
    pub(super) fn new(
        document: DocumentIndex,
        apartment: &'a UiApartment,
    ) -> Result<Self, DesktopError> {
        let mut owner = Self {
            hwnd: None,
            parent: None,
            module: None,
            engine: None,
            _apartment: apartment,
            document: Some(document),
        };
        // SAFETY: This test STA retains one hidden parent before creating its RichEdit child.
        owner.parent = Some(
            unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    w!("STATIC"),
                    w!(""),
                    WINDOW_STYLE::default(),
                    0,
                    0,
                    300,
                    120,
                    None,
                    None,
                    None,
                    None,
                )
            }
            .map_err(|cause| DesktopError::new("console_fixture_parent", cause))?,
        );
        // SAFETY: System-only library and hidden test-thread HWND; every returned owner is retained.
        owner.module = Some(
            unsafe { LoadLibraryExW(w!("Msftedit.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32) }
                .map_err(|e| DesktopError::new("console_fixture_library", e))?,
        );
        // SAFETY: Exact hidden parent and production child style; no focus or clipboard effects.
        owner.hwnd = Some(
            unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    w!("STATIC"),
                    w!(""),
                    crate::windows_controls::console::native::peer_style(),
                    0,
                    0,
                    300,
                    120,
                    owner.parent,
                    None,
                    None,
                    None,
                )
            }
            .map_err(|e| DesktopError::new("console_fixture_window", e))?,
        );
        let child = owner
            .hwnd
            .ok_or_else(|| DesktopError::message("console_fixture_engine", "child absent"))?;
        let module = owner
            .module
            .ok_or_else(|| DesktopError::message("console_fixture_engine", "module absent"))?;
        owner.engine = Some(Engine::new(
            child,
            sugarsubstitute_ui::windows::text_style::TextStyle::Transcript,
        ));
        let engine = owner
            .engine
            .as_ref()
            .ok_or_else(|| DesktopError::message("console_fixture_engine", "owner absent"))?;
        engine.initialize(module)?;
        engine.install_input()?;
        engine.configure_accessibility(module, "Comfy output")?;
        engine.activate()?;
        // SAFETY: Passive identity/visibility checks inspect only this test's two retained HWNDs.
        unsafe {
            let child = owner
                .hwnd
                .ok_or_else(|| DesktopError::message("console_fixture_identity", "child absent"))?;
            let parent = owner.parent.ok_or_else(|| {
                DesktopError::message("console_fixture_identity", "parent absent")
            })?;
            use windows::Win32::UI::WindowsAndMessaging::{
                GA_PARENT, GetAncestor, IsWindowVisible,
            };
            if GetAncestor(child, GA_PARENT) != parent
                || IsWindowVisible(child).as_bool()
                || IsWindowVisible(parent).as_bool()
            {
                return Err(DesktopError::message(
                    "console_fixture_identity",
                    "original child parent or hidden visibility differs",
                ));
            }
        }
        Ok(owner)
    }
    /// Retire the exact child, parent and module before the outer STA may be released.
    pub(super) fn close(&mut self) -> Result<(), DesktopError> {
        if let Some(engine) = self.engine.as_ref() {
            engine.close()?;
            self.engine = None;
        }
        if let Some(hwnd) = self.hwnd {
            // SAFETY: Exact hidden test-thread owner, retained on failure.
            unsafe { DestroyWindow(hwnd) }
                .map_err(|e| DesktopError::new("console_fixture_destroy", e))?;
            self.hwnd = None;
        }
        if let Some(parent) = self.parent {
            // SAFETY: Original child retired before its exact hidden parent on this STA.
            unsafe { DestroyWindow(parent) }
                .map_err(|cause| DesktopError::new("console_fixture_parent_destroy", cause))?;
            self.parent = None;
        }
        if let Some(module) = self.module {
            // SAFETY: Hidden RichEdit child and parent retired before the class module.
            unsafe { FreeLibrary(module) }
                .map_err(|e| DesktopError::new("console_fixture_free", e))?;
            self.module = None;
        }
        self.document = None;
        Ok(())
    }
}
impl Drop for Hidden<'_> {
    fn drop(&mut self) {
        if let Err(cause) = self.close() {
            std::mem::forget(self.engine.take());
            std::mem::forget(self.document.take());
            tracing::error!(operation="console_fixture_cleanup",cause=%cause,"Hidden console fixture remains retained");
        }
    }
}

/// Retain one COM-owned MTA usage reference without authoring a thread or scheduler.
pub(super) struct MtaUsage(Option<windows::Win32::System::Com::CO_MTA_USAGE_COOKIE>);
impl MtaUsage {
    /// COM supplies the process MTA and a cookie while leaving this caller uninitialized.
    pub(super) fn new() -> Result<Self, DesktopError> {
        // SAFETY: This test owns the returned cookie and balances it after its STA retires.
        unsafe { windows::Win32::System::Com::CoIncrementMTAUsage() }
            .map(|cookie| Self(Some(cookie)))
            .map_err(|cause| DesktopError::new("ui_apartment_fixture_mta_acquire", cause))
    }
    /// Consume only the exact successfully released cookie; failed cleanup retains it.
    pub(super) fn close(&mut self) -> Result<(), DesktopError> {
        if let Some(cookie) = self.0 {
            // SAFETY: The exact live cookie belongs exclusively to this fixture.
            unsafe { windows::Win32::System::Com::CoDecrementMTAUsage(cookie) }
                .map_err(|cause| DesktopError::new("ui_apartment_fixture_mta_release", cause))?;
            self.0 = None;
        }
        Ok(())
    }
}
impl Drop for MtaUsage {
    /// Preserve unresolved native ownership without attempting cleanup during error or panic unwinding.
    fn drop(&mut self) {
        if self.0.is_some() {
            tracing::error!(
                operation = "ui_apartment_fixture_mta_unresolved",
                "Retaining native MTA usage until process termination"
            );
        }
    }
}
