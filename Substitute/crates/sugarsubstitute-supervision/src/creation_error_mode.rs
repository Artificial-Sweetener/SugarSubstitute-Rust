// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Make Windows report an unrunnable image as a creation error instead of a blocking dialog.
//!
//! When `CreateProcessW` is handed an image the loader cannot run (a damaged, truncated or
//! non-PE file), Windows raises a modal hard-error dialog such as "Unsupported 16-Bit
//! Application" on the creating thread's desktop, and the call does not return until a person
//! dismisses it. An admitted creation job would then wait on a window nobody may be able to see.
//! `SEM_FAILCRITICALERRORS` on the calling thread makes the same call fail with the native
//! error code instead, which each creation adapter already carries as its typed creation error.
//!
//! The mode is scoped to the creation call on the calling thread only: the process-wide mode,
//! other threads and persistent Windows preferences are untouched, and the exact previous thread
//! mode returns when the call completes or unwinds.
//! See [SetThreadErrorMode](https://learn.microsoft.com/en-us/windows/win32/api/errhandlingapi/nf-errhandlingapi-setthreaderrormode).
use windows::Win32::System::Diagnostics::Debug::{
    GetThreadErrorMode, SEM_FAILCRITICALERRORS, SetThreadErrorMode, THREAD_ERROR_MODE,
};

#[cfg(test)]
mod tests;

/// Run one native creation call with unrunnable images reported as errors on this thread.
///
/// `create` runs on the calling thread and must not hand work to another thread, because the
/// error mode is per thread. The previous thread mode is restored when `create` returns or
/// unwinds.
///
/// # Errors
/// Returns the native failure to enter the mode. `create` has not run in that case, so no
/// process was created and no dialog can have been raised.
pub(crate) fn with_image_errors_reported<T>(
    create: impl FnOnce() -> T,
) -> Result<T, windows::core::Error> {
    let _scope = ReportedImageErrors::enter()?;
    Ok(create())
}

/// Read the calling thread's error mode so tests can prove a creation left it unchanged.
#[cfg(test)]
pub(crate) fn thread_mode() -> THREAD_ERROR_MODE {
    // SAFETY: The query reads only the calling thread's scalar error mode.
    THREAD_ERROR_MODE(unsafe { GetThreadErrorMode() })
}

/// Whether a creation failure is one of the loader's image-format errors, which it reports
/// differently depending on how far into the file it gets before refusing it.
#[cfg(test)]
pub(crate) fn is_image_format_error(code: Option<u32>) -> bool {
    use windows::Win32::Foundation::{
        ERROR_BAD_EXE_FORMAT, ERROR_EXE_MACHINE_TYPE_MISMATCH, ERROR_INVALID_EXE_SIGNATURE,
    };
    [ERROR_BAD_EXE_FORMAT, ERROR_EXE_MACHINE_TYPE_MISMATCH, ERROR_INVALID_EXE_SIGNATURE]
        .iter()
        .any(|expected| code == Some(expected.0))
}

/// Restores the exact thread error mode that preceded `enter`.
struct ReportedImageErrors {
    previous: THREAD_ERROR_MODE,
}

impl ReportedImageErrors {
    fn enter() -> Result<Self, windows::core::Error> {
        // SAFETY: The query reads only the calling thread's scalar error mode.
        let current = THREAD_ERROR_MODE(unsafe { GetThreadErrorMode() });
        let mut previous = THREAD_ERROR_MODE::default();
        // SAFETY: The call changes only the calling thread's scalar error mode, and the
        // initialized `previous` scalar stays writable through this synchronous call.
        unsafe { SetThreadErrorMode(current | SEM_FAILCRITICALERRORS, Some(&mut previous)) }?;
        Ok(Self { previous })
    }
}

impl Drop for ReportedImageErrors {
    fn drop(&mut self) {
        // SAFETY: The call changes only the calling thread's scalar error mode. The guard lives
        // on one stack frame of `with_image_errors_reported`, so this is the thread it saved.
        if let Err(cause) = unsafe { SetThreadErrorMode(self.previous, None) } {
            // Restoration cannot be reported to a creation that has already finished and may
            // have produced a process. The thread keeps the stricter mode, which only
            // suppresses dialogs, and the failure stays visible.
            tracing::error!(
                operation = "creation_error_mode_restore",
                cause = %cause,
                "Thread error mode kept after creation"
            );
        }
    }
}
