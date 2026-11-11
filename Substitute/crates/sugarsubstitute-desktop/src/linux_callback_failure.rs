// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one native drawing callback cause until event-loop observation or teardown.

use crate::DesktopError;

/// Share through the UI-thread-local GTK callback without accumulating error history.
#[derive(Default)]
pub(crate) struct CallbackFailure {
    first: Option<DesktopError>,
}

impl CallbackFailure {
    /// Retain the first unread error and report every later callback failure structurally.
    pub(crate) fn record(&mut self, error: DesktopError) {
        DesktopError::retain_first(&mut self.first, error);
    }

    /// Transfer the original typed cause once so static surfaces and close can observe it.
    pub(crate) fn take(&mut self) -> Result<(), DesktopError> {
        match self.first.take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl Drop for CallbackFailure {
    fn drop(&mut self) {
        if let Some(error) = self.first.take() {
            tracing::error!(operation = "unobserved_gtk_callback", cause = %error, "Native callback owner dropped with an unread failure");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        error::Error,
        fmt,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    /// Observe static/final callback errors once without allowing later failures to overwrite them.
    #[test]
    fn callback_failure_retains_first_cause_until_polled_and_then_accepts_next()
    -> Result<(), Box<dyn Error>> {
        let mut failures = CallbackFailure::default();
        failures.record(DesktopError::new(
            "paint_gtk_frame",
            std::io::Error::new(std::io::ErrorKind::BrokenPipe, "first callback"),
        ));
        failures.record(DesktopError::message("paint_gtk_frame", "later callback"));
        let first = failures.take().err().ok_or("Callback cause lost")?;
        let cause = first
            .source()
            .and_then(|cause| cause.downcast_ref::<std::io::Error>())
            .ok_or("Original callback type lost")?;
        assert_eq!(cause.kind(), std::io::ErrorKind::BrokenPipe);
        assert_eq!(cause.to_string(), "first callback");
        assert!(failures.take().is_ok());
        failures.record(DesktopError::message("paint_gtk_frame", "next callback"));
        assert_eq!(
            failures.take().err().ok_or("Next callback lost")?.to_string(),
            "paint_gtk_frame: next callback"
        );
        assert!(failures.take().is_ok());
        Ok(())
    }

    #[derive(Debug)]
    struct OwnedCause(Arc<AtomicUsize>);
    impl fmt::Display for OwnedCause {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("owned callback")
        }
    }
    impl Error for OwnedCause {}
    impl Drop for OwnedCause {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// Bounded storage transfers primary ownership; reported secondary causes are released immediately.
    #[test]
    fn observed_callback_error_outlives_its_slot_without_accumulating_secondary_causes()
    -> Result<(), Box<dyn Error>> {
        let released = Arc::new(AtomicUsize::new(0));
        let mut failures = CallbackFailure::default();
        failures.record(DesktopError::new("paint_gtk_frame", OwnedCause(released.clone())));
        failures.record(DesktopError::new("paint_gtk_frame", OwnedCause(released.clone())));
        assert_eq!(released.load(Ordering::SeqCst), 1);
        let retained = failures.take().err().ok_or("Owned callback cause lost")?;
        drop(failures);
        assert_eq!(released.load(Ordering::SeqCst), 1);
        drop(retained);
        assert_eq!(released.load(Ordering::SeqCst), 2);
        Ok(())
    }
}
