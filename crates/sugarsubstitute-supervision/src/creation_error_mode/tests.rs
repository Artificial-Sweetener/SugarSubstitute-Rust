// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The creation scope reports image errors only for its own call and restores the exact thread mode.
use super::*;
use crate::setup_storage::test_worker::{self as worker, TestResult};
use windows::Win32::System::Diagnostics::Debug::{SEM_NOGPFAULTERRORBOX, SEM_NOOPENFILEERRORBOX};

/// Replace the calling thread's error mode and return a guard that puts the original back, so a
/// test never leaves its shared worker thread in a mode it chose.
struct ThreadModeSandbox {
    original: THREAD_ERROR_MODE,
}
impl ThreadModeSandbox {
    fn enter(mode: THREAD_ERROR_MODE) -> Result<Self, windows::core::Error> {
        let original = thread_mode();
        // SAFETY: The call changes only the calling thread's scalar error mode.
        unsafe { SetThreadErrorMode(mode, None) }?;
        Ok(Self { original })
    }
}
impl Drop for ThreadModeSandbox {
    fn drop(&mut self) {
        // SAFETY: The call changes only the calling thread's scalar error mode.
        let restored = unsafe { SetThreadErrorMode(self.original, None) };
        assert!(restored.is_ok(), "Test worker thread mode was not restored");
    }
}

#[test]
fn the_scope_adds_critical_error_reporting_and_restores_every_previous_mode() -> TestResult {
    worker::run(|_, _| {
        for previous in [
            THREAD_ERROR_MODE(0),
            SEM_NOGPFAULTERRORBOX,
            SEM_FAILCRITICALERRORS,
            SEM_FAILCRITICALERRORS | SEM_NOOPENFILEERRORBOX,
        ] {
            let _sandbox = ThreadModeSandbox::enter(previous)?;
            let inside = with_image_errors_reported(thread_mode)?;
            assert_eq!(inside, previous | SEM_FAILCRITICALERRORS);
            assert_eq!(thread_mode(), previous);
        }
        Ok(())
    })
}

#[test]
fn the_creation_result_passes_through_unchanged() -> TestResult {
    worker::run(|_, _| {
        let _sandbox = ThreadModeSandbox::enter(THREAD_ERROR_MODE(0))?;
        let value = with_image_errors_reported(|| Err::<u32, &str>("native refusal"))?;
        assert_eq!(value, Err("native refusal"));
        assert_eq!(thread_mode(), THREAD_ERROR_MODE(0));
        Ok(())
    })
}

#[test]
fn an_unwinding_creation_restores_the_previous_mode() -> TestResult {
    worker::run(|_, _| {
        let _sandbox = ThreadModeSandbox::enter(SEM_NOGPFAULTERRORBOX)?;
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_image_errors_reported(|| {
                assert_eq!(thread_mode(), SEM_NOGPFAULTERRORBOX | SEM_FAILCRITICALERRORS);
                std::panic::resume_unwind(Box::new("fixture creation unwind"))
            })
        }));
        assert!(unwind.is_err());
        assert_eq!(thread_mode(), SEM_NOGPFAULTERRORBOX);
        Ok(())
    })
}
