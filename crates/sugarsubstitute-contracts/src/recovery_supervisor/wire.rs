// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Strict purpose-specific recovery framing; decoded bytes never authenticate their sender.
mod codec;
mod fields;
mod key;
mod payload;
#[cfg(test)]
mod tests;
use super::*;
use crate::{execution::CounterDeadline, incident::DurableAcknowledgement};
pub use codec::{decode, encode, encode_capture_root_units};
pub use key::{KEY_BYTES, decode as decode_key, encode as encode_key};
/// Exact largest complete native recovery pipe message, in bytes.
pub const RECOVERY_WIRE_BYTES: usize = 65536;
/// Fixed schema1 correlation envelope, in bytes.
pub const RECOVERY_HEADER_BYTES: usize = 192;
/// Largest immutable original Launch body from the existing supported Entry producer.
pub const RECOVERY_STARTUP_BYTES: usize = 65416;
/// Largest configured local capture-root path in little-endian UTF16 bytes, without NUL.
pub const RECOVERY_ROOT_BYTES: usize = 32766 * 2;
/// Root path fragment bound after its exact identity and total/offset fields.
pub const RECOVERY_ROOT_CHUNK: usize = RECOVERY_WIRE_BYTES - RECOVERY_HEADER_BYTES - 32;
/// Largest startup fragment after its exact offset/total fields.
pub const RECOVERY_STARTUP_CHUNK: usize = RECOVERY_WIRE_BYTES - RECOVERY_HEADER_BYTES - 8;
/// Intended authenticated receiving role; a purpose or direction mismatch rejects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryReceiver {
    /// The sole surviving coordinator/backend/profile authority.
    Coordinator,
    /// The fresh subordinate with one exactly granted successor family.
    Supervisor,
}
/// Closed recovery commands; they confer no coordinator election, resource or process authority.
pub enum RecoveryCommand<'a> {
    /// Bind this exact inherited session capability before receiving any grant/startup data.
    Hello([u8; 32]),
    /// Borrow bytes from the retained actual registered startup, never a path/environment token.
    Startup {
        /// Original full Launch body length.
        total: u32,
        /// Exact next body offset, starting at zero.
        offset: u32,
        /// Original body fragment; receiver retains its admitted assembly destination.
        bytes: &'a [u8],
    },
    /// Actual Root's protected capture location/identity, independently reverified by the fresh process.
    CaptureRoot {
        /// Actual pinned Root directory identity; matching bytes alone confer no path privilege.
        identity: [u8; 24],
        /// Complete even UTF16 byte extent without terminator, at most65532.
        total: u32,
        /// Exact next even byte offset starting at zero.
        offset: u32,
        /// Original configured Root path encoded as little-endian UTF16 units.
        units_le: &'a [u8],
    },
    /// Exact Root-assigned Desktop registration identity for the granted run.
    DesktopIdentity {
        /// Original Root Record launch identity.
        launch: crate::startup::LaunchId,
        /// Original Root Record navigation generation.
        navigation: crate::execution::Generation,
    },
    /// Forward the genuine committed original report acknowledgement.
    Commit(DurableAcknowledgement),
    /// Actual receiver has completed its bounded authenticated startup adoption.
    StartupAccepted,
    /// Actual fresh registration observations; Root independently owns/verifies process watches.
    CaptureRegistered {
        /// Exact authenticated supervisor process.
        supervisor: RecoveryProcessIdentity,
        /// Exact registered handler process.
        handler: RecoveryProcessIdentity,
    },
    /// Actual UI child creation; this is distinct from carrier creation and capture readiness.
    DesktopCreated(RecoveryProcessIdentity),
    /// Root independently opened/watched this exact suspended child and verified outer-family membership.
    /// The supervisor may resume only its identical held child after this reply.
    DesktopEnrolled(RecoveryProcessIdentity),
    /// Actual child terminal observation, independent of full family/profile retirement.
    DesktopExited {
        /// Exact observed UI process identity.
        process: RecoveryProcessIdentity,
        /// Actual native exit status, including259 if genuinely terminal.
        status: u32,
    },
    /// Same original native cleanup counter deadline, never a renewable local duration.
    Stop(CounterDeadline),
    /// Advisory subordinate completion; Root still independently proves physical retirement.
    Retired,
    /// Finite failure category; each process retains its original native/provider causes locally.
    Failed(RecoveryFailureCode),
}
/// Borrow one canonically decoded complete native message without allocating or granting privilege.
pub struct RecoveryFrame<'a> {
    /// Exact original report/run/delegation correlation.
    pub key: RecoveryGrantKey,
    /// Positive strictly increasing transport sequence; replay policy belongs to the session owner.
    pub sequence: u64,
    /// Purpose-specific operation with borrowed bounded payloads.
    pub command: RecoveryCommand<'a>,
}
/// Reject malformed schemas and directions without retaining secret capabilities or body bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryWireError {
    /// Unsupported schema, command, reserved bytes, size or payload grammar.
    Malformed,
    /// Original grant/report/run shape is invalid.
    Binding,
    /// The authenticated receiver cannot consume this command direction.
    Direction,
    /// A bounded destination or original payload range is insufficient.
    Capacity,
}
impl std::fmt::Display for RecoveryWireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Recovery wire rejected: {self:?}")
    }
}
impl std::error::Error for RecoveryWireError {}
impl std::fmt::Debug for RecoveryFrame<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecoveryFrame { redacted }")
    }
}
