// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Queue shell selection between application callbacks on the desktop's existing UI STA.
#[cfg(windows)]
mod picker;
#[cfg(windows)]
pub(crate) use picker::Picker;
#[cfg(windows)]
use std::{
    cell::RefCell, error::Error, ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf,
};
#[cfg(windows)]
use windows::{
    Win32::{
        Foundation::{ERROR_CANCELLED, HWND},
        System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree},
        UI::Shell::{
            FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS,
            FileOpenDialog, IFileOpenDialog, SIGDN_FILESYSPATH,
        },
    },
    core::HRESULT,
};

/// Native shell selection semantics; storage owners validate the selected original later.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionKind {
    /// Choose an existing directory.
    Folder,
    /// Choose an existing regular filesystem file.
    File,
}

#[cfg(windows)]
/// Borrow the owning UI STA/window only from the picker's installed pre-dispatch hook.
/// Native selected path units are preserved; storage and interface text consumers validate them.
/// # Errors
/// Preserves COM/shell failures; user cancellation returns `None`.
fn choose(
    window: HWND,
    kind: SelectionKind,
    active: &ActiveDialog,
) -> Result<Option<PathBuf>, Box<dyn Error>> {
    // SAFETY: The caller retains its UI STA, parent HWND and event loop across this
    // modal operation. Every COM object is released before returning to that loop.
    unsafe {
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
        let kind = match kind {
            SelectionKind::File => FOS_FILEMUSTEXIST,
            SelectionKind::Folder => FOS_PICKFOLDERS,
        };
        dialog.SetOptions(FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | kind)?;
        *active.0.borrow_mut() = Some(dialog.clone());
        let shown = dialog.Show(Some(window));
        active.0.borrow_mut().take();
        match shown {
            Ok(()) => {}
            Err(cause) if cause.code() == HRESULT::from_win32(ERROR_CANCELLED.0) => {
                return Ok(None);
            }
            Err(cause) => return Err(cause.into()),
        }
        let selected = dialog.GetResult()?;
        let pointer = selected.GetDisplayName(SIGDN_FILESYSPATH)?;
        let result = PathBuf::from(OsString::from_wide(pointer.as_wide()));
        CoTaskMemFree(Some(pointer.0.cast()));
        Ok(Some(result))
    }
}

/// UI-thread-only cancellation access, independent of the application's mutable event handler.
#[cfg(windows)]
#[derive(Default)]
pub(super) struct ActiveDialog(RefCell<Option<IFileOpenDialog>>);
#[cfg(windows)]
impl ActiveDialog {
    /// Close the retained shell receiver without holding a RefCell borrow across native reentry.
    fn cancel(&self) -> Result<(), crate::DesktopError> {
        let dialog = self.0.borrow().clone();
        if let Some(dialog) = dialog {
            // SAFETY: The original UI STA retains both references through the modal call.
            unsafe { dialog.Close(HRESULT::from_win32(ERROR_CANCELLED.0)) }
                .map_err(|cause| crate::DesktopError::new("cancel_native_picker", cause))?;
        }
        Ok(())
    }
}

/// Typed same-STA shell boundary; replacement is exposed only to hidden native tests.
#[cfg(windows)]
pub(super) type Selection =
    Box<dyn Fn(HWND, SelectionKind, &ActiveDialog) -> Result<Option<PathBuf>, Box<dyn Error>>>;
