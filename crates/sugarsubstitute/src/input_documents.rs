// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Persist the input canvas's documents in one session directory and restore them at startup.
//!
//! The store keeps these files in its directory:
//!
//! - `input-documents.ssdoc`: the current archive. Every save writes a complete, validated
//!   archive to a fresh sibling and atomically replaces this file.
//! - `input-documents.previous.ssdoc`: the archive the last save replaced, kept as the
//!   recovery copy. Loading falls back to it when the current archive is rejected.
//! - `input-documents.rejected-<sha256>.ssdoc`: a byte-exact copy of a current or recovery file
//!   that failed validation, made before that file is ever replaced and named by its digest.
//! - `bases\`: originals of documents' ComfyUI temporary files; see [`bases`].
//!
//! A rejected archive is an observable miss: loading reports it and returns no documents, and
//! the host rebuilds from the workflow. Unrecognized files, including staging files left by an
//! interrupted write, are never removed. All file work runs as owned jobs on the execution
//! owner's blocking workers; [`Autosave`] coalesces bursts of document changes into one save.
mod autosave;
#[cfg(feature = "native-composition")]
pub(crate) mod bases;
mod files;
#[cfg(all(windows, feature = "native-composition"))]
pub(crate) mod legacy;
#[cfg(all(windows, feature = "windows-native-host"))]
pub(crate) use files::classify_migration_source;
#[cfg(test)]
mod tests;
#[cfg(all(windows, feature = "native-composition"))]
pub(crate) mod workspace;

pub use autosave::{Autosave, AutosaveError, AutosavePolicy, AutosaveSchedule, SaveOutcome};

use std::{error::Error, fmt, io, path::PathBuf};
use sugarsubstitute_canvas::{ArchiveError, InputDocumentArchive, MAX_ARCHIVE_BYTES};
use sugarsubstitute_contracts::execution::{
    AdmissionError, ExecutionPort, JobSpec, MemoryLease, OwnedJob, Scope,
};

/// Current archive file name.
pub const CURRENT_ARCHIVE: &str = "input-documents.ssdoc";
/// Recovery copy file name.
pub const PREVIOUS_ARCHIVE: &str = "input-documents.previous.ssdoc";
/// Fixed working memory of one store job beyond archive bytes: stream buffers and compressor
/// state.
const WORKING_BYTES: u64 = 4 * 1024 * 1024;

/// Why a store job failed. Files on disk are unchanged unless a [`SaveReceipt`] says so.
#[derive(Debug)]
pub enum StoreError {
    /// Encoding, decoding or validation of archive bytes failed.
    Archive(ArchiveError),
    /// The encoded archive did not decode to the content it was made from; nothing was written.
    Unverified,
    /// Filesystem access failed, including a file that changed while it was being copied.
    Storage(io::Error),
}
impl StoreError {
    /// Whether the job stopped for cancellation, deadline or memory admission.
    pub fn interrupted(&self) -> bool {
        match self {
            Self::Archive(cause) => cause.interrupted(),
            Self::Unverified => false,
            Self::Storage(cause) => {
                matches!(cause.kind(), io::ErrorKind::Interrupted | io::ErrorKind::TimedOut)
            }
        }
    }
}
impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Archive(cause) => write!(formatter, "Input-document archive failed: {cause}"),
            Self::Unverified => {
                formatter.write_str("Encoded input documents did not decode to the same content")
            }
            Self::Storage(cause) => write!(formatter, "Input-document storage failed: {cause}"),
        }
    }
}
impl Error for StoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Archive(cause) => Some(cause),
            Self::Unverified => None,
            Self::Storage(cause) => Some(cause),
        }
    }
}
impl From<ArchiveError> for StoreError {
    fn from(cause: ArchiveError) -> Self {
        Self::Archive(cause)
    }
}
impl From<io::Error> for StoreError {
    fn from(cause: io::Error) -> Self {
        Self::Storage(cause)
    }
}

/// Which archive file a restore came from or a rejection concerns.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveFile {
    /// [`CURRENT_ARCHIVE`].
    Current,
    /// [`PREVIOUS_ARCHIVE`].
    Previous,
}

/// An archive file that failed validation; it stays in place until a save preserves it.
#[derive(Debug)]
pub struct Rejection {
    /// The rejected file.
    pub file: ArchiveFile,
    /// Why it was rejected.
    pub cause: ArchiveError,
}

/// Result of loading at startup.
#[derive(Debug)]
pub struct Restored {
    /// The restored archive and the file it came from; `None` when the host rebuilds documents
    /// from the workflow.
    pub archive: Option<(ArchiveFile, InputDocumentArchive)>,
    /// Files that were rejected on the way, in the order they were tried.
    pub rejected: Vec<Rejection>,
}

/// What one save changed on disk.
#[derive(Debug, Eq, PartialEq)]
pub struct SaveReceipt {
    /// Bytes of the archive now current.
    pub bytes: u64,
    /// Whether the replaced archive became the recovery copy.
    pub kept_previous: bool,
    /// Evidence copies of rejected files made before they were replaced.
    pub evidence: Vec<PathBuf>,
}

/// Owner of the input-document files in one session directory.
#[derive(Clone, Debug)]
pub struct InputDocumentStore {
    directory: PathBuf,
    expected: Option<[u8; 32]>,
}
/// Native input startup authority, resolved before workflow sync can replace retained documents.
#[derive(Clone, Debug)]
#[cfg(all(windows, feature = "native-composition"))]
pub(crate) enum InputRestorePlan {
    /// Load the selected namespace's existing current/previous archive normally.
    Current,
    /// Load an independently codec/fingerprint/metadata-validated immutable archive.
    Verified(InputDocumentStore),
    /// Hold workflow sync until the host associates the passive legacy archive or declines it.
    AwaitingAssociation,
}
impl InputDocumentStore {
    /// Use `directory`, created by the first save when absent.
    /// # Errors
    /// Rejects a relative path.
    pub fn new(directory: PathBuf) -> Result<Self, StoreError> {
        if !directory.is_absolute() {
            return Err(StoreError::Storage(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Input-document directory must be absolute",
            )));
        }
        Ok(Self { directory, expected: None })
    }

    #[cfg(all(windows, feature = "native-composition"))]
    pub(crate) fn for_reference(
        directory: PathBuf,
        expected: [u8; 32],
    ) -> Result<Self, StoreError> {
        let mut store = Self::new(directory)?;
        store.expected = Some(expected);
        Ok(store)
    }
    pub(crate) fn load_worker(
        &self,
        memory: &MemoryLease,
        context: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<Restored, StoreError> {
        let restored = files::load(&self.directory, memory, context)?;
        if self.expected.is_some_and(|expected| {
            restored.archive.as_ref().is_none_or(|(_, archive)| archive.fingerprint() != expected)
        }) {
            return Err(StoreError::Unverified);
        }
        Ok(restored)
    }
    /// The session directory holding the archive files.
    pub fn directory(&self) -> &std::path::Path {
        &self.directory
    }

    /// Job memory a load needs, in bytes; restored coverage is charged to the caller's lease.
    pub const fn load_reservation() -> u64 {
        MAX_ARCHIVE_BYTES + WORKING_BYTES
    }

    /// Job memory saving `archive` needs, in bytes: its encoding plus a decoded copy for
    /// validation.
    pub fn save_reservation(archive: &InputDocumentArchive) -> u64 {
        let tiles = archive.stored_tile_bytes();
        archive
            .encoded_bound()
            .min(MAX_ARCHIVE_BYTES)
            .saturating_add(tiles.saturating_add(tiles / 64))
            .saturating_add(WORKING_BYTES)
    }

    /// Submit a load of the current archive, falling back to the recovery copy when the current
    /// archive is rejected. Restored coverage is charged to `memory`.
    ///
    /// The job runs on a blocking worker; observe it through the returned owned job.
    /// # Errors
    /// Preserves admission rejection; no file has been touched.
    pub fn submit_load<P: ExecutionPort>(
        &self,
        execution: &P,
        scope: &Scope,
        spec: JobSpec,
        memory: MemoryLease,
    ) -> Result<OwnedJob<Restored, StoreError>, AdmissionError> {
        let store = self.clone();
        execution.submit_blocking(scope, spec, move |context| store.load_worker(&memory, &context))
    }

    /// Submit an atomic save of `archive`. The encoding is decoded and compared with `archive`
    /// before any file changes; the replaced archive becomes the recovery copy, and rejected
    /// files are preserved as evidence before they are replaced.
    ///
    /// The job runs on a blocking worker; cancelling it before the final replacement leaves the
    /// current archive untouched.
    /// # Errors
    /// Preserves admission rejection; no file has been touched.
    pub fn submit_save<P: ExecutionPort>(
        &self,
        execution: &P,
        scope: &Scope,
        spec: JobSpec,
        archive: InputDocumentArchive,
    ) -> Result<OwnedJob<SaveReceipt, StoreError>, AdmissionError> {
        let directory = self.directory.clone();
        execution.submit_blocking(scope, spec, move |context| {
            files::save(&directory, &archive, &context)
        })
    }
}
