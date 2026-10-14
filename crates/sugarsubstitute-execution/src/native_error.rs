// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve native first causes and sticky supervised fatal facts across retirement.
use std::{error::Error, fmt, sync::Arc};
use sugarsubstitute_contracts::execution::{JobOutcome, PanicCause};

/// Preserve the first native stage failure and its original owned cause.
///
/// Up to eight subsequent cleanup failures remain owned; further actual failures
/// are counted explicitly. `source` always identifies the original cause.
#[derive(Clone, Debug)]
pub struct NativeObservationError {
    stage: &'static str,
    cause: Arc<dyn Error + Send + Sync>,
    pub(super) cleanup: Vec<Arc<NativeObservationError>>,
    pub(super) omitted_cleanup: u64,
    owner_panic: Option<Arc<OwnerPanic>>,
}
impl NativeObservationError {
    /// Retain an external adapter's original owned cause without granting effects.
    /// The adapter counts this error allocation within its original admitted storage.
    /// No native call, provider or scheduling primitive is invoked.
    pub fn new(stage: &'static str, cause: impl Error + Send + Sync + 'static) -> Self {
        Self {
            stage,
            cause: Arc::new(cause),
            cleanup: Vec::new(),
            omitted_cleanup: 0,
            owner_panic: None,
        }
    }
    /// Identify the native operation whose original cause is retained by `source`.
    pub const fn stage(&self) -> &'static str {
        self.stage
    }
    /// Borrow a concrete original Win32 code without formatting or invoking its source.
    /// Non-Win32 HRESULTs and arbitrary causes have no Win32 classification.
    pub fn win32_code(&self) -> Option<u32> {
        if let Some(cause) = self.cause.downcast_ref::<std::io::Error>() {
            return cause.raw_os_error().and_then(|code| u32::try_from(code).ok());
        }
        #[cfg(windows)]
        if let Some(cause) = self.cause.downcast_ref::<windows::core::Error>() {
            let code = cause.code().0 as u32;
            return (code & 0xffff_0000 == 0x8007_0000).then_some(code & 0xffff);
        }
        None
    }
    /// Report an authoritative native-owner panic without inspecting error text.
    /// This fact and its first bounded cause survive cleanup omission and cloning.
    pub const fn contains_owner_panic(&self) -> bool {
        self.owner_panic.is_some()
    }
    /// Borrow the first supervised owner panic for explicit fatal-policy propagation.
    /// Payload is bounded by the execution owner; ordinary error formatting redacts it.
    pub fn owner_panic_cause(&self) -> Option<&PanicCause> {
        self.owner_panic.as_ref().map(|cause| &cause.0)
    }
    pub(crate) fn retain_panic(&mut self, cause: &PanicCause) {
        if self.owner_panic.is_none() {
            self.owner_panic = Some(Arc::new(OwnerPanic(cause.clone())));
        }
    }
    pub(crate) fn retain_cleanup(&mut self, mut cleanup: Self) {
        if self.owner_panic.is_none() {
            self.owner_panic = cleanup.owner_panic.clone();
        }
        let nested = std::mem::take(&mut cleanup.cleanup);
        let omitted = std::mem::replace(&mut cleanup.omitted_cleanup, 0);
        if self.cleanup.len() < 8 {
            self.cleanup.push(Arc::new(cleanup));
        } else {
            self.omitted_cleanup = self.omitted_cleanup.saturating_add(1);
        }
        self.omitted_cleanup = self.omitted_cleanup.saturating_add(omitted);
        for cleanup in nested {
            self.retain_cleanup((*cleanup).clone());
        }
    }
}
impl fmt::Display for NativeObservationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Native observation {} failed: {}", self.stage, self.cause)?;
        for cleanup in &self.cleanup {
            write!(f, "; cleanup: {cleanup}")?;
        }
        if self.omitted_cleanup != 0 {
            write!(f, "; {} further cleanup failures omitted", self.omitted_cleanup)?;
        }
        Ok(())
    }
}
impl Error for NativeObservationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}

struct OwnerPanic(PanicCause);
impl fmt::Debug for OwnerPanic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OwnerPanic { redacted }")
    }
}
pub(crate) fn record_owner_panic<T>(
    outcome: &mut Option<JobOutcome<T, NativeObservationError>>,
    cause: &PanicCause,
) {
    match outcome {
        Some(JobOutcome::Failure(error)) => error.retain_panic(cause),
        Some(JobOutcome::Panicked(_)) => {}
        outcome => *outcome = Some(JobOutcome::Panicked(cause.clone())),
    }
}
pub(crate) fn retain_retirement<T>(
    outcome: &mut Option<JobOutcome<T, NativeObservationError>>,
    mut error: NativeObservationError,
) {
    match outcome {
        Some(JobOutcome::Failure(primary)) => primary.retain_cleanup(error),
        Some(JobOutcome::Panicked(cause)) => {
            error.retain_panic(cause);
            *outcome = Some(JobOutcome::Failure(error));
        }
        outcome => *outcome = Some(JobOutcome::Failure(error)),
    }
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;
    struct Unknown(std::sync::atomic::AtomicUsize);
    impl fmt::Debug for Unknown {
        fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
            assert_eq!(
                self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
                usize::MAX,
                "unknown debug callback"
            );
            Ok(())
        }
    }
    impl fmt::Display for Unknown {
        fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
            assert_eq!(
                self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
                usize::MAX,
                "unknown display callback"
            );
            Ok(())
        }
    }
    impl Error for Unknown {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            assert_eq!(
                self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
                usize::MAX,
                "unknown source callback"
            );
            None
        }
    }
    #[test]
    fn win32_diagnostic_keeps_concrete_code_without_unknown_callbacks() {
        let cause = NativeObservationError::new(
            "original_operation",
            std::io::Error::from_raw_os_error(122),
        );
        assert_eq!(cause.win32_code(), Some(122));
        assert_eq!(
            cause
                .source()
                .and_then(|cause| cause.downcast_ref::<std::io::Error>())
                .and_then(std::io::Error::raw_os_error),
            Some(122)
        );
        let cause = NativeObservationError::new(
            "original_operation",
            Unknown(std::sync::atomic::AtomicUsize::new(0)),
        );
        assert_eq!(cause.win32_code(), None);
        #[cfg(windows)]
        {
            let cause = NativeObservationError::new(
                "original_operation",
                windows::core::Error::from_hresult(windows::core::HRESULT::from_win32(5)),
            );
            assert_eq!(cause.win32_code(), Some(5));
            let cause = NativeObservationError::new(
                "original_operation",
                windows::core::Error::from_hresult(windows::core::HRESULT(0x8000_4005u32 as i32)),
            );
            assert_eq!(cause.win32_code(), None);
        }
    }
}
