// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve original adapter causes without echoing private paths or native strings.
use std::{error::Error, fmt, sync::Arc};
use sugarsubstitute_contracts::{crash::CaptureOperation, execution::AdmissionError};

/// Original unexpected cause at one capture boundary, with redacted formatting.
#[derive(Clone)]
pub struct CrashError {
    operation: CaptureOperation,
    cause: Arc<dyn Error + Send + Sync>,
    secondary: Arc<[Option<Arc<dyn Error + Send + Sync>>; 8]>,
}
impl CrashError {
    pub(crate) fn new(
        operation: CaptureOperation,
        cause: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            operation,
            cause: Arc::new(cause),
            secondary: Arc::new(std::array::from_fn(|_| None)),
        }
    }
    pub(crate) fn invalid(operation: CaptureOperation) -> Self {
        Self::new(operation, std::io::Error::other("Invalid bounded capture boundary"))
    }
    /// Return the categorical operation without exposing sensitive cause text.
    pub const fn operation(&self) -> CaptureOperation {
        self.operation
    }
    #[cfg(any(windows, test))]
    pub(crate) fn same_cause(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.cause, &other.cause)
    }
    #[cfg(any(windows, test))]
    pub(crate) fn with_secondary(mut self, cause: impl Error + Send + Sync + 'static) -> Self {
        if let Some(slot) =
            Arc::make_mut(&mut self.secondary).iter_mut().find(|slot| slot.is_none())
        {
            *slot = Some(Arc::new(cause));
        }
        self
    }
    /// Borrow preserved secondary cleanup causes without formatting them into diagnostics.
    pub fn secondary_causes(&self) -> impl Iterator<Item = &(dyn Error + Send + Sync + 'static)> {
        self.secondary.iter().flatten().map(Arc::as_ref)
    }
}
impl fmt::Debug for CrashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CrashError").field("operation", &self.operation).finish_non_exhaustive()
    }
}
impl fmt::Display for CrashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Capture {:?} failed", self.operation)
    }
}
impl Error for CrashError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}

/// Reject a submission before native effects while preserving input ownership.
pub enum CrashAdmissionError {
    /// This capture owner has already attempted registration or has active work.
    Busy,
    /// The finite operation specification/deadline/storage bound is invalid.
    InvalidSpecification,
    /// Central execution rejected admission before invoking the factory.
    Execution(AdmissionError),
}
impl fmt::Debug for CrashAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Busy => "Busy",
            Self::InvalidSpecification => "InvalidSpecification",
            Self::Execution(_) => "Execution(..)",
        })
    }
}
impl fmt::Display for CrashAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Capture admission rejected: {self:?}")
    }
}
impl Error for CrashAdmissionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Execution(cause) => Some(cause),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
