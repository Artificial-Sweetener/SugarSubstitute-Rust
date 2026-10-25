// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep shared shell file selection on the desktop's existing UI STA.
#[cfg(windows)]
use std::{error::Error, ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf};
#[cfg(windows)]
use tao::{platform::windows::WindowExtWindows, window::Window};
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
/// Borrow the owning UI STA/window until modal selection or cancellation completes.
/// Native selected path units are preserved; storage and interface text consumers validate them.
/// # Errors
/// Preserves COM/shell failures; user cancellation returns `None`.
pub(crate) fn choose(
    window: &Window,
    kind: SelectionKind,
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
        match dialog.Show(Some(HWND(window.hwnd() as *mut _))) {
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
