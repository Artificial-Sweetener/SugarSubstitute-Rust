// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own retained filesystem custody, bounded records and staged native publication.
//!
//! These crate-private capabilities run within work admitted by the execution owner.
//! Ordinary directory pins and protected, allocation-charged native objects retain
//! their distinct security and retirement contracts. Features own record schemas,
//! authorization, migration, recovery and the decision to replace existing state.
//! Paths borrowed from a retained object are observations, never authority grants.

pub(crate) mod artifacts;
mod control;
#[cfg(windows)]
pub(crate) mod files;
#[cfg(windows)]
pub(crate) mod imports;
#[cfg(windows)]
pub(crate) mod input;
#[cfg(windows)]
pub(crate) mod payload;
#[cfg(windows)]
#[allow(unsafe_code)]
pub(crate) mod protected;
#[cfg(windows)]
#[allow(unsafe_code)]
pub(crate) mod protection;
#[cfg(windows)]
#[allow(unsafe_code)]
pub(crate) mod receiving;
#[cfg(windows)]
pub(crate) mod record_publication;
#[cfg(windows)]
mod record_read;
#[cfg(windows)]
pub(crate) mod records;
#[cfg(windows)]
#[allow(unsafe_code)]
pub(crate) mod retirement;
#[cfg(windows)]
#[allow(unsafe_code)]
pub(crate) mod staging;
#[cfg(windows)]
pub(crate) mod streamed_record;
#[cfg(windows)]
pub(crate) mod user_paths;

use std::io;
use sugarsubstitute_contracts::execution::JobContext;

/// Check the active finite operation before entering ordinary storage mechanics.
pub(crate) fn check(context: &JobContext) -> io::Result<()> {
    if context.cancellation_requested() {
        return Err(io::Error::new(io::ErrorKind::Interrupted, "Storage operation cancelled"));
    }
    if context.deadline().is_some_and(|until| context.now() >= until) {
        return Err(io::Error::new(io::ErrorKind::TimedOut, "Storage operation expired"));
    }
    Ok(())
}

/// Reject an invalid storage request without dropping its originating I/O cause.
pub(crate) fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
