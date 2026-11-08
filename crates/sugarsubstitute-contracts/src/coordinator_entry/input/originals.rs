// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Name a complete immutable invocation snapshot without carrying paths or native authority.
use super::EntryShapeError;

/// SHA-256 commitment to one version-one passive originals manifest.
///
/// This fixed matching value supplies no path, handle, installation ownership or storage
/// capability. The receiver resolves its own protected namespace and independently validates
/// native provenance, captured bytes and authenticated invocation before using a snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntryOriginalsReference([u8; 32]);
impl EntryOriginalsReference {
    /// Retain a complete nonzero digest without allocating or inspecting storage.
    /// # Errors
    /// Rejects the reserved all-zero value through the existing inert identity-shape error.
    pub fn new(manifest_sha256: [u8; 32]) -> Result<Self, EntryShapeError> {
        if manifest_sha256 == [0; 32] {
            return Err(EntryShapeError::Identity);
        }
        Ok(Self(manifest_sha256))
    }
    /// Borrow the complete immutable-manifest commitment; it grants no resolver authority.
    pub const fn manifest_sha256(&self) -> &[u8; 32] {
        &self.0
    }
}
