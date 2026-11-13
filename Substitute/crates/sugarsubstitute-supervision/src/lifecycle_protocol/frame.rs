// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Define explicit claimed header identities and allocation-free codec entry points.
use super::{DecodedPayload, ProtocolError, WirePayload};
use sugarsubstitute_contracts::lifecycle::{ConnectionId, RequestId, RunKey};

/// Claimed run/connection/request facts, granting no authentication or admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WireHeader {
    /// Complete claimed coordinator namespace/epoch and desktop instance/generation.
    pub run: RunKey,
    /// Absent only in Hello; otherwise a positive claimed channel identity.
    pub connection: Option<ConnectionId>,
    /// Absent in Hello, Welcome and the server's UiChallenge and UpdateStatus; positive otherwise.
    pub request: Option<RequestId>,
    /// Positive absolute Windows QPC admission deadline, unchanged on retransmission/reply.
    /// Managed Acquire/Retry may await the actual producer beyond admission; each
    /// native transfer has its own finite deadline. Running output subscriptions also
    /// retain correlation while waiting, with bounded reply and parser transfers.
    /// Other requests retain their cutoff.
    pub deadline_qpc: i64,
}
impl WireHeader {
    pub(super) fn validate(self, kind: u16) -> Result<(), ProtocolError> {
        if self.deadline_qpc <= 0 {
            return Err(ProtocolError::Clock);
        }
        let valid = match kind {
            1 => self.connection.is_none() && self.request.is_none(),
            2 | 9 | 22 => self.connection.is_some() && self.request.is_none(),
            3..=8 | 10..=21 | 23 | 24 => self.connection.is_some() && self.request.is_some(),
            _ => return Err(ProtocolError::UnsupportedTag),
        };
        if valid { Ok(()) } else { Err(ProtocolError::Identity) }
    }
}
/// Borrow existing supported domain inputs for one exact encoded message.
#[derive(Debug)]
pub struct WireMessage<'a> {
    /// Claimed metadata checked against the operation's identity-presence rules.
    pub header: WireHeader,
    /// One currently supported payload; future operations are not placeholders.
    pub payload: WirePayload<'a>,
}
/// Borrow a complete validated frame without allocating payload storage.
#[derive(Debug)]
pub struct DecodedFrame<'a> {
    /// Claimed metadata, never a trusted client binding.
    pub header: WireHeader,
    /// Borrowed or fixed-size decoded operation data.
    pub payload: DecodedPayload<'a>,
}
/// Provide the sole fixed-schema lifecycle framing implementation.
pub struct FrameCodec;
impl FrameCodec {
    /// Decode one complete frame with no owned payload allocation or native calls.
    /// Unknown schema/tags, malformed lengths, Unicode and trailing bytes reject.
    pub fn decode(source: &[u8]) -> Result<DecodedFrame<'_>, ProtocolError> {
        super::decode::frame(source)
    }
    /// Encode into caller-owned storage after full input/size preflight.
    /// Errors leave output unchanged. Returned size is the exact complete frame;
    /// bytes beyond it remain untouched. No temporary encoded frame is allocated.
    pub fn encode(message: &WireMessage<'_>, output: &mut [u8]) -> Result<usize, ProtocolError> {
        super::encode::frame(message, output)
    }
}
