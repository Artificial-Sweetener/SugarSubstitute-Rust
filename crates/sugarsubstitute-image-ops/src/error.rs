// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve image-domain failures and original external boundary causes.
use std::{error::Error, fmt};
use sugarsubstitute_contracts::execution::MemoryClaimError;

/// Preserve original execution controls when projecting cancellation into image-domain errors.
pub(crate) fn check(
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> Result<(), ImageError> {
    if context.cancellation_requested() {
        return Err(ImageError::Cancelled);
    }
    if context.deadline().is_some_and(|deadline| context.now() >= deadline) {
        return Err(ImageError::Deadline);
    }
    Ok(())
}

/// Reject invalid graphs, unavailable resources or unfinished image requests.
#[derive(Debug)]
pub enum ImageError {
    /// Invalid dimensions, coordinates, operation parameters or graph references.
    Invalid(&'static str),
    /// Cancellation was requested; actual worker termination belongs to execution.
    Cancelled,
    /// The admitted operation's effective deadline elapsed.
    Deadline,
    /// Retained allocations cannot fit their original execution reservation.
    Memory(MemoryClaimError),
    /// The allocator could not provide already-admitted storage.
    Allocation(std::collections::TryReserveError),
    /// A source adapter failed, preserving the original error chain.
    Source(Box<dyn Error + Send + Sync>),
}
impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => f.write_str(message),
            Self::Cancelled => f.write_str("Image evaluation cancelled"),
            Self::Deadline => f.write_str("Image evaluation deadline elapsed"),
            Self::Memory(cause) => write!(f, "Image memory admission: {cause}"),
            Self::Allocation(cause) => write!(f, "Image allocation: {cause}"),
            Self::Source(cause) => write!(f, "Image source: {cause}"),
        }
    }
}
impl Error for ImageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory(cause) => Some(cause),
            Self::Allocation(cause) => Some(cause),
            Self::Source(cause) => Some(cause.as_ref()),
            _ => None,
        }
    }
}
impl From<MemoryClaimError> for ImageError {
    fn from(cause: MemoryClaimError) -> Self {
        Self::Memory(cause)
    }
}
impl From<std::collections::TryReserveError> for ImageError {
    fn from(cause: std::collections::TryReserveError) -> Self {
        Self::Allocation(cause)
    }
}
