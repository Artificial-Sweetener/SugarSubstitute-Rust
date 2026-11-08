// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Classify originals before native acquisition without confusing workflow and attachment lives.
use super::*;
use std::collections::BTreeSet;
use sugarsubstitute_contracts::execution::MemoryClaim;

const MAX_RECORDED_OUTPUTS: usize = 65_536;
pub(super) const MEMORY_BYTES: u64 = 8 * 1024 * 1024;

/// Current-webview recognition includes removed originals; identities never age into live I/O.
pub(super) struct RecordedOutputs {
    ids: BTreeSet<[u8; 36]>,
    saturated: bool,
    _memory: MemoryClaim,
}
impl RecordedOutputs {
    /// Reserve the bounded fixed-key index from the Driver's existing owned allocation.
    pub(super) fn new(memory: &MemoryLease) -> Result<Self, StoreError> {
        Ok(Self {
            ids: BTreeSet::new(),
            saturated: false,
            _memory: memory.claim(MEMORY_BYTES).map_err(io::Error::other)?,
        })
    }
    /// Adopt only validated loaded/published IDs. Saturation refuses unknown native sources
    /// rather than forgetting a removed original and granting an unsigned transfer.
    pub(super) fn remember(&mut self, checkpoint: &SessionCheckpoint) -> Result<(), StoreError> {
        for output in checkpoint.entries.iter().flat_map(|entry| &entry.outputs) {
            self.remember_output(&output.id)?;
        }
        Ok(())
    }
    /// Retain one validated durable identity without permitting admission above the fixed bound.
    pub(super) fn remember_output(&mut self, id: &str) -> Result<(), StoreError> {
        let key: [u8; 36] = id.as_bytes().try_into().map_err(|_| SessionError::Invalid)?;
        if !self.ids.contains(&key) {
            if self.ids.len() == MAX_RECORDED_OUTPUTS {
                if !self.saturated {
                    tracing::warn!(
                        operation = "workspace_native_identity_saturated",
                        limit = MAX_RECORDED_OUTPUTS,
                        "Unknown native originals unavailable; recorded identities remain protected"
                    );
                }
                self.saturated = true;
            } else {
                self.ids.insert(key);
            }
        }
        Ok(())
    }
    /// Classification alone grants no bytes, native binding, or filesystem authority.
    #[cfg(any(feature = "native-composition", test))]
    fn recognizes(&self, id: &str) -> bool {
        self.saturated
            || <[u8; 36]>::try_from(id.as_bytes()).is_ok_and(|key| self.ids.contains(&key))
    }
}

/// The sole native fallback decision, consumed directly by the root source adapter.
#[cfg(all(windows, any(feature = "native-composition", test)))]
#[derive(Debug, PartialEq)]
pub(crate) enum NativeOriginalResolution {
    /// A genuinely unrecorded live output retains the existing backend path.
    Live,
    /// Known/retired/saturated identities have no current original admission.
    Unavailable,
    /// Immutable original identity under the separately accepted current workflow binding.
    Verified { binding: SessionBinding, digest: String, bytes: u64, asset: Option<SessionAsset> },
}

#[cfg(all(windows, any(feature = "native-composition", test)))]
impl WorkspaceDriver {
    /// Classify identity before address/binding lookup. Removed and saturated unknown IDs
    /// must never enter the existing unsigned live backend transfer path.
    pub(crate) fn requires_verified_original(&self, output_id: &str) -> bool {
        self.originals.recognizes(output_id)
    }
    /// Correlate only a known pending restore selection; no original can be acquired before ACK.
    pub(crate) fn pending_native_original(
        &self,
        output_id: &str,
        address: &sugarsubstitute_contracts::canvas_surface::CanvasSurfaceAddress,
    ) -> bool {
        self.restore.as_ref().is_some_and(|restore| restore.sent)
            && !self.policy.live()
            && self.native_entry(output_id, address).is_some()
    }

    /// Resolve all native original/asset paths through one recorded-ID and workflow-fence owner.
    /// The native surface independently admits the complete attachment address before I/O.
    pub(crate) fn resolve_native_original(
        &self,
        output_id: &str,
        address: &sugarsubstitute_contracts::canvas_surface::CanvasSurfaceAddress,
        file: &sugarsubstitute_contracts::canvas_surface::CanvasFileReference,
    ) -> NativeOriginalResolution {
        if !self.requires_verified_original(output_id) {
            return NativeOriginalResolution::Live;
        }
        if self.replay_retired && self.published.is_none() {
            return NativeOriginalResolution::Unavailable;
        }
        let Some((binding, output, checkpoint)) = self.native_entry(output_id, address) else {
            return NativeOriginalResolution::Unavailable;
        };
        let accepted = if self.published.is_some() {
            self.last_ack.as_ref().map(|ack| ack.3.as_slice()).unwrap_or(&[])
        } else {
            self.restore
                .as_ref()
                .and_then(|restore| restore.bindings.as_deref())
                .unwrap_or(&self.media_bindings)
        };
        if !accepted.iter().any(|known| {
            same_runtime(known, binding)
                && binding.graph_revision >= known.graph_revision
                && binding.source_revision >= known.source_revision
                && binding.navigation_revision >= known.navigation_revision
        }) {
            return NativeOriginalResolution::Unavailable;
        }
        use sugarsubstitute_contracts::canvas_surface::CanvasFileStorage;
        let storage = match file.storage() {
            CanvasFileStorage::Input => "input",
            CanvasFileStorage::Output => "output",
            CanvasFileStorage::Temporary => "temp",
        };
        let exact_descriptor = output.file.filename == file.filename()
            && output.file.subfolder == file.subfolder()
            && output.file.storage == storage;
        let asset = exact_descriptor
            .then_some(output.asset_digest.as_ref())
            .flatten()
            .and_then(|digest| checkpoint.assets.iter().find(|asset| &asset.digest == digest))
            .cloned();
        NativeOriginalResolution::Verified {
            binding: binding.clone(),
            digest: output.digest.clone(),
            bytes: output.bytes,
            asset,
        }
    }

    /// Consume the existing SugarCubes canvasSessionId(workflow) mapping exactly. Workflow
    /// generation belongs to accepted SessionBinding; attachment generation is never compared.
    fn native_entry(
        &self,
        output_id: &str,
        address: &sugarsubstitute_contracts::canvas_surface::CanvasSurfaceAddress,
    ) -> Option<(&SessionBinding, &SessionOutput, &SessionCheckpoint)> {
        let binding = self.policy.fence().bindings.iter().find(|binding| {
            address.session_id() == format!("canvas:{}", binding.runtime_session)
        })?;
        let checkpoint = self.published.as_ref().map(|value| &value.checkpoint).or_else(|| {
            self.restored.as_ref()?.checkpoint.as_ref().map(|(_, checkpoint)| checkpoint)
        })?;
        let output = checkpoint
            .entries
            .iter()
            .find(|entry| entry.id == binding.entry_id)?
            .outputs
            .iter()
            .find(|output| output.id == output_id)?;
        Some((binding, output, checkpoint))
    }
}
