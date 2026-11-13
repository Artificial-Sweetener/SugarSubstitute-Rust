// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Distinguish durable semantic identities from titles, paths and runtime session keys.
use super::{SessionError, primitives::uuid};
use std::fmt;

/// Generate UUID value types while keeping every authoritative meaning distinct.
macro_rules! identity {
    ($($(#[$doc:meta])* $name:ident),+ $(,)?) => {$ (
        $(#[$doc])*
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);
        impl $name {
            /// Adopt a canonical lowercase non-nil UUID; this grants no access authority.
            /// # Errors
            /// Rejects malformed, noncanonical, nil or unsupported-variant identities.
            pub fn new(value: impl Into<String>) -> Result<Self, SessionError> {
                let value = value.into();
                if !uuid(&value) { return Err(SessionError::Invalid); }
                Ok(Self(value))
            }
            /// Borrow the stable wire spelling.
            pub fn as_str(&self) -> &str { &self.0 }
        }
        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    )+ };
}
identity!(
    /// One desktop workspace within a user profile.
    WorkspaceId,
    /// One open document occurrence; rename preserves it and duplicate allocates another.
    WorkspaceEntryId,
    /// One final artifact occurrence, independent of a filename or node number.
    OutputId,
    /// One explicitly submitted scene within its group; ordinary submissions have no scene.
    SceneId,
    /// One immutable captured checkpoint.
    CheckpointId,
    /// One durable session namespace bound by the application to its admitted BackendTarget.
    /// The application owns its durable mapping to the existing admitted BackendTarget and
    /// reuses it only after independently matching that target. Checkpoint data cannot
    /// establish this mapping; current process/custody grants are acquired afresh.
    /// This is neither a target-selection owner nor native I/O authority.
    BackendNamespaceId,
    /// One accepted generation run, independent of a Comfy prompt identifier.
    RunId,
    /// One frozen output source, independent of a graph node number.
    SourceId,
    /// One ordered scene submission group.
    SceneGroupId,
);
