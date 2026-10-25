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
    pub(crate) fn new(
        operation: &'static str,
        cause: impl Into<Box<dyn Error + Send + Sync>>,
    ) -> Self {
        Self { operation, cause: cause.into() }
    }
    pub(crate) fn invalid(message: &'static str) -> Self {
        Self::new("gpu_request", std::io::Error::new(std::io::ErrorKind::InvalidInput, message))
    }
    pub(crate) fn memory_pressure(&self) -> bool {
        let mut cause = self.source();
        while let Some(current) = cause {
            if current.is::<sugarsubstitute_contracts::execution::MemoryClaimError>() {
                return true;
            }
            cause = current.source();
        }
        false
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
