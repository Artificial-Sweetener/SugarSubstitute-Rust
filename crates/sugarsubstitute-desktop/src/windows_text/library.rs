// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the system text implementation until every field/transcript engine has retired.
use crate::DesktopError;
use std::cell::Cell;
use windows::{
    Win32::{
        Foundation::{FreeLibrary, HMODULE},
        System::LibraryLoader::{LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW},
    },
    core::w,
};

/// An affine native library reference; explicit closure follows checked document retirement.
#[derive(Default)]
pub(crate) struct Library {
    module: Cell<Option<HMODULE>>,
}
impl Library {
    /// Borrow the exact retained system implementation, never a search-path replacement.
    pub(crate) fn module(&self) -> Option<HMODULE> {
        self.module.get()
    }

    /// Load after the containing native owner has adopted this inert lifetime.
    pub(crate) fn load(&self) -> Result<HMODULE, DesktopError> {
        if self.module.get().is_some() {
            return Err(DesktopError::message("text_library_load", "library already loaded"));
        }
        // SAFETY: Search only the OS system directory and retain the returned reference.
        let module =
            unsafe { LoadLibraryExW(w!("Msftedit.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32) }
                .map_err(|cause| DesktopError::new("text_library_load", cause))?;
        self.module.set(Some(module));
        Ok(module)
    }

    /// Release only after the caller has observed all dependent engines and callbacks retired.
    pub(crate) fn close(&self) -> Result<(), DesktopError> {
        if let Some(module) = self.module.get() {
            // SAFETY: The native owner calls only after its dependent document lifetimes retired.
            unsafe { FreeLibrary(module) }
                .map_err(|cause| DesktopError::new("text_library_free", cause))?;
            self.module.set(None);
        }
        Ok(())
    }
}
