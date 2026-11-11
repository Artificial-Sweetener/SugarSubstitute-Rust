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
            Common::COMDLG_FILTERSPEC, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_OVERWRITEPROMPT,
            FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, FileOpenDialog, FileSaveDialog, IFileDialog,
            IFileOpenDialog, IFileSaveDialog, SIGDN_FILESYSPATH,
        },
    },
    core::{HRESULT, HSTRING, Interface, PCWSTR},
};

/// Native shell selection semantics; storage owners validate the selected original later.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionKind {
    /// Choose an existing directory.
    Folder,
    /// Choose an existing regular filesystem file.
    File,
    /// Choose where to save a text file; the shell confirms replacing an existing file.
    Save {
        /// Localized dialog title.
        title: &'static str,
        /// Suggested file name.
        file_name: &'static str,
        /// Localized file type label, such as "Text report (*.txt)".
        filter: &'static str,
    },
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
    // modal operation. Every COM object is released before returning to that loop; the
    // borrowed title, name and filter strings outlive the synchronous configuration calls.
    unsafe {
        let dialog: IFileDialog = match kind {
            SelectionKind::Save { title, file_name, filter } => {
                let dialog: IFileSaveDialog =
                    CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)?;
                let (title, file_name, filter) =
                    (HSTRING::from(title), HSTRING::from(file_name), HSTRING::from(filter));
                let pattern = HSTRING::from("*.txt");
                dialog.SetFileTypes(&[COMDLG_FILTERSPEC {
                    pszName: PCWSTR(filter.as_ptr()),
                    pszSpec: PCWSTR(pattern.as_ptr()),
                }])?;
                dialog.SetDefaultExtension(&HSTRING::from("txt"))?;
                dialog.SetTitle(&title)?;
                dialog.SetFileName(&file_name)?;
                dialog.SetOptions(FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | FOS_OVERWRITEPROMPT)?;
                dialog.cast()?
            }
            open => {
                let dialog: IFileOpenDialog =
                    CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
                let flags =
                    if open == SelectionKind::Folder { FOS_PICKFOLDERS } else { FOS_FILEMUSTEXIST };
                dialog.SetOptions(FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | flags)?;
                dialog.cast()?
            }
        };
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
pub(super) struct ActiveDialog(RefCell<Option<IFileDialog>>);
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
