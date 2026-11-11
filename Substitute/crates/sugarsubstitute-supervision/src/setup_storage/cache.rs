// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own disposable runtime storage, disk admission, retained pins and restart-safe reclamation.
//!
//! Call on execution-owned storage workers. This owner never spawns work or authenticates cache
//! content. Features retain their keys, compatibility and verification rules. A lease must outlive
//! every file, native consumer and job using an entry. Settings, trust and installed generations
//! are never admitted here. Unknown content is preserved and reported by maintenance.
mod admission;
mod consumers;
pub use admission::storage_key;
pub(crate) use consumers::NativeCachePin;
mod entry;
mod generated_files;
mod generated_records;
pub(in crate::setup_storage) use generated_files::GeneratedFiles;
mod lookup;
mod maintenance;
mod native_output;
pub use native_output::PendingEntries;
mod payload;
mod policy;
mod reader;
mod record;
pub use reader::CacheReader;
pub(in crate::setup_storage) use record::public_error;
mod retire;
mod root;
mod stream;
mod tree;
use super::check;
use super::invalid;
use crate::durable_storage::{
    files::{self, Directory},
    protected::directories::Mode,
    protection::{self, Protection},
    retirement, staging,
};
pub use entry::CacheLease;
pub use maintenance::Maintenance;
pub use policy::{CachePolicy, Namespace};
use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
};
use sugarsubstitute_contracts::execution::JobContext;

/// Schema-qualified directory below an explicit application-owned installation or setup scope.
pub const DIRECTORY: &str = "runtime-cache-v1";
/// Maximum metadata and traversal workspace, in addition to native protection admission.
pub const RESERVATION_BYTES: u64 = 6 * 1024 * 1024;
const ROOT_BYTES: u64 = 64 * 1024;
/// Retained generation inventory and native ancestry, charged to its scope's original admission.
pub(super) const ENTRY_BYTES: u64 = 2 * 1024 * 1024;
const MAX_RECORD: usize = 1024 * 1024;
const MAX_ENTRIES: usize = 1024;
const MAX_FILES: usize = 8192;
const MAX_OBJECTS: usize = MAX_FILES * MAX_DEPTH;
const MAX_DEPTH: usize = 512;

/// Pinned cache scope with one policy shared by admission and maintenance.
#[derive(Clone)]
pub struct RuntimeCache {
    scope: std::sync::Arc<Scope>,
}
/// Share native scope custody and its original retained allocation admission across feature jobs.
struct Scope {
    anchor: Directory,
    root: Directory,
    _owner: File,
    policy: CachePolicy,
    memory: sugarsubstitute_contracts::execution::MemoryLease,
    _memory: sugarsubstitute_contracts::execution::MemoryClaim,
}
impl RuntimeCache {
    /// Open or atomically initialize this scope's disposable cache directory.
    /// The anchor must be an absolute local directory, created when missing. No ambient path is consulted.
    /// # Errors
    /// Rejects links, foreign ownership, unsupported schemas, invalid limits and native failures.
    pub fn open(anchor: &Path, policy: CachePolicy, context: &JobContext) -> io::Result<Self> {
        Self::open_retained(anchor, policy, context.memory(), context)
    }
    /// Open later storage using its owner's original admitted allocation and current cancellation.
    /// This retains allocation capacity, not an old execution context or scheduler authority.
    pub(in crate::setup_storage) fn open_retained(
        anchor: &Path,
        policy: CachePolicy,
        memory: &sugarsubstitute_contracts::execution::MemoryLease,
        context: &JobContext,
    ) -> io::Result<Self> {
        root::open(anchor, policy, memory, context).map_err(record::public_error)
    }
    /// Explicit application-owned anchor whose native identity remains pinned by this scope.
    pub fn anchor(&self) -> &Path {
        self.scope.anchor.path()
    }
    /// Stable path shared by native and delegated cache consumers.
    pub fn path(&self) -> &Path {
        self.scope.root.path()
    }
}

/// Obtain Unix UTC seconds for retention on the storage worker; tests supply explicit instants.
/// # Errors
/// Reports clocks before the Unix epoch instead of expiring entries with guessed timestamps.
pub fn utc_seconds() -> io::Result<u64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .map_err(io::Error::other)
}

#[cfg(test)]
mod tests;
