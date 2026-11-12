// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the original provisioning metadata grant across admitted transport operations.
use super::*;
use crate::durable_storage::streamed_record::{StreamRead, StreamWrite};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
/// Closed metadata roles grant neither arbitrary filenames nor another cache namespace.
#[derive(Clone, Copy, serde::Deserialize)]
pub enum MetadataRecord {
    /// Upstream ETag responses and their original feature-owned JSON schema.
    #[serde(rename = "fetch-cache.json")]
    Fetch,
    /// Upstream Torch bundle catalogue snapshots.
    #[serde(rename = "torch-stack-cache.json")]
    TorchStack,
    /// Upstream validated remote Torch index manifest snapshots.
    #[serde(rename = "torch-index-manifest-cache.json")]
    TorchIndex,
}
impl MetadataRecord {
    /// Resolve only the fixed filename assigned to this native role.
    fn name(self) -> &'static str {
        match self {
            Self::Fetch => "fetch-cache.json",
            Self::TorchStack => "torch-stack-cache.json",
            Self::TorchIndex => "torch-index-manifest-cache.json",
        }
    }
}

/// Shared original lease and a weak reference to the actual transfer lifetime, never its job status.
struct State {
    lease: CacheLease,
    active: Weak<()>,
}
/// Native metadata authority derived only from an actual prepared provisioning launch.
/// Clones retain the same exclusive cache lease and original memory admission. Call filesystem
/// methods only on admitted blocking jobs. The process family cannot finalize this grant while
/// any access capability or actual transfer remains alive; cancellation does not retire either.
#[derive(Clone)]
pub struct ProvisioningMetadata(Arc<Mutex<State>>);
/// One exact native source, retained across bounded read requests and independent of job results.
pub struct MetadataRead {
    inner: StreamRead,
    _owner: ProvisioningMetadata,
    _transfer: Arc<()>,
}
/// One fully admitted native replacement. Dropping closes its handle and preserves staged bytes.
pub struct MetadataWrite {
    inner: StreamWrite,
    _owner: ProvisioningMetadata,
    _transfer: Arc<()>,
}
/// Bounded source bytes retain their own original execution allocation after reader retirement.
pub struct MetadataChunk {
    bytes: Vec<u8>,
    _memory: sugarsubstitute_contracts::execution::MemoryClaim,
}
impl MetadataChunk {
    /// Borrow exact source bytes without detaching their memory custody.
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }
}
impl ProvisioningMetadata {
    pub(super) fn new(lease: CacheLease) -> Self {
        Self(Arc::new(Mutex::new(State { lease, active: Weak::new() })))
    }
    /// Contention rejects admission instead of blocking another native job or the UI.
    fn state(&self) -> io::Result<MutexGuard<'_, State>> {
        self.0.try_lock().map_err(|cause| match cause {
            std::sync::TryLockError::WouldBlock => {
                io::Error::new(io::ErrorKind::WouldBlock, "Metadata operation active")
            }
            std::sync::TryLockError::Poisoned(_) => io::Error::other("Metadata custody poisoned"),
        })
    }
    /// Open one of the worker's three reconstructible metadata schema records.
    /// # Errors
    /// Rejects overlapping transfers, links, oversize files and native/control errors.
    pub fn read(&self, record: MetadataRecord, context: &JobContext) -> io::Result<MetadataRead> {
        let mut state = self.state()?;
        let transfer = state.transfer()?;
        let inner = state.lease.generated_read(record.name(), context)?;
        state.active = Arc::downgrade(&transfer);
        Ok(MetadataRead { inner, _owner: self.clone(), _transfer: transfer })
    }
    /// Admit original bytes plus complete replacement scratch before any file effect.
    /// # Errors
    /// Rejects overlap, byte pressure, links, cancellation and native storage errors.
    pub fn write(
        &self,
        record: MetadataRecord,
        length: u64,
        context: &JobContext,
    ) -> io::Result<MetadataWrite> {
        let mut state = self.state()?;
        let transfer = state.transfer()?;
        let inner = state.lease.generated_write(record.name(), length, context)?;
        state.active = Arc::downgrade(&transfer);
        Ok(MetadataWrite { inner, _owner: self.clone(), _transfer: transfer })
    }
    pub(super) fn path(&self) -> io::Result<std::path::PathBuf> {
        Ok(self.state()?.lease.path())
    }
    pub(super) fn finish(&mut self, pressure: bool, context: &JobContext) -> io::Result<()> {
        if Arc::strong_count(&self.0) != 1 {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Metadata access has not retired",
            ));
        }
        let mut state = self.state()?;
        finalize(&mut state.lease, context)?;
        if pressure {
            state.lease.retire(context)?;
        }
        Ok(())
    }
}
impl State {
    /// An unfinished native transfer retains admission even between individual requests.
    fn transfer(&self) -> io::Result<Arc<()>> {
        if self.active.upgrade().is_some() {
            return Err(io::Error::new(io::ErrorKind::WouldBlock, "Metadata transfer active"));
        }
        Ok(Arc::new(()))
    }
}
impl MetadataRead {
    /// Read at most 8 KiB under the current job's controls; retain the source until actual completion.
    /// # Errors
    /// Preserves native shortening, cancellation, deadlines and read failures.
    pub fn next(&mut self, context: &JobContext) -> io::Result<MetadataChunk> {
        let memory = context.memory().claim(8192).map_err(io::Error::other)?;
        Ok(MetadataChunk { bytes: self.inner.next(context)?, _memory: memory })
    }
    /// Completion follows the original native length, including an empty record.
    pub fn done(&self) -> bool {
        self.inner.done()
    }
}
impl MetadataWrite {
    /// Write at most 8 KiB into the original admitted stage without publishing it.
    /// # Errors
    /// Rejects overflow, cancellation, deadlines and native write failures.
    pub fn append(&mut self, bytes: &[u8], context: &JobContext) -> io::Result<()> {
        self.inner.append(bytes, context)
    }
    /// Flush and publish only the complete admitted byte extent; then retire this transfer.
    /// # Errors
    /// Preserves original target bytes on incomplete input and reports publication/native errors.
    pub fn commit(self, context: &JobContext) -> io::Result<()> {
        self.inner.commit(context)
    }
}

#[cfg(test)]
mod tests;
