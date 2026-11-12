// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode bounded, untrusted lifecycle messages independently of native authentication.
//!
//! Wire version 14 uses little-endian integers and a fixed 104-byte header. Kind21
//! reports the policy-accepted native main reveal after frontend readiness. The parent
//! rechecks its retained profile/browser authorization before accepting that observation.
//! Registration reply tag17 carries explicit parent-derived backend execution policy before
//! child configuration projection. Granted/Reloaded repeat that same closed policy. Receiving
//! these inert facts never grants installation, backend or filesystem custody.
//! Earlier wire versions are rejected; all processes in a launch use the same distribution.
//! Managed
//! Acquire/Retry headers bound admission; accepted requests await the owned backend
//! until Ready, cancellation or exit. Every native transfer remains finite. Frames
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
//! Acquire starts with positive u64 launch/navigation. Managed tag 1 carries
//! u16 port/u8 argument count, then root and
//! interpreter followed by every argument. Each string has a u32 UTF-16-unit
//! count and exact UTF-16LE bytes without a terminator. Attached tag 2 carries
//! u8 host (1 IPv4, 2 IPv6, 3 localhost)/u16 port. Paths have at most 32766 units;
//! argument counts and UTF-8 byte budgets match existing launch contracts.
//! Reload carries its lease reference followed by positive u64 launch/navigation.
//! Kinds 12..18 are RequestedProfileFolder, EnvironmentPrepared, ControllerPrepared,
//! MainRetired, NativeActivity, OutputPull and MainFailed. Main payloads reuse the
//! header's full run and carry positive u64 launch/navigation. Folder fragments carry
//! u32 total/offset/count and exact raw UTF16LE, including unmatched surrogates.
//! Canonical offsets are 0 or 16384, counts at most16384 and totals at most32766.
//! ControllerPrepared adds a positive u32 PID. MainFailed adds u8 stage/failure.
//! NativeActivity carries positive u64 revision/u8 activity (1 active,2 inactive,
//! 3 native-acknowledged suspension claim); OutputPull carries lease/u64 prior
//! consumed delivery (0 absent). Decoding alone authenticates none of these facts.
//! Reply tags 1..17 are documented on `ReplyOutcome`. Granted tag3 includes lease,
//! main identity, u8 loopback host/u16 port, two u8-counted ASCII version labels,
//! and the verified listener's u32 PID and u64 creation time in 100 ns units.
//! Reloaded tag5 carries the same context after fresh source inspection/HTTP verification.
//! Both grant forms end with the canonical backend identity SHA256 (32 raw bytes),
//! installation/source correlation (32 lowercase ASCII hex bytes), then one policy byte
//! (1 external, 2 installation-owned). These are matching/execution data only. Registered
//! tag17 carries just that policy byte before child configuration projection; subsequent
//! grants repeat it. Missing, unknown and earlier-version contexts are rejected.
//! Version6 lease-only reload replies are rejected by all coordinated native peers.
//! Labels satisfy the shared readiness grammar and contain at most64 bytes.
//! OutputDelivery carries lease/u64 delivery/u8 stream/u64 stream sequence/u8 gap
//! presence (optional u64 dropped bytes/skipped batches)/u8 EOF/u32 raw-byte count.
//! Raw bytes contain at most32768 bytes. EOF and transport delivery never prove
//! ingestion, native retirement or process settlement. Stage tags1..5 and failure
//! tags1..6 are documented on their shape-only enums. Failure codes 1..4 mean
//! Preparation, Cleanup, UnexpectedExit and Deadline. Rejection codes 1..10 mean
//! ForeignOwner, StaleIdentity, ProfileContention, WrongPhase, ChangedDuplicate,
//! StaleRequest, Overloaded, IdentityExhausted, InvalidClock and EndpointMismatch.
//! Kind19 SettleAttempt carries the original positive u64 launch/navigation and
//! Acquire request ID. The authenticated parent retains the resource association
//! and acknowledges only real cleanup settlement. Kind20 RetryBackend carries the
//! exact Main identity and failed lease reference (40 payload bytes). Reply tag15
//! BackendUnavailable carries failed lease/u32 status/u16 affected clients/u8 retry
//! readiness (32 bytes including tag); client count is 1..64 and readiness is 0/1.
//! Tag16 BackendReplaced carries Main identity/previous lease/current lease (65
//! bytes including tag). Replacement may answer an exact pending RetryBackend or
//! the existing OutputPull; the authenticated request owner issues one reply only.
//! Tag16 conveys an actual historical Ready event and changes only the child's backend
//! binding. Parent checks genuine producer metadata and the original installed selection;
//! an exact successor death may already be retained when this handoff is sent. The child
//! keeps recovery disabled through this reply. Its next authenticated current-lease
//! OutputPull acknowledges consumption: Parent adopts a still-live Ready interest or,
//! after exact new-death and absent-old validation, consumes only its binding ledger.
//! Written alone never proves child adoption. Actual current output/idle permits recovery
//! presentation; an immediate BackendUnavailable names the newly consumed failed binding.
//! Native Reload remains supported for the original Main creation lease. A running
//! successor's native Reload is unimplemented and rejected WrongPhase before effects;
//! a stale original lease gains no release or reload authority. Web reload remains with
//! the frontend owner. These copied wire facts grant no retry, current readiness,
//! native adoption or teardown proof.
//! Kind22 UpdateStatus, sent by the coordinator without a request, carries a positive u64
//! revision, u8 workflow activity (1 idle, 2 running) and a u32-counted encoded status of at
//! most `MAX_UPDATE_STATUS_BYTES`, whose content its contract owner decodes and bounds. Kind23
//! RestartToApply is an empty child request, sent after the close save finished and before
//! ExitIntent; it grants no process authority, and the coordinator relaunches only when the
//! child then exits normally.
//! Only schema14 is accepted; parent and captured child deploy together.
//! Unknown tags and trailing bytes reject; no future operation is implied.
mod binary;
mod decode;
mod encode;
mod errors;
mod frame;
mod main_fields;
mod main_types;
mod output;
mod payload;
mod raw_windows;
mod reply;
mod reply_codec;
mod target;
mod utf16;

pub use errors::ProtocolError;
pub use frame::{DecodedFrame, FrameCodec, WireHeader, WireMessage};
pub use main_types::{
    BackendPeer, GrantedContext, MainFailureCode, MainStage, MainStageFailure, VersionLabel,
};
pub use output::OutputDelivery;
pub use payload::{CapabilityRef, DecodedPayload, LeaseReference, WirePayload};
pub use raw_windows::{
    FolderFragment, MAX_FOLDER_FRAGMENT_UNITS, MAX_PROFILE_FOLDER_UNITS, RawWindowsUnits,
};
pub use reply::ReplyOutcome;
pub use target::{BorrowedBackendTarget, DecodedTarget, ManagedTargetRef};
pub use utf16::{ArgumentRefs, Utf16Text};

/// Exact supported wire version; unknown versions reject without negotiation.
pub const WIRE_VERSION: u16 = 14;
/// Exact header byte count, including the whole claimed run and deadline.
pub const HEADER_BYTES: usize = 104;
/// Maximum complete control frame, including header and payload.
pub const MAX_FRAME_BYTES: usize = 65_536;
/// Largest encoded update status a Kind22 frame carries.
pub const MAX_UPDATE_STATUS_BYTES: usize = 32_768;

#[cfg(test)]
mod tests;
