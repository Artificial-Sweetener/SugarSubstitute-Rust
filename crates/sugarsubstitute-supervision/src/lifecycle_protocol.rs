// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode bounded, untrusted lifecycle messages independently of native authentication.
//!
//! Wire version 1 uses little-endian integers and a fixed 104-byte header. Frames
//! contain at most 65536 bytes including that header. Claimed identities, secrets
//! and completion data establish no peer, process, filesystem or UI authority.
//! Native channel owners authenticate peers and separately admit requests before
//! converting borrowed payloads into domain-owned values or causing effects.
//! Absolute positive Windows QPC deadlines are opaque here; clock frequency and
//! monotonic translation belong to the native owner, never serialized `Moment`.
//!
//! Header offsets: `magic[4]` at 0; `version:u16` at 4; `header_bytes:u16` at 6;
//! `frame_bytes:u32` at 8 (complete supplied length); `kind:u16` at 12;
//! `reserved:u16=0` at 14; `namespace[32]` at 16; `epoch[16]` at 48;
//! `instance:u64` at 64; `generation:u64` at 72; `connection:u64` at 80;
//! `request:u64` at 88; `deadline_qpc:i64` at 96. Zero denotes an absent connection
//! or request only for the kind-specific cases documented on `WireHeader`.
//!
//! Kinds 1..11 are Hello, Welcome, Register, Acquire, Release, Reload, ExitIntent,
//! ExitCompleted, UiChallenge, UiResponse and Reply. Payloads have no padding:
//! Hello=32 bytes; Welcome=i64 frequency; Register=empty; lease reference=three
//! u64 values (resource, generation, lease); intent=u64 shutdown/u8 intent/u32
//! status; completion=u64 shutdown; challenge/response=u64 challenge.
//! Intent tags 1..3 mean Normal, StartupCancelled and StartupFailed.
//! Acquire managed tag 1 carries u16 port/u8 argument count, then root and
//! interpreter followed by every argument. Each string has a u32 UTF-16-unit
//! count and exact UTF-16LE bytes without a terminator. Attached tag 2 carries
//! u8 host (1 IPv4, 2 IPv6, 3 localhost)/u16 port. Paths have at most 32766 units;
//! argument counts and UTF-8 byte budgets match existing launch contracts.
//! Reply tags 1..8 are documented on `ReplyOutcome`. Failure codes 1..4 mean
//! Preparation, Cleanup, UnexpectedExit and Deadline. Rejection codes 1..10 mean
//! ForeignOwner, StaleIdentity, ProfileContention, WrongPhase, ChangedDuplicate,
//! StaleRequest, Overloaded, IdentityExhausted, InvalidClock and EndpointMismatch.
//! Unknown tags and trailing bytes reject; no future operation is implied.
mod binary;
mod decode;
mod encode;
mod errors;
mod frame;
mod payload;
mod reply;
mod target;
mod utf16;

pub use errors::ProtocolError;
pub use frame::{DecodedFrame, FrameCodec, WireHeader, WireMessage};
pub use payload::{CapabilityRef, DecodedPayload, LeaseReference, WirePayload};
pub use reply::ReplyOutcome;
pub use target::{DecodedTarget, ManagedTargetRef};
pub use utf16::{ArgumentRefs, Utf16Text};

/// Exact supported wire version; unknown versions reject without negotiation.
pub const WIRE_VERSION: u16 = 1;
/// Exact header byte count, including the whole claimed run and deadline.
pub const HEADER_BYTES: usize = 104;
/// Maximum complete control frame, including header and payload.
pub const MAX_FRAME_BYTES: usize = 65_536;

#[cfg(test)]
mod tests;
