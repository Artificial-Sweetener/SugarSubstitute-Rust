// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the shell's modal folder/file picker on the installer's existing UI STA.
use std::error::Error;
use tao::{platform::windows::WindowExtWindows, window::Window};
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

pub(crate) fn choose(
    window: &Window,
    kind: crate::installer::native::PickerKind,
) -> Result<Option<String>, Box<dyn Error>> {
    // SAFETY: The caller retains its UI STA, parent HWND and event loop across this
    // modal operation. Every COM object is released before returning to that loop.
    unsafe {
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
        let kind = match kind {
            crate::installer::native::PickerKind::File => FOS_FILEMUSTEXIST,
            crate::installer::native::PickerKind::Folder => FOS_PICKFOLDERS,
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
        let result = pointer.to_string();
        CoTaskMemFree(Some(pointer.0.cast()));
        Ok(Some(result?))
    }
}
