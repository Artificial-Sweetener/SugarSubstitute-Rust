// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire only a direct original-cutoff rejection that returned the never-issued allocation.
use super::super::{ParentError, ParentLifecycle, Reply, Result, types::WritePurpose};
use sha2::{Digest, Sha256};
use sugarsubstitute_contracts::execution::Moment;
use sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeError;
use sugarsubstitute_supervision::lifecycle_protocol::{DecodedPayload, FrameCodec, ReplyOutcome};

/// Preserve the actual first write rejection and a failed later clock observation.
/// Both remain strict; neither failure establishes recoverable original expiry.
#[derive(Debug)]
pub(super) struct RejectionObservationFailure {
    pub(super) rejected: LifecyclePipeError,
    pub(super) observation: super::super::Cause,
}
impl std::fmt::Display for RejectionObservationFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Original write rejected with {:?}; clock observation failed",
            self.rejected
        )
    }
}
impl std::error::Error for RejectionObservationFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.observation.as_ref())
    }
}

impl ParentLifecycle {
    /// Called exclusively for the actual control.write rejection and its returned
    /// original allocation. A caller-supplied label cannot cancel issued/uncertain I/O.
    /// Header, original cutoff, semantic purpose and exact output source must match;
    /// disposal returns the existing buffer slot and manufactures no Written/ACK fact.
    pub(in super::super) fn retire_unissued_expired_write(
        &mut self,
        purpose: WritePurpose,
        until: Moment,
        cause: LifecyclePipeError,
        bytes: &[u8],
        original_digest: [u8; 32],
        now: Moment,
    ) -> Result<bool> {
        if cause != LifecyclePipeError::DeadlineExpired || self.write.is_some() {
            return Ok(false);
        }
        let returned_digest: [u8; 32] = Sha256::digest(bytes).into();
        if returned_digest != original_digest {
            return Ok(false);
        }
        let Some(acquisition) = &self.acquisition else { return Ok(false) };
        if until != acquisition.original || now < acquisition.original || acquisition.write_active()
        {
            return Ok(false);
        }
        let Ok(decoded) = FrameCodec::decode(bytes) else { return Ok(false) };
        if decoded.header.run != acquisition.header.run
            || decoded.header.connection != acquisition.header.connection
        {
            return Ok(false);
        }
        let DecodedPayload::Reply(actual) = decoded.payload else { return Ok(false) };
        let original = acquisition.header.request.ok_or(ParentError::WrongPhase)?;
        let index = match purpose {
            WritePurpose::AcquisitionProgress(header) => {
                if decoded.header != header
                    || header != acquisition.header
                    || acquisition.progress().is_none_or(|lease| {
                        actual != ReplyOutcome::Pending(super::super::replies::reference(lease))
                    })
                {
                    return Ok(false);
                }
                None
            }
            WritePurpose::Reply(index) => {
                let record = self.requests.get(index)?;
                if decoded.header != record.header
                    || record.until != Some(until)
                    || !self.requests.belongs_to(index, original)?
                    || !matches_reply(record.reply, actual, &record.output_bytes)
                {
                    return Ok(false);
                }
                Some(index)
            }
            _ => return Ok(false),
        };
        self.expire_acquisition(now)?;
        let acquisition = self.acquisition.as_mut().ok_or(ParentError::WrongPhase)?;
        if !acquisition.retiring() {
            return Ok(false);
        }
        acquisition.retain_write_cutoff(cause)?;
        if let Some(index) = index {
            self.requests.abandon(index)?;
        }
        Ok(true)
    }
}

/// Compare the existing encoded reply's complete correlation without readiness effects.
fn matches_reply(expected: Option<Reply>, actual: ReplyOutcome<'_>, bytes: &[u8]) -> bool {
    use super::super::replies::reference;
    match (expected, actual) {
        (Some(Reply::Granted { lease, identity }), ReplyOutcome::Granted(context)) => {
            context.lease == reference(lease) && context.identity == identity
        }
        (Some(Reply::Failed(expected)), ReplyOutcome::Failed(actual)) => expected == actual,
        (Some(Reply::Rejected(expected)), ReplyOutcome::Rejected(actual)) => expected == actual,
        (Some(Reply::Cancelled), ReplyOutcome::Cancelled) => true,
        (Some(Reply::OutputIdle(lease)), ReplyOutcome::OutputIdle(actual)) => {
            reference(lease) == actual
        }
        (Some(Reply::Output(expected)), ReplyOutcome::OutputDelivery(actual)) => {
            actual.lease == reference(expected.lease)
                && actual.delivery_id == expected.id
                && actual.stream == expected.stream
                && actual.sequence == expected.sequence
                && actual.gap == expected.gap
                && actual.end == expected.end
                && bytes.get(..expected.count) == Some(actual.bytes)
        }
        _ => false,
    }
}
