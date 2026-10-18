// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire native clipboard/document references and acquire the Windows application color scheme.
use super::Engine;
use crate::DesktopError;
use windows::{
    Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
        System::{
            LibraryLoader::GetProcAddress,
            Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW},
        },
    },
    core::{IUnknown, Interface, s, w},
};

impl Engine {
    /// Qt's Windows style follows the native application color scheme independently of terminal QSS.
    pub(super) fn refresh_system_theme(&self) -> Result<(), DesktopError> {
        let mut light = 1u32;
        let mut length = size_of::<u32>() as u32;
        // SAFETY: Read a bounded DWORD setting without mutating or retaining registry state.
        let result = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
                w!("AppsUseLightTheme"),
                RRF_RT_REG_DWORD,
                None,
                Some((&raw mut light).cast()),
                Some(&mut length),
            )
        };
        if result == ERROR_FILE_NOT_FOUND {
            self.state.system_dark.set(false);
            return Ok(());
        }
        if result != ERROR_SUCCESS {
            return Err(DesktopError::new(
                "console_system_theme",
                windows::core::Error::from(windows::core::HRESULT::from_win32(result.0)),
            ));
        }
        if length != size_of::<u32>() as u32 || light > 1 {
            return Err(DesktopError::message(
                "console_system_theme",
                "invalid native application color scheme",
            ));
        }
        self.state.system_dark.set(light == 0);
        self.state.invalidate();
        self.state.check()
    }
    /// Native TxSetText renders clipboard data owned by this instance before emptying its document.
    /// ShutdownTextServices then consumes exactly the explicitly transferred COM reference and
    /// detaches the host even if a rendered data object retains another native service reference.
    pub(super) fn shutdown_document(&self) -> Result<(), DesktopError> {
        if self.shutdown.get() {
            return Ok(());
        }
        let Some(services) = self.services.borrow().as_ref().cloned() else {
            self.shutdown.set(true);
            return Ok(());
        };
        let module = self.module.get().ok_or_else(|| {
            DesktopError::message("console_shutdown", "system module custody absent")
        })?;
        // SAFETY: The exact retained OS DLL supplies this documented consuming shutdown function.
        let symbol =
            unsafe { GetProcAddress(module, s!("ShutdownTextServices")) }.ok_or_else(|| {
                DesktopError::message("console_shutdown", "system text shutdown export absent")
            })?;
        type Shutdown = unsafe extern "system" fn(*mut std::ffi::c_void) -> windows::core::HRESULT;
        // SAFETY: This exact named export has the documented ShutdownTextServices ABI.
        let shutdown: Shutdown = unsafe { std::mem::transmute(symbol) };
        if self.state.ui_active.get() {
            // SAFETY: Retire only the matching successful native UI activation of this document.
            unsafe { services.OnTxUIDeactivate() }
                .map_err(|cause| DesktopError::new("console_ui_deactivate", cause))?;
            self.state.ui_active.set(false);
        }
        // SAFETY: Only this original instance empties its own document and resolves its delayed copy.
        unsafe { services.TxSetText(w!("")) }
            .map_err(|cause| DesktopError::new("console_clipboard_render", cause))?;
        // SAFETY: This original affine native document remains retained through deactivation.
        unsafe { services.OnTxInPlaceDeactivate() }
            .map_err(|cause| DesktopError::new("console_deactivate", cause))?;
        let reference: IUnknown = services
            .cast()
            .map_err(|cause| DesktopError::new("console_shutdown_reference", cause))?;
        // SAFETY: ShutdownTextServices consumes this newly owned IUnknown reference exactly once.
        unsafe { shutdown(reference.into_raw()) }
            .ok()
            .map_err(|cause| DesktopError::new("console_shutdown", cause))?;
        self.shutdown.set(true);
        Ok(())
    }
}
