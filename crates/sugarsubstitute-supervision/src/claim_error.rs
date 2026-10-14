// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve claim failures without rendering requested paths or credentials.
use std::{error::Error, fmt, sync::Arc};
use sugarsubstitute_contracts::execution::AdmissionError;
mod native;
mod state;
pub(crate) use native::{BoundedNativeCause, NativeCauseDestination};
use state::{ClaimState, SharedClaimState};

/// A retained failure at a specific native claim operation.
#[derive(Clone)]
pub struct ClaimError {
    state: ClaimState,
}
impl fmt::Debug for ClaimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClaimError").field("operation", &self.operation()).finish_non_exhaustive()
    }
}
impl ClaimError {
    /// Carry only the containing storage error category; original causes remain unchanged.
    pub(crate) fn with_storage_metadata(
        mut self,
        code: crate::diagnostic_storage::DiagnosticStorageFailureCode,
        after: Option<crate::diagnostic_storage::DiagnosticStorageFailureCode>,
    ) -> Self {
        self.state.set_storage_metadata(code, after);
        self
    }
    /// Borrow containing storage metadata; these values grant no native authority.
    pub(crate) const fn storage_metadata(
        &self,
    ) -> (
        crate::diagnostic_storage::DiagnosticStorageFailureCode,
        Option<crate::diagnostic_storage::DiagnosticStorageFailureCode>,
    ) {
        self.state.storage_metadata()
    }
    pub(crate) fn new(operation: &'static str, cause: impl Error + Send + Sync + 'static) -> Self {
        Self { state: ClaimState::Shared(SharedClaimState::ordinary(operation, Arc::new(cause))) }
    }
    pub(crate) fn invalid(operation: &'static str) -> Self {
        Self::new(operation, std::io::Error::from(std::io::ErrorKind::InvalidData))
    }
    /// Compare actual original cause/control identities; equality grants no authority.
    pub(crate) fn same_primary(&self, other: &Self) -> bool {
        self.operation() == other.operation() && self.state.same_primary(&other.state)
    }
    /// Compare the original and secondary observation identities without granting authority.
    pub(crate) fn same_observation(&self, other: &Self) -> bool {
        self.operation() == other.operation()
            && self.state.same_primary(&other.state)
            && match (self.cleanup_cause(), other.cleanup_cause()) {
                (None, None) => true,
                (Some(left), Some(right)) => left.state.same_control(&right.state),
                _ => false,
            }
    }
    /// Return the bounded original operation identifier.
    pub fn operation(&self) -> &'static str {
        match &self.state {
            ClaimState::Allocation(_) => "diagnostic_native_allocation",
            ClaimState::CaptureAllocation(_) => "capture_native_allocation",
            ClaimState::Shared(state) => state.operation(),
        }
    }
    /// Borrow only a concrete original Win32 leaf, retaining all cause ownership.
    /// Unknown leaf callbacks, formatting and secondary causes are never inspected.
    pub fn win32_code(&self) -> Option<u32> {
        let cause = self.source()?;
        if let Some(native) =
            cause.downcast_ref::<sugarsubstitute_execution::windows::NativeObservationError>()
        {
            return native.win32_code();
        }
        if let Some(cause) = cause.downcast_ref::<std::io::Error>() {
            return cause.raw_os_error().and_then(|code| u32::try_from(code).ok());
        }
        if let Some(cause) = cause.downcast_ref::<windows::core::Error>() {
            let code = cause.code().0 as u32;
            return (code & 0xffff_0000 == 0x8007_0000).then_some(code & 0xffff);
        }
        None
    }
    /// Borrow an observed cleanup failure without replacing the primary cause.
    pub fn cleanup_cause(&self) -> Option<&Self> {
        match &self.state {
            ClaimState::Allocation(_) | ClaimState::CaptureAllocation(_) => None,
            ClaimState::Shared(state) => state.cleanup(),
        }
    }
    pub(crate) fn finish<T>(result: Result<T, Self>, cleanup: Result<(), Self>) -> Result<T, Self> {
        match (result, cleanup) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(cause), Ok(())) | (Ok(_), Err(cause)) => Err(cause),
            (Err(cause), Err(secondary)) => Err(cause.append_cleanup(secondary)),
        }
    }
    fn append_cleanup(self, secondary: Self) -> Self {
        if self.state.native_ticket().is_some() || secondary.state.native_ticket().is_some() {
            return self.with_native_secondary(secondary);
        }
        if let Some(existing) = self.cleanup_cause() {
            let updated = existing.clone().append_cleanup(secondary);
            return Self {
                state: ClaimState::Shared(SharedClaimState::ordinary_update(self, updated)),
            };
        }
        Self { state: ClaimState::Shared(SharedClaimState::joined(self, secondary, None)) }
    }
}
impl fmt::Display for ClaimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Native resource claim failed at {}", self.operation())
    }
}
impl Error for ClaimError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.state.source()
    }
}
/// Reject a submission before new native effects; retained predecessors stay owned.
#[derive(Debug)]
pub enum ClaimAdmissionError {
    /// Initialization or closure prevents this operation.
    Closed,
    /// A pending operation, prepared launch, or live family prevents retirement.
    Busy,
    /// The fixed owner or storage inventory is full.
    Saturated,
    /// This identity belongs to another owner or a retired generation.
    Identity,
    /// The job class, operation, reservation, or launch shape is invalid.
    Specification,
    /// The execution owner rejected admission without running the factory.
    Execution(AdmissionError),
    /// Actual registry admission rejected the managed attempt.
    Registry(crate::RegistryAdmissionError),
    /// Native preparation failed; the detailed cause remains available.
    Native(ClaimError),
}
impl fmt::Display for ClaimAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Resource claim admission rejected: {}",
            match self {
                Self::Closed => "closed",
                Self::Busy => "busy",
                Self::Saturated => "saturated",
                Self::Identity => "identity",
                Self::Specification => "specification",
                Self::Execution(_) => "execution",
                Self::Registry(_) => "registry",
                Self::Native(_) => "native preparation",
            }
        )
    }
}
impl Error for ClaimAdmissionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Execution(cause) => Some(cause),
            Self::Registry(cause) => Some(cause),
            Self::Native(cause) => Some(cause),
            _ => None,
        }
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
        let cause = ClaimError::new("original_operation", std::io::Error::from_raw_os_error(122));
        assert_eq!(cause.win32_code(), Some(122));
        assert_eq!(
            cause
                .source()
                .and_then(|cause| cause.downcast_ref::<std::io::Error>())
                .and_then(std::io::Error::raw_os_error),
            Some(122)
        );
        let cause =
            ClaimError::new("original_operation", Unknown(std::sync::atomic::AtomicUsize::new(0)));
        assert_eq!(cause.win32_code(), None);
        #[cfg(windows)]
        {
            let cause = ClaimError::new(
                "original_operation",
                windows::core::Error::from_hresult(windows::core::HRESULT::from_win32(5)),
            );
            assert_eq!(cause.win32_code(), Some(5));
            let cause = ClaimError::new(
                "original_operation",
                windows::core::Error::from_hresult(windows::core::HRESULT(0x8000_4005u32 as i32)),
            );
            assert_eq!(cause.win32_code(), None);
        }
    }
}
