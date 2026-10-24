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
    core::{HRESULT, PCWSTR},
};

pub(super) fn choose(
    window: &Window,
    field: &str,
    copy: &sugarsubstitute_contracts::localization::installer::InstallerMessages,
) -> Result<Option<String>, Box<dyn Error>> {
    if !matches!(field, "installRoot" | "comfyRoot" | "models" | "outputs" | "python") {
        return Err("Unsupported installer path picker".into());
    }
    // SAFETY: The caller retains its UI STA, parent HWND and event loop across this
    // modal operation. Every COM object is released before returning to that loop.
    unsafe {
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
        let kind = if field == "python" { FOS_FILEMUSTEXIST } else { FOS_PICKFOLDERS };
        let title = match field {
            "installRoot" => copy.install_folder,
            "comfyRoot" => copy.comfy_folder,
            "models" => copy.models,
            "outputs" => copy.outputs,
            _ => copy.python,
        }
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
        let select = copy.select.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        dialog.SetTitle(PCWSTR(title.as_ptr()))?;
        dialog.SetOkButtonLabel(PCWSTR(select.as_ptr()))?;
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
