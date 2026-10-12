// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer the original authenticated entry allocation into captured startup custody.
use super::{AuthenticatedEntryLaunch, EntryTransportError, protocol};
use std::ops::Range;

/// Retain an original charged entry frame for one captured desktop startup.
///
/// Shape validation grants no election, profile, process or capture authority.
/// The original connection loan remains held until this value returns its frame.
/// Capture admission must independently fence its actual execution owner, current
/// run, cancellation and unchanged receive deadline before native effects.
pub struct PreparedStartupSnapshot {
    pub(super) launch: AuthenticatedEntryLaunch,
    pub(super) body: Range<usize>,
}

/// Return the same authenticated entry custody when startup shape validation fails.
#[derive(Debug)]
pub struct StartupSnapshotRejection {
    /// Original typed validation cause.
    pub error: EntryTransportError,
    /// Original frame, connection loan, peer and counter deadline.
    pub launch: AuthenticatedEntryLaunch,
}

impl AuthenticatedEntryLaunch {
    /// Validate and transfer the immutable original Launch body without allocation.
    ///
    /// The original memory claim and connection loan move with this value; neither
    /// the clock epoch nor the receive deadline is reconstructed or renewed.
    /// # Errors
    /// Returns the exact original launch and cause if its retained frame fails
    /// the full SSCE1 or unchanged Launch body grammar.
    #[expect(
        clippy::result_large_err,
        reason = "Return original admitted custody without a new fallible allocation"
    )]
    pub fn into_startup_snapshot(
        self,
    ) -> Result<PreparedStartupSnapshot, StartupSnapshotRejection> {
        let validate = || {
            self.view()?;
            let body = self
                .frame
                .bytes()
                .get(protocol::HEADER_BYTES..)
                .ok_or(EntryTransportError::Malformed)?;
            protocol::launch_payload::decode(body)?;
            Ok::<_, EntryTransportError>(protocol::HEADER_BYTES..self.frame.length)
        };
        match validate() {
            Ok(body) => Ok(PreparedStartupSnapshot { launch: self, body }),
            Err(error) => Err(StartupSnapshotRejection { error, launch: self }),
        }
    }
}

impl std::fmt::Debug for PreparedStartupSnapshot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedStartupSnapshot")
            .field("request", &self.launch.request_id())
            .field("body_bytes", &self.body.len())
            .finish_non_exhaustive()
    }
}
