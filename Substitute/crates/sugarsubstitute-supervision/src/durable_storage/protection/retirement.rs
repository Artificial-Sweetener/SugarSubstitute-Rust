// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe ordinary file closure before publication, retaining failed handles through cleanup.
use super::*;
use std::os::windows::io::IntoRawHandle;

impl Protection {
    /// Observe closure while retaining any failed native handle in this operation's owner.
    /// Cleanup remains mandatory after cancellation; this consumes no new native reservation.
    pub(crate) fn close_file(&mut self, file: File) -> io::Result<()> {
        if self.handles.len() == self.handles.capacity() {
            return Err(super::super::invalid("Publication handle reservation exhausted"));
        }
        let mut handle = Handle::new(HANDLE(file.into_raw_handle()), self.budget.clone());
        let result = handle.close().map_err(io::Error::other);
        if result.is_err() {
            self.handles.push(handle);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup_storage::test_worker::{self as worker, TestResult};
    use std::io::Write;
    use windows::Win32::Foundation::{
        HANDLE_FLAG_PROTECT_FROM_CLOSE, HANDLE_FLAGS, SetHandleInformation,
    };

    /// An actual failed Windows close retains the file until ordinary cleanup observes retirement.
    #[test]
    fn protected_close_failure_retains_native_custody_until_checked_cleanup() -> TestResult {
        worker::run_reserved(8 * 1024 * 1024, |context, _| {
            let root = tempfile::tempdir()?;
            let path = root.path().join("retained");
            let failed_close_observed = std::cell::Cell::new(false);
            let outcome = with(context, |protection| {
                let mut file = protection.file(&path, context)?;
                file.write_all(b"preserved")?;
                file.sync_all()?;
                let raw = HANDLE(file.as_raw_handle());
                // SAFETY: The fixture exclusively owns this live handle. This native flag prevents
                // its closure until the same fixture clears it while protection retains ownership.
                unsafe {
                    SetHandleInformation(
                        raw,
                        HANDLE_FLAG_PROTECT_FROM_CLOSE.0,
                        HANDLE_FLAG_PROTECT_FROM_CLOSE,
                    )
                }
                .map_err(io::Error::other)?;
                let Some(cause) = protection.close_file(file).err() else {
                    return Err(io::Error::other("Protected fixture handle unexpectedly closed"));
                };
                let retained = File::open(&path).is_err();
                // SAFETY: Failed close did not invalidate this protected handle; its actual owner
                // remains in the operation. Clearing only this flag enables mandatory checked cleanup.
                unsafe {
                    SetHandleInformation(raw, HANDLE_FLAG_PROTECT_FROM_CLOSE.0, HANDLE_FLAGS(0))
                }
                .map_err(io::Error::other)?;
                assert!(retained, "Failed close released the native file prematurely");
                failed_close_observed.set(true);
                Err::<(), _>(cause)
            });
            assert!(failed_close_observed.get(), "Native close fault was not qualified");
            assert!(outcome.is_err());
            assert_eq!(std::fs::read(path)?, b"preserved");
            Ok(())
        })
    }
}
