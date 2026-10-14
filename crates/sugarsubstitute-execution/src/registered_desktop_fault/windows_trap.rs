// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Authored Windows x64 breakpoint on the actual qualification worker only.
//! The private armed borrow retains original capture custody through every call.
//! Thread-local error mode suppresses critical-error, WER and OpenFile dialogs;
//! unrelated processes, threads and persistent Windows preferences are untouched.
use super::{RegisteredDesktopFaultError, effect::Armed};
use crate::native::NativeObservationError;
use windows::Win32::System::Diagnostics::Debug::{
    GetThreadErrorMode, SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX, SEM_NOOPENFILEERRORBOX,
    SetThreadErrorMode, THREAD_ERROR_MODE,
};

/// All native effects follow checks outside Core/cleanup/stage locks on the same worker.
pub(super) fn run(armed: &Armed<'_>) -> Result<(), RegisteredDesktopFaultError> {
    armed.check()?;
    // SAFETY: These APIs touch only the calling thread's scalar error mode. The
    // initialized previous scalar remains writable through this synchronous call.
    let current = THREAD_ERROR_MODE(unsafe { GetThreadErrorMode() });
    let mut previous = THREAD_ERROR_MODE::default();
    // SAFETY: The same-thread initialized scalar is writable only during this call.
    unsafe {
        SetThreadErrorMode(
            current | SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX | SEM_NOOPENFILEERRORBOX,
            Some(&mut previous),
        )
    }
    .map_err(|cause| {
        RegisteredDesktopFaultError::Native(NativeObservationError::new(
            "qualification_thread_error_mode",
            cause,
        ))
    })?;
    let result = armed.check();
    if result.is_ok() {
        k1_registered_desktop_trap(armed);
    }
    // SAFETY: Restore this same thread's exact previous scalar after the accepted
    // finite call returns. This is closure of the accepted effect, not fresh work.
    let restored = unsafe { SetThreadErrorMode(previous, None) };
    let first = result.err().unwrap_or(RegisteredDesktopFaultError::Returned);
    match restored {
        Ok(()) => Err(first),
        Err(cause) => Err(RegisteredDesktopFaultError::Restoration {
            first: Box::new(first),
            cause: NativeObservationError::new("qualification_thread_mode_restore", cause),
        }),
    }
}

/// The selected Host image/PDB contains this named, non-inlined exception instruction.
#[inline(never)]
fn k1_registered_desktop_trap(_armed: &Armed<'_>) {
    // SAFETY: The constructor-private seal exists only on the actual admitted
    // Windows x64 worker after native authentication and registered capture Ready.
    // INT3 raises a synchronous breakpoint exception without modifying Rust-owned
    // memory or retaining pointers. A debugger may resume, which is explicitly
    // reported as failure by the caller; no noreturn/nomem claim is made.
    unsafe {
        core::arch::asm!("int3", options(nostack));
    }
}
