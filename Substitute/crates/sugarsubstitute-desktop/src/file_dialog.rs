// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Queue shell selection between application callbacks on the desktop's existing UI STA.
#[cfg(windows)]
mod export_folder;
#[cfg(windows)]
mod picker;
#[cfg(windows)]
pub(crate) use picker::Picker;
use std::path::PathBuf;
#[cfg(windows)]
use std::{cell::RefCell, error::Error, ffi::OsString, os::windows::ffi::OsStringExt};
#[cfg(windows)]
use windows::{
    Win32::{
        Foundation::{ERROR_CANCELLED, HWND},
        System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree},
        UI::Shell::{
            Common::COMDLG_FILTERSPEC, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_OVERWRITEPROMPT,
            FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, FileOpenDialog, FileSaveDialog, IFileDialog,
            IFileOpenDialog, IFileSaveDialog, IShellItem, SHCreateItemFromParsingName,
            SIGDN_FILESYSPATH,
        },
    },
    core::{GUID, HRESULT, HSTRING, Interface, PCWSTR},
};

/// Save dialogs remember their own last folder, apart from every other dialog of the process.
#[cfg(windows)]
const SAVE_DIALOG: GUID = GUID::from_u128(0x5b0e_8d3c_7a41_4c55_9e1f_2d6b_8a4c_1e70);

/// Native shell selection semantics; storage owners validate the selected original later.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionKind {
    /// Choose an existing directory.
    Folder,
    /// Choose an existing regular filesystem file.
    File,
    /// Choose where to save a file; the shell confirms replacing an existing file.
    Save {
        /// Localized dialog title.
        title: &'static str,
        /// Suggested file name; the dialog fills its product and date placeholders (see
        /// `sugarsubstitute_contracts::localization::incident::file_name`).
        file_name: &'static str,
        /// The product the saved file is about, with the version of its update when known.
        product: String,
        /// Localized file type label, such as "Text report (*.txt)".
        filter: &'static str,
        /// File extension without its dot, such as "txt".
        extension: &'static str,
    },
    /// Choose one existing file of one type, starting in a folder the caller names.
    OpenFrom {
        /// The existing folder the dialog starts in.
        folder: PathBuf,
        /// File extension without its dot.
        extension: String,
    },
    /// Choose where to save a file of one type, starting in a folder the caller names; the
    /// shell confirms replacing an existing file.
    SaveInto {
        /// The existing folder the dialog starts in.
        folder: PathBuf,
        /// Suggested file name; may be empty.
        file_name: String,
        /// File extension without its dot.
        extension: String,
    },
}

#[cfg(windows)]
/// The date and time in the user's time zone, for a suggested file name.
fn now() -> sugarsubstitute_contracts::localization::incident::LocalDateTime {
    // SAFETY: a plain query of the local clock with no arguments.
    let now = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    sugarsubstitute_contracts::localization::incident::LocalDateTime {
        year: now.wYear,
        month: u8::try_from(now.wMonth).unwrap_or(1),
        day: u8::try_from(now.wDay).unwrap_or(1),
        hour: u8::try_from(now.wHour).unwrap_or(0),
        minute: u8::try_from(now.wMinute).unwrap_or(0),
    }
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
            SelectionKind::Save { title, file_name, product, filter, extension } => {
                let dialog: IFileSaveDialog =
                    CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)?;
                let file_name = sugarsubstitute_contracts::localization::incident::file_name(
                    file_name,
                    &product,
                    now(),
                );
                let (title, file_name, filter) =
                    (HSTRING::from(title), HSTRING::from(file_name), HSTRING::from(filter));
                let pattern = HSTRING::from(format!("*.{extension}"));
                dialog.SetFileTypes(&[COMDLG_FILTERSPEC {
                    pszName: PCWSTR(filter.as_ptr()),
                    pszSpec: PCWSTR(pattern.as_ptr()),
                }])?;
                dialog.SetDefaultExtension(&HSTRING::from(extension))?;
                dialog.SetTitle(&title)?;
                dialog.SetFileName(&file_name)?;
                dialog.SetOptions(FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | FOS_OVERWRITEPROMPT)?;
                dialog.SetClientGuid(&SAVE_DIALOG)?;
                let remembered = dialog.GetFolder().ok().and_then(|folder| path_of(&folder).ok());
                if let Some(folder) = export_folder::resolve(remembered) {
                    let folder: IShellItem =
                        SHCreateItemFromParsingName(&HSTRING::from(folder.as_os_str()), None)?;
                    dialog.SetFolder(&folder)?;
                }
                dialog.cast()?
            }
            SelectionKind::OpenFrom { folder, extension } => {
                let dialog: IFileOpenDialog =
                    CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
                start_in(&dialog.cast()?, &folder, &extension)?;
                dialog.SetOptions(FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | FOS_FILEMUSTEXIST)?;
                dialog.cast()?
            }
            SelectionKind::SaveInto { folder, file_name, extension } => {
                let dialog: IFileSaveDialog =
                    CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)?;
                start_in(&dialog.cast()?, &folder, &extension)?;
                dialog.SetFileName(&HSTRING::from(file_name))?;
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
        Ok(Some(path_of(&dialog.GetResult()?)?))
    }
}

/// Offer one file type in `dialog` and start it in `folder`, whatever folder it showed last.
#[cfg(windows)]
fn start_in(
    dialog: &IFileDialog,
    folder: &std::path::Path,
    extension: &str,
) -> windows::core::Result<()> {
    let pattern = HSTRING::from(format!("*.{extension}"));
    // SAFETY: The dialog is live on this STA; the borrowed strings outlive these synchronous
    // configuration calls, and the folder item is released when it leaves scope.
    unsafe {
        dialog.SetFileTypes(&[COMDLG_FILTERSPEC {
            pszName: PCWSTR(pattern.as_ptr()),
            pszSpec: PCWSTR(pattern.as_ptr()),
        }])?;
        dialog.SetDefaultExtension(&HSTRING::from(extension))?;
        let folder: IShellItem =
            SHCreateItemFromParsingName(&HSTRING::from(folder.as_os_str()), None)?;
        dialog.SetFolder(&folder)
    }
}

/// The file-system path of a shell item.
#[cfg(windows)]
fn path_of(item: &IShellItem) -> windows::core::Result<PathBuf> {
    // SAFETY: The item is live on this STA; the returned string is copied, then freed once.
    unsafe {
        let pointer = item.GetDisplayName(SIGDN_FILESYSPATH)?;
        let path = PathBuf::from(OsString::from_wide(pointer.as_wide()));
        CoTaskMemFree(Some(pointer.0.cast()));
        Ok(path)
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
