// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Join authenticated archive custody to fresh extraction or already pinned application bytes.
use super::*;

/// Materialization input for a newly authenticated application archive.
/// Retained payloads share immutable file custody, never just a directory or claimed digest.
pub enum BundleSource<'a> {
    /// Extract into this workspace and verify the resulting complete distribution.
    Extract(&'a Path),
    /// Reuse bytes verified earlier by this process after matching the signed archive inventory.
    Retained(Arc<VerifiedBundle>),
}
impl BundleSource<'_> {
    /// Bind only exact authenticated inventory to retained files; keep the new archive alive.
    /// Current installation authority must be checked before and after this storage operation.
    pub(in crate::setup_storage) fn bind(
        self,
        mut download: super::super::VerifiedDownload,
        trust: &super::super::TrustStore,
        context: &JobContext,
    ) -> Result<VerifiedBundle, super::super::TrustFailure> {
        match self {
            Self::Extract(workspace) => {
                let cache = trust.workspace_cache(workspace, context)?;
                Ok(VerifiedBundle::from_download(download, &cache, context)?)
            }
            Self::Retained(bundle) => {
                super::super::packed::require_manifest(
                    &mut download,
                    &bundle.payload.manifest_bytes,
                    context,
                )?;
                Ok(VerifiedBundle {
                    payload: Arc::clone(&bundle.payload),
                    download: Some(download),
                })
            }
        }
    }
}
