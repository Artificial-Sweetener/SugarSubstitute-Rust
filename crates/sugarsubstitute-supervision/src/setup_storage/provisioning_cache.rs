// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit delegated provisioning caches and recover them only after their actual native family ends.
use super::cache;
use super::cache::CacheLease;
use super::cache::Namespace;
use super::cache::RuntimeCache;
use super::cache_custody::CacheFamilyId;
use super::check;
use super::invalid;
use crate::durable_storage::files::Directory;
use sha2::{Digest, Sha256};
use std::{ffi::OsString, io};
use sugarsubstitute_contracts::execution::JobContext;
mod metadata;
pub use metadata::{
    MetadataChunk, MetadataRead, MetadataRecord, MetadataWrite, ProvisioningMetadata,
};
const DOWNLOAD_BYTES: u64 = 24 * 1024 * 1024 * 1024;
const METADATA_BYTES: u64 = 256 * 1024 * 1024;
/// Authenticated worker pressure selects disposable grants after actual native settlement.
#[derive(Clone, Copy, Default)]
pub struct ProvisioningCachePressure {
    /// The download byte grant rejected a write before mutation.
    pub downloads: bool,
    /// The reconstructible metadata byte grant rejected a write before mutation.
    pub metadata: bool,
}
const PREFIX: &str = "setup-worker-v1";

/// Writable grants remain durably pinned until every native process in this family has retired.
pub(crate) struct ProvisioningCaches {
    family: CacheFamilyId,
    downloads: CacheLease,
    metadata: ProvisioningMetadata,
    runtime: CacheLease,
}
impl ProvisioningCaches {
    /// Keep upstream semantic keys and byte verification in the authenticated provisioning worker.
    /// Download sidecars retain their own schema across releases; catalogue parsing is isolated by
    /// the authenticated worker distribution, which includes its pinned upstream source identity.
    pub(crate) fn prepare(
        cache: &RuntimeCache,
        distribution: &str,
        context: &JobContext,
    ) -> io::Result<Self> {
        recover_provisioning_cache(cache, context)?;
        let now = cache::utc_seconds()?;
        let family = CacheFamilyId::new(context)?;
        let mut downloads = cache.clone().acquire(
            Namespace::SetupDownloads,
            &cache::storage_key(b"setup-downloads:owner-schema-1:resume-schema-1"),
            DOWNLOAD_BYTES,
            now,
            context,
        )?;
        let mut metadata = cache.clone().acquire(
            Namespace::UpstreamMetadata,
            &cache::storage_key(format!("setup-upstream-metadata-v1:{distribution}").as_bytes()),
            METADATA_BYTES,
            now,
            context,
        )?;
        let mut runtime = cache.clone().fresh(
            Namespace::SetupRuntime,
            &cache::storage_key(format!("setup-runtime-v1:{}", family.encoded()).as_bytes()),
            METADATA_BYTES,
            now,
            context,
        )?;
        downloads.verify_storage(context)?;
        metadata.verify_storage(context)?;
        downloads.begin_generated(
            &descriptor(&downloads, Namespace::SetupDownloads, family)?,
            context,
        )?;
        metadata.begin_generated(
            &descriptor(&metadata, Namespace::UpstreamMetadata, family)?,
            context,
        )?;
        runtime
            .begin_generated(&descriptor(&runtime, Namespace::SetupRuntime, family)?, context)?;
        Ok(Self { family, downloads, metadata: ProvisioningMetadata::new(metadata), runtime })
    }
    /// Matching facts for the native adapter's private named containment job.
    pub(crate) fn family(&self) -> CacheFamilyId {
        self.family
    }
    /// Explicit paths and byte ceilings; presentation/IPC content cannot select these grants.
    pub(crate) fn arguments(&self) -> io::Result<[OsString; 10]> {
        Ok([
            "--setup-cache-downloads".into(),
            self.downloads.path().into_os_string(),
            "--setup-cache-download-bytes".into(),
            DOWNLOAD_BYTES.to_string().into(),
            "--setup-cache-metadata".into(),
            self.metadata.path()?.into_os_string(),
            "--setup-cache-metadata-bytes".into(),
            METADATA_BYTES.to_string().into(),
            "--setup-cache-runtime".into(),
            self.runtime.path().into_os_string(),
        ])
    }
    /// Clone only the actual prepared metadata grant, never a worker-selected filesystem path.
    pub(crate) fn metadata(&self) -> ProvisioningMetadata {
        self.metadata.clone()
    }
    /// Finalize from the registry's unique actually settled family result. This is called only
    /// after every owned process and observation has retired; supplied exit codes grant nothing.
    /// Startup recovery independently requires native object absence before calling `finalize`.
    pub(crate) fn finish_settled(
        &mut self,
        pressure: ProvisioningCachePressure,
        context: &JobContext,
    ) -> io::Result<()> {
        check(context)?;
        finalize(&mut self.downloads, context)?;
        self.metadata.finish(pressure.metadata, context)?;
        finalize(&mut self.runtime, context)?;
        if pressure.downloads {
            self.downloads.retire(context)?;
        }
        self.runtime.retire(context)
    }
}
/// Recover disposable provisioning output only after its private native containment object vanishes.
/// Run on an admitted storage worker with 8 MiB alongside the retained cache scope. Every pending
/// grant is inspected once, so a live family never hides a later retired family. No process stops.
/// # Errors
/// Preserves damaged ownership, cancellation and unexpected native query or storage failures.
pub fn recover_provisioning_cache(cache: &RuntimeCache, context: &JobContext) -> io::Result<()> {
    for namespace in
        [Namespace::SetupDownloads, Namespace::UpstreamMetadata, Namespace::SetupRuntime]
    {
        let mut entries = cache.pending(namespace, context)?;
        while let Some(mut lease) = entries.next(context)? {
            let value =
                lease.producer().ok_or_else(|| invalid("Provisioning recovery facts absent"))?;
            let mut parts = value.split(':');
            if parts.next() != Some(PREFIX) {
                return Err(invalid("Provisioning recovery schema is invalid"));
            }
            let family = CacheFamilyId::parse(
                parts.next().ok_or_else(|| invalid("Provisioning family absent"))?,
            )?;
            if descriptor(&lease, namespace, family)? != value {
                return Err(invalid("Provisioning recovery binding is damaged"));
            }
            if family.retired(context)? {
                finalize(&mut lease, context)?;
                if namespace == Namespace::SetupRuntime {
                    lease.retire(context)?;
                }
            }
        }
    }
    cache.maintain(cache::utc_seconds()?, context)?;
    Ok(())
}
/// Retry is idempotent after a previous inventory commit; oversize output stays reclaimable.
fn finalize(lease: &mut CacheLease, context: &JobContext) -> io::Result<()> {
    if lease.producer().is_none() {
        return Ok(());
    }
    match lease.record_generated(cache::utc_seconds()?, context) {
        Ok(_) => Ok(()),
        Err(cause) if cause.kind() == io::ErrorKind::StorageFull => {
            tracing::warn!(operation="provisioning_cache_overflow", cause=%cause,
                "Retired cache output exceeded its grant and remains eligible for shared cleanup");
            Ok(())
        }
        Err(cause) => Err(cause),
    }
}
/// Bind the feature's matching facts to the actual writable data object without path dependence.
fn descriptor(
    lease: &CacheLease,
    namespace: Namespace,
    family: CacheFamilyId,
) -> io::Result<String> {
    let identity = Directory::open(&lease.path(), false)?.identity()?;
    let mut digest = Sha256::new();
    digest.update(PREFIX.as_bytes());
    digest.update(namespace.directory().as_bytes());
    for word in [identity.0, identity.1, identity.2] {
        digest.update(word.to_le_bytes());
    }
    digest.update(family.encoded().as_bytes());
    Ok(format!("{PREFIX}:{}:{:x}", family.encoded(), digest.finalize()))
}

#[cfg(test)]
mod tests;
