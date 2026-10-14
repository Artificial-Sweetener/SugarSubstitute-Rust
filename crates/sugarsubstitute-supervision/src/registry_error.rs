// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain native/admission causes without displaying executable or environment data.
use std::{error::Error, fmt, sync::Arc};
use sugarsubstitute_contracts::execution::{AdmissionError, PanicCause};

/// Reject bounded launch storage before execution admission or native effects.
pub enum RegistryInputError {
    /// Output binding/owner/resource generation does not match the owned launch.
    OutputBinding,
    /// Stdio and output control do not belong to the exact source session.
    OutputProvenance,
    /// Another output artifact or mutually exclusive fixture events are present.
    OutputConflict,
    /// Bootstrap and enrollment do not belong to the exact same admitted session.
    LifecycleProvenance,
    /// Diagnostic pins are missing or belong to different actual sessions/bindings.
    DiagnosticProvenance,
    /// Diagnostic channel ownership conflicts with another inherited native role.
    DiagnosticConflict,
    /// Checked capacity accounting overflowed or exceeded supported storage.
    Capacity,
    /// Retained storage could not be allocated.
    Allocation(std::collections::TryReserveError),
    /// Environment capacity accounting failed.
    Environment(crate::LaunchEnvironmentError),
}
impl fmt::Display for RegistryInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Windows launch storage is invalid")
    }
}
impl Error for RegistryInputError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Capacity
            | Self::LifecycleProvenance
            | Self::DiagnosticProvenance
            | Self::DiagnosticConflict
            | Self::OutputBinding
            | Self::OutputProvenance
            | Self::OutputConflict => None,
            Self::Allocation(cause) => Some(cause),
            Self::Environment(cause) => Some(cause),
        }
    }
}
/// Reject an operation without admitting or claiming a native effect.
pub enum RegistryAdmissionError {
    /// Caller binding belongs to another coordinator or executor lifetime.
    ForeignOwner,
    /// Required scope, specification or immutable budget is invalid.
    InvalidSpecification,
    /// Admission is closed for this exact family or registry.
    Closed,
    /// The fixed family/member-watch allowance is exhausted.
    Saturated,
    /// Exact key is absent or no longer identifies a current record.
    Stale,
    /// An operation is already owned for this family.
    Busy,
    /// A monotonically assigned registry identity cannot advance.
    IdentityExhausted,
    /// The execution owner rejected work before its factory ran.
    Execution(AdmissionError),
}
impl fmt::Display for RegistryAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Windows registry admission rejected")
    }
}
impl Error for RegistryAdmissionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Execution(cause) => Some(cause),
            _ => None,
        }
    }
}
impl fmt::Debug for RegistryInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RegistryInputError { redacted }")
    }
}
impl fmt::Debug for RegistryAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RegistryAdmissionError { redacted }")
    }
}
/// Own the first failure plus at most eight subsequent cleanup causes.
/// Display/Debug redact inputs; callers can inspect the original source explicitly.
#[derive(Clone)]
pub struct RegistryError {
    stage: &'static str,
    cause: Arc<dyn Error + Send + Sync>,
    cleanup: Vec<RegistryError>,
    omitted: u64,
    owner_panic: Option<Arc<OwnerPanic>>,
}
impl RegistryError {
    pub(crate) fn new(stage: &'static str, cause: impl Error + Send + Sync + 'static) -> Self {
        Self { stage, cause: Arc::new(cause), cleanup: Vec::new(), omitted: 0, owner_panic: None }
    }
    pub(crate) fn owner_panic(stage: &'static str, cause: PanicCause) -> Self {
        let cause = Arc::new(OwnerPanic(cause));
        Self {
            stage,
            cause: cause.clone(),
            cleanup: Vec::new(),
            omitted: 0,
            owner_panic: Some(cause),
        }
    }
    pub(crate) fn native_observation(
        stage: &'static str,
        cause: sugarsubstitute_execution::windows::NativeObservationError,
    ) -> Self {
        let panic = cause.owner_panic_cause().cloned();
        let mut error = Self::new(stage, cause);
        error.owner_panic = panic.map(|cause| Arc::new(OwnerPanic(cause)));
        error
    }
    pub(crate) fn invalid(stage: &'static str) -> Self {
        Self::new(stage, std::io::Error::other("Native lifecycle proof is incomplete"))
    }
    pub(crate) fn retain(&mut self, mut cause: Self) {
        if self.owner_panic.is_none() {
            self.owner_panic = cause.owner_panic.clone();
        }
        let nested = std::mem::take(&mut cause.cleanup);
        let omitted = std::mem::replace(&mut cause.omitted, 0);
        if self.cleanup.len() < 8 {
            self.cleanup.push(cause);
        } else {
            self.omitted = self.omitted.saturating_add(1);
        }
        self.omitted = self.omitted.saturating_add(omitted);
        for cause in nested {
            self.retain(cause);
        }
    }
    /// Identify the failed operation without exposing launch data.
    pub const fn stage(&self) -> &'static str {
        self.stage
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
    /// Borrow the bounded secondary native causes in actual observation order.
    pub fn cleanup_causes(&self) -> &[Self] {
        &self.cleanup
    }
    /// Count additional causes omitted after the bounded secondary inventory filled.
    pub const fn omitted_causes(&self) -> u64 {
        self.omitted
    }
    /// Report a supervised authoritative-owner panic anywhere in the observed causes.
    /// This fatal policy fact survives bounded cause omission and cloning. It does
    /// not authorize automatic restart or termination; callers apply their owned
    /// fatal-error policy. Panic text stays in the explicitly inspectable source.
    pub const fn contains_owner_panic(&self) -> bool {
        self.owner_panic.is_some()
    }
    /// Borrow the first bounded fatal cause even if its secondary error was omitted.
    /// This explicit diagnostic access preserves the redaction of Display and Debug.
    pub fn owner_panic_cause(&self) -> Option<&PanicCause> {
        self.owner_panic.as_ref().map(|cause| &cause.0)
    }
}
struct OwnerPanic(PanicCause);
impl fmt::Debug for OwnerPanic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OwnerPanic { redacted }")
    }
}
impl fmt::Display for OwnerPanic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.as_str())
    }
}
impl Error for OwnerPanic {}
impl fmt::Debug for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegistryError")
            .field("stage", &self.stage)
            .field("secondary_count", &self.cleanup.len())
            .field("omitted", &self.omitted)
            .finish()
    }
}
impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Windows registry operation {} failed", self.stage)
    }
}
impl Error for RegistryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
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
        let cause =
            RegistryError::new("original_operation", std::io::Error::from_raw_os_error(122));
        assert_eq!(cause.win32_code(), Some(122));
        assert_eq!(
            cause
                .source()
                .and_then(|cause| cause.downcast_ref::<std::io::Error>())
                .and_then(std::io::Error::raw_os_error),
            Some(122)
        );
        let cause = RegistryError::new(
            "original_operation",
            Unknown(std::sync::atomic::AtomicUsize::new(0)),
        );
        assert_eq!(cause.win32_code(), None);
        #[cfg(windows)]
        {
            let cause = RegistryError::new(
                "original_operation",
                windows::core::Error::from_hresult(windows::core::HRESULT::from_win32(5)),
            );
            assert_eq!(cause.win32_code(), Some(5));
            let cause = RegistryError::new(
                "original_operation",
                windows::core::Error::from_hresult(windows::core::HRESULT(0x8000_4005u32 as i32)),
            );
            assert_eq!(cause.win32_code(), None);
        }
    }
}
