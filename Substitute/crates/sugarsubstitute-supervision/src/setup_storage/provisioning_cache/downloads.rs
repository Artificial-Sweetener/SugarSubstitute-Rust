// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain native download authority while upstream owns semantic keys, resume metadata and hashes.
use super::*;
use cache::GeneratedFiles;
use std::sync::{Arc, Mutex, MutexGuard};
use sugarsubstitute_contracts::execution::MemoryClaim;

/// An actual prepared download grant, independent of worker-selected JSON paths and job results.
/// Every clone pins the same exclusive cache lease, original memory, admitted names and payloads.
/// Call I/O only from admitted blocking jobs. Finalization requires actual process-family retirement
/// and retirement of every access clone; a cancelled job or dropped reply proves neither.
#[derive(Clone)]
pub struct ProvisioningDownloads(Arc<Mutex<GeneratedFiles>>);

/// A bounded native sidecar retains the allocation which funded its read after job completion.
pub struct DownloadRecord {
    bytes: Vec<u8>,
    _memory: MemoryClaim,
}
impl DownloadRecord {
    /// Borrow exact bytes; sidecar schemas and parsing stay with the upstream feature.
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }
}
impl ProvisioningDownloads {
    /// Derive native authority only from the original exclusively prepared generated lease.
    pub(super) fn new(lease: CacheLease, context: &JobContext) -> io::Result<Self> {
        Ok(Self(Arc::new(Mutex::new(
            GeneratedFiles::new(lease, context).map_err(cache::public_error)?,
        ))))
    }
    /// Reject overlapping native operations instead of waiting on the UI or another worker.
    fn state(&self) -> io::Result<MutexGuard<'_, GeneratedFiles>> {
        self.0.try_lock().map_err(|cause| match cause {
            std::sync::TryLockError::WouldBlock => {
                io::Error::new(io::ErrorKind::WouldBlock, "Download operation active")
            }
            std::sync::TryLockError::Poisoned(_) => io::Error::other("Download custody poisoned"),
        })
    }
    /// Keep category interpretation with storage while preserving every original native cause.
    fn with<T>(
        &self,
        operation: impl FnOnce(&mut GeneratedFiles) -> io::Result<T>,
    ) -> io::Result<T> {
        let mut state = self.state()?;
        operation(&mut state).map_err(cache::public_error)
    }
    /// Observe the native root for process arguments; this path does not establish authority.
    pub(super) fn path(&self) -> io::Result<std::path::PathBuf> {
        Ok(self.state()?.path())
    }

    /// Resolve the existing SHA-256 key directory and immutable schema-1 ownership marker.
    /// # Errors
    /// Rejects invalid identities, nonempty unmarked directories, changed markers, unsafe paths,
    /// exhausted original grants, cancellation and native failures without replacing unknown bytes.
    pub fn entry(&self, key: &str, context: &JobContext) -> io::Result<std::path::PathBuf> {
        if key.is_empty() || key.encode_utf16().count() > 1024 {
            return Err(invalid("Invalid upstream cache identity"));
        }
        /// Preserve the existing JavaScript marker's field order and original upstream key.
        #[derive(serde::Serialize)]
        struct Marker<'a> {
            schema_version: u8,
            #[serde(rename = "upstreamKey")]
            upstream_key: &'a str,
        }
        let _scratch = context.memory().claim(16 * 1024).map_err(io::Error::other)?;
        let marker = serde_json::to_vec(&Marker { schema_version: 1, upstream_key: key })
            .map_err(io::Error::other)?;
        let directory = format!("{:x}", Sha256::digest(key.as_bytes()));
        self.with(|state| state.initialize(&directory, ".substitute-cache.json", &marker, context))
    }
    /// Observe a closed admitted payload's physical length for upstream resume decisions.
    /// # Errors
    /// Rejects active writers, changed native identity, byte pressure and native/control failures.
    pub fn length(&self, name: &str, context: &JobContext) -> io::Result<Option<u64>> {
        self.with(|state| state.length(name, context))
    }
    /// Read one bounded native sidecar without interpreting its feature-owned schema.
    /// # Errors
    /// Rejects absent/active/unsafe/oversized records and preserves native/control failures.
    pub fn read(&self, name: &str, context: &JobContext) -> io::Result<DownloadRecord> {
        let record = self.with(|state| state.read(name, context))?;
        Ok(DownloadRecord { bytes: record.bytes, _memory: record.memory })
    }
    /// Replace a bounded sidecar after admitting original bytes plus complete staged replacement.
    /// # Errors
    /// Preserves original bytes on incomplete publication and reports pressure/native/control errors.
    pub fn write(&self, name: &str, bytes: &[u8], context: &JobContext) -> io::Result<()> {
        self.with(|state| state.write(name, bytes, context))
    }
    /// Retire one inactive native file by verified identity, retaining unknown or uncertain objects.
    /// # Errors
    /// Rejects active writers, links and native/control failures; absent admitted files are harmless.
    pub fn remove(&self, name: &str, context: &JobContext) -> io::Result<()> {
        self.with(|state| state.remove(name, context))
    }
    /// Admit a native payload and optional expected extent without resetting any existing prefix.
    /// Returns its physical byte length. Resume/restart decisions remain with the upstream caller.
    /// # Errors
    /// Rejects overlap, original grant exhaustion, unsafe paths and native/control failures.
    pub fn open(&self, name: &str, expected: Option<u64>, context: &JobContext) -> io::Result<u64> {
        self.with(|state| state.open(name, expected, context))
    }
    /// Explicitly invalidate the retained payload's bytes after the feature chooses to restart.
    /// # Errors
    /// Preserves native/control failures; resetting does not free its admitted expected byte extent.
    pub fn reset(&self, name: &str, context: &JobContext) -> io::Result<()> {
        self.with(|state| state.reset(name, context))
    }
    /// Append one bounded chunk at the physical end after aggregate admission, without per-chunk sync.
    /// # Errors
    /// Rejects holes/replays, oversize chunks, byte pressure and native/control failures.
    pub fn append(
        &self,
        name: &str,
        offset: u64,
        bytes: &[u8],
        context: &JobContext,
    ) -> io::Result<()> {
        self.with(|state| state.append(name, offset, bytes, context))
    }
    /// Flush the actual prefix before closing the native writer, including on cancellation/abort.
    /// # Errors
    /// Failure retains the actual writer and its original admission for observable recovery.
    pub fn close(&self, name: &str) -> io::Result<()> {
        self.with(|state| state.close(name))
    }

    /// Validate physical contents while retaining active writers and their admitted byte extents.
    /// # Errors
    /// Unadmitted content, links and native/control failures remain observable.
    pub fn validate(&self, context: &JobContext) -> io::Result<()> {
        self.with(|state| state.validate(context))
    }

    /// Finalization is reachable only after the registry supplies actual family-retirement evidence.
    pub(super) fn finish(&mut self, pressure: bool, context: &JobContext) -> io::Result<()> {
        if Arc::strong_count(&self.0) != 1 {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Download access has not retired",
            ));
        }
        let mut state = self.state()?;
        match state.record(context).map_err(cache::public_error) {
            Ok(()) => {}
            Err(cause) if cause.kind() == io::ErrorKind::StorageFull => {
                tracing::warn!(operation="provisioning_download_overflow", cause=%cause, "Retired download output exceeded its original grant");
            }
            Err(cause) => return Err(cause),
        }
        if pressure {
            state.retire(context).map_err(cache::public_error)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
