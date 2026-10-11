// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve claim failures without rendering requested paths or credentials.
use std::{error::Error, fmt, sync::Arc};
use sugarsubstitute_contracts::execution::AdmissionError;

/// A retained failure at a specific native claim operation.
#[derive(Clone)]
pub struct ClaimError {
    operation: &'static str,
    cause: Arc<dyn Error + Send + Sync>,
    cleanup: Option<Arc<ClaimError>>,
}
impl fmt::Debug for ClaimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClaimError").field("operation", &self.operation).finish_non_exhaustive()
    }
}
impl ClaimError {
    pub(crate) fn new(operation: &'static str, cause: impl Error + Send + Sync + 'static) -> Self {
        Self { operation, cause: Arc::new(cause), cleanup: None }
    }
    pub(crate) fn invalid(operation: &'static str) -> Self {
        Self::new(operation, std::io::Error::from(std::io::ErrorKind::InvalidData))
    }
    /// Return the bounded operation identifier; input text is never included.
    pub const fn operation(&self) -> &'static str {
        self.operation
    }
    /// Borrow an observed cleanup failure without replacing the primary cause.
    pub fn cleanup_cause(&self) -> Option<&Self> {
        self.cleanup.as_deref()
    }
    pub(crate) fn finish<T>(result: Result<T, Self>, cleanup: Result<(), Self>) -> Result<T, Self> {
        match (result, cleanup) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(cause), Ok(())) => Err(cause),
            (Ok(_), Err(cause)) => Err(cause),
            (Err(mut cause), Err(secondary)) => {
                cause.append_cleanup(secondary);
                Err(cause)
            }
        }
    }
    fn append_cleanup(&mut self, secondary: Self) {
        if let Some(existing) = self.cleanup.as_mut() {
            Arc::make_mut(existing).append_cleanup(secondary);
        } else {
            self.cleanup = Some(Arc::new(secondary));
        }
    }
}
impl fmt::Display for ClaimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Native resource claim failed at {}", self.operation)
    }
}
impl Error for ClaimError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
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
