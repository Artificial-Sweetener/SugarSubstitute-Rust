// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exact v1 ABI; a process-lifetime pin precedes every possible callback installation.
use crate::{CrashError, capture::lock, native::CALLBACK_PIN_BYTES};
use std::sync::Mutex;
use sugarsubstitute_contracts::{crash::CaptureOperation, execution::MemoryClaim};
use windows::{
    Win32::{
        Foundation::{FreeLibrary, HMODULE},
        System::LibraryLoader::*,
    },
    core::{PCSTR, PCWSTR},
};

#[repr(C)]
pub(super) struct Input {
    pub(super) handler: *const u16,
    pub(super) handler_length: usize,
    pub(super) database: *const u16,
    pub(super) database_length: usize,
    pub(super) metrics: *const u16,
    pub(super) metrics_length: usize,
    pub(super) build: *const u8,
    pub(super) build_length: usize,
    pub(super) coordinator: *const u8,
    pub(super) coordinator_length: usize,
    pub(super) deadline_counter: i64,
    pub(super) counter_frequency: i64,
}
#[repr(C)]
#[derive(Default)]
pub(super) struct ResultValue {
    pub(super) version: u32,
    pub(super) registered: u32,
    pub(super) error: u32,
    pub(super) cleanup_error: u32,
    pub(super) pending_io: u32,
    pub(super) retired: u32,
    pub(super) process: usize,
    pub(super) thread: usize,
    pub(super) cleanup_count: u32,
    pub(super) cleanup_codes: [u32; 8],
    pub(super) omitted_cleanup: u64,
}
type Start = unsafe extern "C" fn(*const Input, *mut ResultValue);
type Retire = unsafe extern "C" fn(*mut ResultValue);
#[derive(Clone, Copy)]
pub(super) struct Api {
    start: Start,
    retire: Retire,
}
struct Pin {
    _module: usize,
    _memory: MemoryClaim,
}
// Exactly one native callback context per process; never cloned/exported.
static PIN: Mutex<Option<Pin>> = Mutex::new(None);

pub(super) fn load(
    path: &[u16],
    memory: MemoryClaim,
) -> Result<Api, (CrashError, Option<MemoryClaim>)> {
    let mut pinned = lock(&PIN);
    if pinned.is_some() {
        return Err((CrashError::invalid(CaptureOperation::Runtime), Some(memory)));
    }
    // SAFETY: Absolute pinned NUL path lives through loading; dependencies resolve
    // exclusively in System32, not the current directory or ambient PATH.
    let module = match unsafe {
        LoadLibraryExW(PCWSTR(path.as_ptr()), None, LOAD_LIBRARY_SEARCH_SYSTEM32)
    } {
        Ok(module) => module,
        Err(cause) => {
            return Err((CrashError::new(CaptureOperation::Runtime, cause), Some(memory)));
        }
    };
    // Pin before symbol lookup/entry: even a failed StartHandler can install callbacks.
    let mut retained = HMODULE::default();
    // SAFETY: The exact just-loaded absolute module is retained while Windows pins it.
    let result = unsafe {
        GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_PIN, PCWSTR(path.as_ptr()), &mut retained)
    };
    // A partial nonnull PIN output is retained before status interpretation.
    // It is never an unload proof, even if the API reports failure.
    if !retained.0.is_null() {
        *pinned = Some(Pin { _module: module.0 as usize, _memory: memory });
        if let Err(cause) = result {
            return Err((CrashError::new(CaptureOperation::Runtime, cause), None));
        }
        if retained != module {
            return Err((CrashError::invalid(CaptureOperation::Runtime), None));
        }
    } else if result.is_ok() {
        *pinned = Some(Pin { _module: module.0 as usize, _memory: memory });
        return Err((CrashError::invalid(CaptureOperation::Runtime), None));
    } else if let Err(cause) = result {
        // Failed checked unload retains the actual module and its storage charge.
        // SAFETY: This is our owned LoadLibrary reference, before callback entry.
        if let Err(cleanup) = unsafe { FreeLibrary(module) } {
            *pinned = Some(Pin { _module: module.0 as usize, _memory: memory });
            return Err((
                CrashError::new(CaptureOperation::Runtime, cause).with_secondary(cleanup),
                None,
            ));
        }
        return Err((CrashError::new(CaptureOperation::Runtime, cause), Some(memory)));
    }
    // SAFETY: The process-pinned module owns both fixed C ABI exports permanently.
    let start = unsafe { GetProcAddress(module, PCSTR(c"ss_capture_start_v1".as_ptr().cast())) };
    let start_error = start.is_none().then(windows::core::Error::from_thread);
    // SAFETY: The process-pinned module owns both fixed C ABI exports permanently.
    let retire =
        unsafe { GetProcAddress(module, PCSTR(c"ss_capture_retire_io_v1".as_ptr().cast())) };
    let retire_error = retire.is_none().then(windows::core::Error::from_thread);
    if let Some(cause) = start_error {
        let mut failure = CrashError::new(CaptureOperation::Runtime, cause);
        if let Some(secondary) = retire_error {
            failure = failure.with_secondary(secondary);
        }
        return Err((failure, None));
    }
    if let Some(cause) = retire_error {
        return Err((CrashError::new(CaptureOperation::Runtime, cause), None));
    }
    let (Some(start), Some(retire)) = (start, retire) else {
        return Err((CrashError::invalid(CaptureOperation::Runtime), None));
    };
    // SAFETY: Hash-verified first-party bridge exports exactly these repr(C) signatures.
    Ok(unsafe {
        Api {
            start: std::mem::transmute::<unsafe extern "system" fn() -> isize, Start>(start),
            retire: std::mem::transmute::<unsafe extern "system" fn() -> isize, Retire>(retire),
        }
    })
}
impl Api {
    pub(super) fn start(self, input: &Input) -> ResultValue {
        let mut output = ResultValue::default();
        // SAFETY: Borrowed arrays remain valid for this finite synchronous call;
        // initialized output transfers all handles before its status is interpreted.
        unsafe { (self.start)(input, &mut output) };
        output
    }
    pub(super) fn retire(self) -> ResultValue {
        let mut output = ResultValue::default();
        // SAFETY: Module/context remain process-pinned; completion storage never moves.
        unsafe { (self.retire)(&mut output) };
        output
    }
}
pub(super) fn pinned_bytes() -> u64 {
    if lock(&PIN).is_some() { CALLBACK_PIN_BYTES as u64 } else { 0 }
}
pub(super) fn pinned_reservation_bytes() -> u64 {
    lock(&PIN).as_ref().map_or(0, |pin| pin._memory.reserved_bytes())
}
