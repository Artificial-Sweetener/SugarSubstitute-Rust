// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own retained filesystem custody, bounded records and staged native publication.
//!
//! These crate-private capabilities run within work admitted by the execution owner.
//! Ordinary directory pins and protected, allocation-charged native objects retain
//! their distinct security and retirement contracts. Features own record schemas,
//! authorization, migration, recovery and the decision to replace existing state.
//! Paths borrowed from a retained object are observations, never authority grants.

mod control;
pub(crate) mod files;
pub(crate) mod input;
pub(crate) mod payload;
#[allow(unsafe_code)]
pub(crate) mod protected;
#[allow(unsafe_code)]
pub(crate) mod protection;
#[allow(unsafe_code)]
pub(crate) mod receiving;
mod record_read;
pub(crate) mod records;
#[allow(unsafe_code)]
pub(crate) mod retirement;
#[allow(unsafe_code)]
pub(crate) mod staging;
pub(crate) mod streamed_record;

use std::io;
use sugarsubstitute_contracts::execution::JobContext;

/// Check the active finite operation before entering ordinary storage mechanics.
fn check(context: &JobContext) -> io::Result<()> {
    if context.cancellation_requested() {
        return Err(io::Error::new(io::ErrorKind::Interrupted, "Storage operation cancelled"));
    }
    if context.deadline().is_some_and(|until| context.now() >= until) {
        return Err(io::Error::new(io::ErrorKind::TimedOut, "Storage operation expired"));
    }
    Ok(())
}

/// Reject an invalid storage request without dropping its originating I/O cause.
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
