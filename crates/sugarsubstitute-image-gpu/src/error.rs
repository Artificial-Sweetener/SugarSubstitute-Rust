// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve GPU, image and execution failures across the backend boundary.
use std::{error::Error, fmt};

/// An invalid GPU request or its original driver/image/execution cause.
#[derive(Debug)]
pub struct GpuError {
    operation: &'static str,
    cause: Box<dyn Error + Send + Sync>,
}
impl GpuError {
    /// Native retirement failure accompanying the original preparation error, when both occurred.
    pub fn retirement_cause(&self) -> Option<&Self> {
        self.cause.downcast_ref::<Failures>().map(|causes| &causes.retirement)
    }
    pub(crate) fn new(
        operation: &'static str,
        cause: impl Into<Box<dyn Error + Send + Sync>>,
    ) -> Self {
        Self { operation, cause: cause.into() }
    }
    pub(crate) fn invalid(message: &'static str) -> Self {
        Self::new("gpu_request", std::io::Error::new(std::io::ErrorKind::InvalidInput, message))
    }
    /// Whether execution admission rejected an allocation, with no accompanying native retirement
    /// failure. The caller may release reconstructible resources and retry within the same budget.
    pub fn is_memory_pressure(&self) -> bool {
        if self.retirement_cause().is_some() {
            return false;
        }
        let mut cause = self.source();
        while let Some(current) = cause {
            if current
                .downcast_ref::<Self>()
                .is_some_and(|cause| cause.retirement_cause().is_some())
            {
                return false;
            }
            if current.is::<sugarsubstitute_contracts::execution::MemoryClaimError>() {
                return true;
            }
            cause = current.source();
        }
        false
    }
    pub(crate) fn retirement(primary: Self, retirement: Self) -> Self {
        Self::new("gpu_preparation_retirement", Failures { primary, retirement })
    }
}

#[derive(Debug)]
struct Failures {
    primary: GpuError,
    retirement: GpuError,
}
impl fmt::Display for Failures {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}; native retirement: {}", self.primary, self.retirement)
    }
}
impl Error for Failures {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.primary)
    }
}
impl fmt::Display for GpuError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.operation, self.cause)
    }
}
impl Error for GpuError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_execution_admission_without_native_retirement_failure_is_retryable() {
        let admission = || {
            GpuError::new(
                "allocation",
                sugarsubstitute_contracts::execution::MemoryClaimError {
                    requested_bytes: 100,
                    available_bytes: 1,
                },
            )
        };
        assert!(admission().is_memory_pressure());
        assert!(
            !GpuError::new("driver", std::io::Error::from(std::io::ErrorKind::OutOfMemory))
                .is_memory_pressure()
        );
        let combined = GpuError::retirement(admission(), GpuError::invalid("Native failure"));
        assert!(!combined.is_memory_pressure());
        assert!(!GpuError::new("wrapped", combined).is_memory_pressure());
    }
    #[test]
    fn preparation_and_retirement_keep_both_original_typed_causes() -> Result<(), Box<dyn Error>> {
        let cause = GpuError::retirement(
            GpuError::new("read_source", std::io::Error::from_raw_os_error(17)),
            GpuError::new("native_retirement", std::io::Error::from_raw_os_error(23)),
        );
        let primary = cause
            .source()
            .and_then(Error::source)
            .and_then(Error::source)
            .and_then(|cause| cause.downcast_ref::<std::io::Error>())
            .ok_or("Missing original source failure")?;
        assert_eq!(primary.raw_os_error(), Some(17));
        let retirement = cause
            .retirement_cause()
            .and_then(Error::source)
            .and_then(|cause| cause.downcast_ref::<std::io::Error>())
            .ok_or("Missing original native retirement failure")?;
        assert_eq!(retirement.raw_os_error(), Some(23));
        Ok(())
    }
}
