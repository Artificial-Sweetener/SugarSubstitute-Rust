// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit complete application distributions using shared verified payload custody.
mod payload;
mod prepared;
#[cfg(test)]
mod tests;
use crate::durable_storage::files::{self, Directory};
pub(super) use payload::Payload;
pub(super) use prepared::VerifiedCopy;
use std::{
    io,
    path::{Path, PathBuf},
};
use sugarsubstitute_contracts::{execution::JobContext, update::Component};

/// Complete application distribution with authenticated and pinned source files.
/// Updater-only packages have a distinct owner and cannot enter application provisioning.
pub struct VerifiedBundle {
    pub(super) payload: Payload,
}
impl VerifiedBundle {
    /// Bind the immutable source to native creation before any worker can read its files.
    pub(crate) fn bind_native_cache(
        &self,
        family: super::cache_custody::CacheFamilyId,
        context: &JobContext,
    ) -> io::Result<Option<super::cache::NativeCachePin>> {
        self.payload.bind_native_cache(family, context)
    }

    /// Extract an authenticated application download and retain exact archive and payload custody.
    /// Reserve 24 MiB alongside retained download and cache-scope admission on a storage worker.
    /// # Errors
    /// Rejects incomplete application packages, altered bytes, unsafe paths and cancellation.
    pub fn from_download(
        source: super::VerifiedDownload,
        cache: &super::cache::RuntimeCache,
        context: &JobContext,
    ) -> io::Result<Self> {
        Ok(Self {
            payload: Payload::from_download(source, cache, Component::Application, context)?,
        })
    }
    /// Verify a full distribution against the manifest hash compiled into its trusted installer.
    /// Execute on an admitted storage worker, with `RESERVATION_BYTES` reserved.
    /// # Errors
    /// Rejects malformed manifests, incomplete distributions, changed bytes and unsafe paths.
    pub fn open(root: &Path, expected: &str, context: &JobContext) -> io::Result<Self> {
        Ok(Self { payload: Payload::open(root, expected, Component::Application, context)? })
    }
    /// Original authenticated archive identity; this does not grant activation authority.
    pub fn download_identity(&self) -> Option<&sugarsubstitute_update::VerifiedBytes> {
        self.payload.download_identity()
    }
    /// Exact provisioning paths retained under this distribution's filesystem custody.
    pub fn worker_paths(&self) -> (PathBuf, PathBuf) {
        (self.payload.root.join("electron/electron.exe"), self.payload.root.join("worker"))
    }
    /// Authenticated manifest digest used by installation generation identity.
    pub fn digest(&self) -> &str {
        self.payload.digest()
    }
    /// Authenticated distribution release label.
    pub fn release(&self) -> &str {
        self.payload.release()
    }
    /// Require entry support before publishing an application trial.
    pub(super) fn require_installed_launch(&self) -> io::Result<()> {
        self.payload.compatibility.require_installed_launch()
    }
    /// Retain the authenticated inventory with the installation so accepted launches need no cache.
    pub(in crate::setup_storage) fn retain_inventory(
        &self,
        target: &Path,
        context: &JobContext,
    ) -> io::Result<()> {
        crate::durable_storage::protection::with(context, |protection| {
            files::write_immutable(
                &target.join(super::inventory::INSTALLED_MANIFEST),
                &self.payload.manifest_bytes,
                |path| protection.file(path, context),
            )?;
            let directory = Directory::open(target, false)?;
            protection.verify_file(&directory.read(super::inventory::INSTALLED_MANIFEST)?)
        })
    }
}
