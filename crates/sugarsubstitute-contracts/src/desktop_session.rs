// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Share desktop checkpoint values independently of storage, Comfy and native adapters.
//!
//! V1 accepts only v1. Storage adapters must preserve rejected/newer originals and the last good checkpoint;
//! they must not replace an unsupported checkpoint with an empty autosave. Relative session
//! names are below the installation layout owner's `InstallationLayout::user()` data root.
//! Original outputs and referenced assets are authoritative data without a TTL. Storage,
//! chunk assembly/digest verification, cancellation and shutdown belong to application owners.
//! DTO shape checks grant no filesystem, origin, process or native authority.

mod document;
mod identity;
mod navigation;
mod placement;
mod primitives;
mod protocol;
mod scenes;
mod snapshot;
mod submissions;
/// Schema-derived untrusted JSON values; semantic validators must admit them before use.
pub mod wire;

pub use crate::{configuration::ProfileId, installation::InstallationId};
pub use document::validate_document;
pub use identity::{
    BackendNamespaceId, CheckpointId, OutputId, RunId, SceneGroupId, SceneId, SourceId,
    WorkspaceEntryId, WorkspaceId,
};
pub use placement::validate_placement;
pub use protocol::{admit_operation, decode_message, encode_message, same_fence, validate_message};
pub use snapshot::{decode_checkpoint, encode_checkpoint, validate_checkpoint};

use std::{error::Error, fmt};

/// Maximum encoded UTF-8 checkpoint bytes, charged by the execution/storage adapter.
pub const MAX_CHECKPOINT_BYTES: usize = 128 * 1024 * 1024;
/// Maximum encoded UTF-8 document envelope, including current and saved baseline graphs.
pub const MAX_DOCUMENT_BYTES: usize = 32 * 1024 * 1024;
/// Maximum encoded UTF-8 message bytes, independently of canvas/panel limits.
pub const MAX_MESSAGE_BYTES: usize = 512 * 1024;
/// Maximum raw UTF-8 bytes in each ordered session-transfer chunk.
pub const MAX_CHUNK_BYTES: usize = 64 * 1024;
/// Maximum authoritative temporary-final asset bytes per profile namespace.
pub const MAX_ASSET_BYTES: u64 = 8 * 1024 * 1024 * 1024;

/// Fail-closed contract errors; UI adapters localize machine-readable reasons.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionError {
    /// Malformed shape, identity, membership, bounds or coordinates.
    Invalid,
    /// Encoded bytes exceed the contract's admission limit.
    TooLarge,
    /// Message origin differs from the independently authorized current origin.
    UntrustedOrigin,
    /// Scope, runtime identity or revision no longer matches the current owner.
    Stale,
    /// A peer has not advertised the requested capability.
    Unsupported,
    /// Comfy has not reached its committed workflow boundary.
    NotCommitted,
    /// Workflow persistence is explicitly disabled by the user.
    OptedOut,
}
impl fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Desktop session contract: {self:?}")
    }
}
impl Error for SessionError {}

/// Name the profile namespace relative to `InstallationLayout::user()`, with no IO authority.
pub fn session_directory(profile: &ProfileId) -> String {
    format!("sessions/{}/", profile.as_str())
}

#[cfg(test)]
mod tests;
