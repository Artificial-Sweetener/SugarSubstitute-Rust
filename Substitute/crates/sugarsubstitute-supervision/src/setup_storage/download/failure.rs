// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve transfer, verification and durable-flush failures for the update coordinator.
use super::space::SpaceShortage;
use crate::setup_storage::cache::CacheBudgetExhausted;
use std::{error::Error, fmt, io};

/// Preparation failed without authorizing execution of cached bytes.
#[derive(Debug)]
pub enum DownloadFailure {
    /// Filesystem, transfer I/O, admission or cancellation cause.
    Storage(io::Error),
    /// The disk does not have the space the update still needs, even after reclaiming. Not
    /// transient: the attempt ends without resuming and the report names the shortage.
    InsufficientSpace(SpaceShortage),
    /// The runtime cache's own byte budget refused the space; the disk may have plenty.
    CacheBudget(io::Error),
    /// Response framing disagrees with the authenticated request.
    Response(sugarsubstitute_update::delivery::Rejection),
    /// Complete bytes or metadata validity failed verification.
    Verification(sugarsubstitute_update::Error),
    /// Both a transfer and its final partial-file flush failed.
    Flush {
        /// Original transfer cause.
        transfer: Box<DownloadFailure>,
        /// Persistence cause; do not claim the partial was durably retained.
        flush: io::Error,
    },
}
impl fmt::Display for DownloadFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Storage(_) => "Update package I/O failed",
            Self::InsufficientSpace(_) => "Not enough free disk space for the update package",
            Self::CacheBudget(_) => "The update package exceeds the runtime cache's byte budget",
            Self::Response(_) => "Update package response rejected",
            Self::Verification(_) => "Update package verification failed",
            Self::Flush { .. } => "Update transfer and partial-file flush failed",
        })
    }
}
impl Error for DownloadFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Storage(cause) | Self::CacheBudget(cause) => Some(cause),
            Self::InsufficientSpace(shortage) => Some(shortage),
            Self::Response(cause) => Some(cause),
            Self::Verification(cause) => Some(cause),
            Self::Flush { transfer, .. } => Some(transfer.as_ref()),
        }
    }
}
impl DownloadFailure {
    /// The disk-space shortage this failure reports, including one beneath a failed flush.
    pub fn space_shortage(&self) -> Option<SpaceShortage> {
        match self {
            Self::InsufficientSpace(shortage) => Some(*shortage),
            Self::Flush { transfer, .. } => transfer.space_shortage(),
            _ => None,
        }
    }
}
/// A typed shortage or cache budget refusal keeps its own variant; every other I/O cause is
/// storage.
impl From<io::Error> for DownloadFailure {
    fn from(cause: io::Error) -> Self {
        if let Some(shortage) = SpaceShortage::find(&cause) {
            Self::InsufficientSpace(shortage)
        } else if CacheBudgetExhausted::within(&cause) {
            Self::CacheBudget(cause)
        } else {
            Self::Storage(cause)
        }
    }
}
impl From<sugarsubstitute_update::Error> for DownloadFailure {
    fn from(cause: sugarsubstitute_update::Error) -> Self {
        Self::Verification(cause)
    }
}
impl From<sugarsubstitute_update::delivery::Rejection> for DownloadFailure {
    fn from(cause: sugarsubstitute_update::delivery::Rejection) -> Self {
        Self::Response(cause)
    }
}

impl From<crate::durable_storage::payload::TransferFailure> for DownloadFailure {
    fn from(cause: crate::durable_storage::payload::TransferFailure) -> Self {
        use crate::durable_storage::payload::TransferFailure;
        use sugarsubstitute_update::delivery::Rejection;
        match cause {
            TransferFailure::Extent => Self::Response(Rejection::Offset),
            TransferFailure::Length => Self::Response(Rejection::BodyLength),
            TransferFailure::Storage(cause) => cause.into(),
            TransferFailure::Flush { transfer, flush } => {
                Self::Flush { transfer: Box::new((*transfer).into()), flush }
            }
        }
    }
}
