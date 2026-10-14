// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve a direct native no-queue cutoff rejection for the exact authored finite write.
use super::super::requests::Purpose;
use super::*;
use std::fmt;
/// Genuine synchronous native queue rejection retains its unchanged scope and typed cause.
/// Construction validates the actual returned original encoded buffer before returning its pool loan.
#[derive(Debug)]
pub(in crate::production::child) struct NativeWriteExpiry {
    pub(in crate::production::child) header: WireHeader,
    pub(in crate::production::child) original: Moment,
    pub(in crate::production::child) purpose: Purpose,
    cause: LifecyclePipeError,
}
impl fmt::Display for NativeWriteExpiry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Original finite write cutoff expired before native queue admission")
    }
}
impl Error for NativeWriteExpiry {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}
impl NativeWriteExpiry {
    /// Project only this original synchronous DeadlineExpired; other errors remain unchanged.
    pub(super) fn from_returned(
        message: &WireMessage<'_>,
        original: Moment,
        error: LifecyclePipeError,
        bytes: &[u8],
    ) -> Result<Self> {
        if error != LifecyclePipeError::DeadlineExpired {
            return Err(Box::new(error));
        }
        let decoded = FrameCodec::decode(bytes)?;
        if decoded.header != message.header {
            return Err("Rejected original native write header changed".into());
        }
        let purpose = match (&message.payload, decoded.payload) {
            (
                WirePayload::Acquire { identity: expected, .. },
                sugarsubstitute_supervision::lifecycle_protocol::DecodedPayload::Acquire {
                    identity,
                    ..
                },
            ) if *expected == identity => Purpose::Acquisition,
            (
                WirePayload::OutputPull { lease: expected, acknowledged_delivery: expected_ack },
                sugarsubstitute_supervision::lifecycle_protocol::DecodedPayload::OutputPull {
                    lease,
                    acknowledged_delivery,
                },
            ) if *expected == lease && *expected_ack == acknowledged_delivery => {
                Purpose::OutputPull
            }
            _ => return Err(Box::new(error)),
        };
        Ok(Self { header: message.header, original, purpose, cause: error })
    }
}

#[cfg(test)]
#[path = "expiry_tests.rs"]
mod tests;
