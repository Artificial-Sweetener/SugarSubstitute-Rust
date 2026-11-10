// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Stand in for native process creation when a test only needs Windows to refuse an image.
//!
//! Handing a fixture's inert bytes to `CreateProcessW` makes Windows raise a modal hard-error
//! dialog that blocks the creating thread until a person dismisses it. This boundary records each
//! creation attempt and fails with the native error Windows reports for a non-executable image,
//! so the admission, retention and recovery behavior under test runs without reaching the OS loader.

use super::ApplicationLaunch;
use crate::ClaimError;
use std::sync::atomic::{AtomicUsize, Ordering};
use sugarsubstitute_contracts::execution::JobContext;
use windows::Win32::Foundation::ERROR_BAD_EXE_FORMAT;

/// Count creation attempts that reached the native boundary and were refused there.
#[derive(Default)]
pub(crate) struct RejectedCreation {
    attempts: AtomicUsize,
}

impl RejectedCreation {
    /// Number of creation attempts the launch admitted and handed to the native boundary.
    pub(crate) fn attempts(&self) -> usize {
        self.attempts.load(Ordering::SeqCst)
    }
}

impl ApplicationLaunch {
    /// Launch through the production admission path with a creation boundary that records the
    /// attempt and fails as Windows does for an image that is not a valid executable.
    ///
    /// # Errors
    /// Returns `application_launch_admission` when the launch refuses before creation, or the
    /// `application_launch_create` failure for an admitted attempt.
    pub(crate) fn launch_rejected(
        &self,
        context: &JobContext,
        creation: &RejectedCreation,
    ) -> Result<(), ClaimError> {
        self.launch_with(context, |_, _, _| {
            creation.attempts.fetch_add(1, Ordering::SeqCst);
            Err(ClaimError::new(
                "application_launch_create",
                std::io::Error::from_raw_os_error(ERROR_BAD_EXE_FORMAT.0 as i32),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        WindowsLaunchCommand,
        setup_storage::test_worker::{self as worker, TestResult},
    };

    /// Write bytes that are not an executable image and name them as the launch command.
    fn inert_launch(
        root: &std::path::Path,
    ) -> Result<ApplicationLaunch, Box<dyn std::error::Error + Send + Sync>> {
        let image = root.join("inert.exe");
        std::fs::write(&image, b"inert, never executed")?;
        Ok(ApplicationLaunch::new(WindowsLaunchCommand::from_os(&image, root, &[])?))
    }

    #[test]
    fn an_admitted_launch_reaches_the_creation_boundary_once_and_a_repeat_is_refused_before_it()
    -> TestResult {
        worker::run_reserved(1024 * 1024, |context, _| {
            let root = tempfile::tempdir()?;
            let launch = inert_launch(root.path())?;
            let creation = RejectedCreation::default();
            let cause = launch.launch_rejected(context, &creation).err().ok_or("Image started")?;
            assert_eq!(cause.operation(), "application_launch_create");
            assert_eq!(creation.attempts(), 1);
            let repeat =
                launch.launch_rejected(context, &creation).err().ok_or("Repeat started")?;
            assert_eq!(repeat.operation(), "application_launch_admission");
            assert_eq!(creation.attempts(), 1);
            Ok(())
        })
    }

    #[test]
    fn an_expired_launch_is_refused_before_the_creation_boundary() -> TestResult {
        worker::run_reserved_expiring(1024 * 1024, |context, clock| {
            let root = tempfile::tempdir()?;
            let launch = inert_launch(root.path())?;
            let creation = RejectedCreation::default();
            clock.expire();
            let cause = launch
                .launch_rejected(context, &creation)
                .err()
                .ok_or("Expired launch was admitted")?;
            assert_eq!(cause.operation(), "application_launch_admission");
            assert_eq!(creation.attempts(), 0);
            Ok(())
        })
    }
}
