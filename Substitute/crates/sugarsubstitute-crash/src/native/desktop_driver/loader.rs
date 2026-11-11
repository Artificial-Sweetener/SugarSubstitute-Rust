// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt both module outputs before interpreting each finite loader status.
use super::{failure, invalid};
use crate::native::module_pin::Fields;
use std::os::windows::ffi::OsStrExt;
use sugarsubstitute_execution::windows::NativeObservationError;
use sugarsubstitute_execution::{
    NativeCaptureInput, NativeCaptureProgress, NativeCaptureReply, NativeCaptureStep,
};
use windows::{
    Win32::{
        Foundation::{FreeLibrary, HMODULE},
        System::LibraryLoader::*,
    },
    core::{PCSTR, PCWSTR},
};
type Attach = unsafe extern "C" fn(*const NativeCaptureInput, *mut NativeCaptureReply);
type Progress = unsafe extern "C" fn(*const NativeCaptureProgress, *mut NativeCaptureReply);
type Step = unsafe extern "C" fn(*const NativeCaptureStep, *mut NativeCaptureReply);
/// Function pointers remain backed by the same permanently retained verified module.
#[derive(Clone, Copy)]
pub(in crate::native) struct Api {
    pub(super) attach: Attach,
    pub(super) poll: Progress,
    pub(super) step: Step,
    pub(super) retire: Progress,
}

/// Selectors and module cells were installed before this finite method is entered.
/// No Rust/C++ owner mutex is held across filesystem, loader, provider or symbol work.
pub(super) fn load(fields: &mut Fields) -> Result<(), NativeObservationError> {
    let parts = fields.parts.as_mut().ok_or_else(|| invalid("capture_loader_parts"))?;
    let path = parts.retention.client_path().map_err(|e| failure("capture_loader_path", e))?;
    let length = path.as_os_str().encode_wide().count();
    if length == 0 || length >= 32_768 {
        return Err(invalid("capture_loader_path_bound"));
    }
    let mut wide = Vec::with_capacity(length + 1);
    if wide.capacity() != length + 1 {
        return Err(invalid("capture_loader_path_capacity"));
    }
    wide.extend(path.as_os_str().encode_wide());
    wide.push(0);
    if wide[..length].contains(&0) {
        return Err(invalid("capture_loader_path_nul"));
    }
    parts.loaded.check_effect().map_err(|e| failure("capture_loader_current", e))?;
    // SAFETY: canonical absolute no-write/no-delete pinned name lives through this call;
    // dependency search is restricted to System32, outside every owner lock.
    let module =
        unsafe { LoadLibraryExW(PCWSTR(wide.as_ptr()), None, LOAD_LIBRARY_SEARCH_SYSTEM32) };
    match module {
        Ok(module) => fields.loaded = module.0 as usize,
        Err(cause) => {
            return Err(failure(
                "capture_loader_load",
                parts.loaded.native_failure("capture_module_load", cause),
            ));
        }
    }
    parts.pinned.check_effect().map_err(|e| failure("capture_loader_pin_current", e))?;
    fields.pin_uncertain = true;
    let mut pinned = HMODULE::default();
    // SAFETY: the identical canonical pinned name and actual loaded reference remain live.
    // PIN acceptance may create process-lifetime effects even on an error return.
    let status = unsafe {
        GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_PIN, PCWSTR(wide.as_ptr()), &mut pinned)
    };
    fields.pinned = pinned.0 as usize;
    if let Err(cause) = status {
        return Err(failure(
            "capture_loader_pin",
            parts.pinned.native_failure("capture_module_pin", cause),
        ));
    }
    if fields.pinned == 0 || fields.pinned != fields.loaded {
        return Err(invalid("capture_loader_pin_identity"));
    }
    fields.pin_uncertain = false;
    let module = HMODULE(fields.loaded as *mut _);
    let mut symbol=|name:&'static std::ffi::CStr|->Result<unsafe extern "system" fn()->isize,NativeObservationError> {
        parts.retention.check_effect().map_err(|e|failure("capture_loader_symbol_current",e))?;
        // SAFETY: the fixed export name belongs to the reviewed process-pinned DLL.
        let value=unsafe {GetProcAddress(module,PCSTR(name.as_ptr().cast()))};
        value.ok_or_else(||failure("capture_loader_symbol",parts.loaded.native_failure("capture_module_symbol",windows::core::Error::from_thread())))
    };
    let attach = symbol(c"ss_capture_attach_v2")?;
    let poll = symbol(c"ss_capture_attach_poll_v2")?;
    let step = symbol(c"ss_capture_attach_step_v2")?;
    let retire = symbol(c"ss_capture_attach_retire_v2")?;
    // SAFETY: the authenticated reviewed first-party DLL exposes exactly these
    // repr(C) signatures; Execution owns all synchronous input/callback/output lifetimes.
    fields.api = Some(unsafe {
        Api {
            attach: std::mem::transmute::<unsafe extern "system" fn() -> isize, Attach>(attach),
            poll: std::mem::transmute::<unsafe extern "system" fn() -> isize, Progress>(poll),
            step: std::mem::transmute::<unsafe extern "system" fn() -> isize, Step>(step),
            retire: std::mem::transmute::<unsafe extern "system" fn() -> isize, Progress>(retire),
        }
    });
    Ok(())
}

/// Close only the exact pre-PIN LoadLibrary reference using its original cleanup proof.
/// A late successful accepted return clears that cell before any next provider/gate.
pub(super) fn close_loaded(
    fields: &mut Fields,
    proof: &sugarsubstitute_execution::CaptureBundleRetirement,
) -> Result<(), NativeObservationError> {
    if fields.loaded == 0 {
        return Ok(());
    }
    if fields.pinned != 0 || fields.pin_uncertain {
        return Err(invalid("capture_module_permanent"));
    }
    proof.check().map_err(|e| failure("capture_module_close_current", e))?;
    let module = HMODULE(fields.loaded as *mut _);
    // SAFETY: this exact non-pinned LoadLibrary reference is owned in the retained cell.
    // No owner/provider lock crosses the finite call; failure retains its original output.
    match unsafe { FreeLibrary(module) } {
        Ok(()) => {
            fields.loaded = 0;
            Ok(())
        }
        Err(cause) => {
            let parts =
                fields.parts.as_mut().ok_or_else(|| invalid("capture_module_close_parts"))?;
            Err(failure(
                "capture_module_close",
                parts.loaded.native_failure("capture_module_close", cause),
            ))
        }
    }
}
