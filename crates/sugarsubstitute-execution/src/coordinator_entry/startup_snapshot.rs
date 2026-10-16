// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer the original authenticated entry allocation into captured startup custody.
mod admission;
use super::{AuthenticatedEntryLaunch, EntryTransportError, protocol};
pub(crate) use admission::StartupAdmissionBinding;
use std::ops::Range;

/// Retain an original charged entry frame for one captured desktop startup.
///
/// Shape validation grants no election, profile, process or capture authority.
/// The original connection loan remains held until this value returns its frame.
/// Capture admission must independently fence its actual execution owner, current
/// run, cancellation and unchanged receive deadline before native effects.
pub struct PreparedStartupSnapshot {
    pub(super) launch: Option<AuthenticatedEntryLaunch>,
    pub(super) registered: Option<super::registered_startup::RegisteredSnapshot>,
    pub(super) delegated: Option<crate::recovery_supervisor::startup::SourceRef>,
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
            Ok(body) => Ok(PreparedStartupSnapshot {
                launch: Some(self),
                registered: None,
                delegated: None,
                body,
            }),
            Err(error) => Err(StartupSnapshotRejection { error, launch: self }),
        }
    }
}

impl std::fmt::Debug for PreparedStartupSnapshot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedStartupSnapshot")
            .field(
                "original_entry",
                &self.launch.as_ref().map(AuthenticatedEntryLaunch::request_id),
            )
            .field("body_bytes", &self.body.len())
            .finish_non_exhaustive()
    }
}

impl PreparedStartupSnapshot {
    /// Borrow only the original admitted Launch body; no frame allocation is copied.
    /// # Errors
    /// Rejects missing original frame custody or an invalid retained body range.
    pub(crate) fn body(&self) -> Result<&[u8], EntryTransportError> {
        match (&self.launch, &self.registered, &self.delegated) {
            (Some(launch), None, None) => {
                launch.frame.bytes().get(self.body.clone()).ok_or(EntryTransportError::Malformed)
            }
            (None, Some(registered), None) => {
                registered.body().get(self.body.clone()).ok_or(EntryTransportError::Malformed)
            }
            (None, None, Some(delegated)) => {
                delegated.body.get(self.body.clone()).ok_or(EntryTransportError::Malformed)
            }
            _ => Err(EntryTransportError::Malformed),
        }
    }

    /// Project inert matching facts for final Core-to-Entry admission validation.
    /// The original body and connection loan remain solely in this snapshot.
    /// # Errors
    /// Rejects a missing frame or mismatched original SSCE1 correlation.
    pub(crate) fn admission_binding(&self) -> Result<StartupAdmissionBinding, EntryTransportError> {
        match (&self.launch, &self.registered, &self.delegated) {
            (Some(_), None, None) => admission::project(self).map(StartupAdmissionBinding::Entry),
            (None, Some(registered), None) => {
                Ok(StartupAdmissionBinding::Registered(registered.clone()))
            }
            (None, None, Some(delegated)) => {
                Ok(StartupAdmissionBinding::Delegated(delegated.clone()))
            }
            _ => Err(EntryTransportError::Malformed),
        }
    }
}

impl PreparedStartupSnapshot {
    /// Transfer only the sealed child-side authenticated recovery source.
    pub(crate) fn delegated(source: crate::recovery_supervisor::startup::SourceRef) -> Self {
        let length = source.length;
        Self { launch: None, registered: None, delegated: Some(source), body: 0..length }
    }
}

impl PreparedStartupSnapshot {
    /// Borrow the genuine paired-source loan; ordinary Entry snapshots cannot supply it.
    #[cfg(windows)]
    pub(crate) fn recovery_source(
        &self,
    ) -> Option<&crate::recovery_supervisor::startup::SourceRef> {
        if self.launch.is_none() && self.registered.is_none() {
            self.delegated.as_ref()
        } else {
            None
        }
    }
}
