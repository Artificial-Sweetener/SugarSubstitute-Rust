// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the originals of input documents whose files ComfyUI may not keep.
//!
//! A base from ComfyUI's temporary storage is deleted when ComfyUI starts, and an output can be
//! moved or deleted, so the masks the user painted over it could never restore. Each such
//! file's encoded original is retained in the session directory's `bases` folder when it is
//! first acquired:
//!
//! - `<sha256>.image`: the encoded original, named by the digest of its bytes. It is written
//!   once and shared by every reference with the same content.
//! - `<sha256 of reference>.reference`: the content digest the reference last acquired.
//!
//! The retained original is what the user authored on, so it is preferred over a live file of
//! the same name, which ComfyUI may reuse for new content after it restarts. A retained file
//! whose bytes no longer match its digest is never used and stays in place as evidence.
//!
//! [`RetainedBases::unreferenced`] decides which retained files no document needs any more:
//! every reference record no current surface names, and every original no remaining reference
//! record names. Files this owner does not recognise are never selected.
use super::StoreError;
use sha2::{Digest, Sha256};
use std::{
    io::{self, Read},
    path::{Path, PathBuf},
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim, MemoryLease};
use sugarsubstitute_supervision::{
    ArtifactError, ArtifactPublication, RecordDirectory, with_artifact_input, write_artifact,
};

/// The folder below the session directory that holds retained bases.
pub(crate) const BASES: &str = "bases";
/// Largest encoded reference record, in bytes.
const MAX_REFERENCE_BYTES: u64 = 128;

/// The retained bases of one session directory.
#[derive(Clone, Debug)]
pub(crate) struct RetainedBases {
    directory: PathBuf,
}

impl RetainedBases {
    /// Retain bases below `session`, the input-document store's directory.
    pub(crate) fn new(session: &Path) -> Self {
        Self { directory: session.join(BASES) }
    }

    /// Retain `bytes` as the encoded original of `reference`. Runs on an execution worker.
    /// # Errors
    /// Preserves storage, cancellation and deadline failures; an existing original whose
    /// bytes differ from its name is reported changed and left untouched.
    pub(crate) fn retain(
        &self,
        reference: &str,
        bytes: &[u8],
        memory: &MemoryLease,
        context: &JobContext,
    ) -> Result<(), StoreError> {
        let digest = hex_digest(&Sha256::digest(bytes));
        let image = self.directory.join(format!("{digest}.image"));
        // Equal content is written once; a present original is reused only when it verifies.
        let length = bytes.len() as u64;
        if self.read_verified(&image, &digest, length, memory, context)?.is_none() {
            write_artifact(
                &image,
                true,
                ArtifactPublication::CreateOnly,
                bytes.len().max(1),
                context,
                |output| output.write_all(bytes),
            )
            .map_err(storage)?;
        }
        write_artifact(
            &self.reference(reference),
            true,
            ArtifactPublication::ReplaceSelected,
            digest.len(),
            context,
            |output| output.write_all(digest.as_bytes()),
        )
        .map_err(storage)
    }

    /// The verified encoded original last retained for `reference`, charged to `memory`, or
    /// `None` when none was retained or its bytes no longer match.
    /// # Errors
    /// Preserves storage, admission, cancellation and deadline failures.
    pub(crate) fn find(
        &self,
        reference: &str,
        maximum: u64,
        memory: &MemoryLease,
        context: &JobContext,
    ) -> Result<Option<(Vec<u8>, MemoryClaim)>, StoreError> {
        let Some(digest) = read(&self.reference(reference), MAX_REFERENCE_BYTES, context)? else {
            return Ok(None);
        };
        let digest = String::from_utf8(digest).map_err(|_| invalid())?;
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(invalid());
        }
        let image = self.directory.join(format!("{digest}.image"));
        let found = self.read_verified(&image, &digest, maximum, memory, context)?;
        if found.is_none() {
            tracing::warn!(
                operation = "input_documents_retained_base",
                "A retained base no longer matches its digest; it is kept as evidence"
            );
        }
        Ok(found)
    }

    /// The retained files no document needs once only `keep` references are in use, by leaf
    /// name in sorted order. An original stays while any kept reference record names it.
    /// # Errors
    /// Preserves listing, storage, cancellation and deadline failures and refuses a reference
    /// record this owner cannot read.
    pub(crate) fn unreferenced<'a>(
        &self,
        keep: impl IntoIterator<Item = &'a str>,
        context: &JobContext,
    ) -> Result<Vec<String>, StoreError> {
        let kept: std::collections::BTreeSet<String> =
            keep.into_iter().map(reference_leaf).collect();
        let mut listing = match RecordDirectory::open(&self.directory, false, false, context) {
            Ok(listing) => listing,
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(cause) => return Err(StoreError::Storage(cause)),
        };
        let mut leaves = Vec::new();
        let mut restart = true;
        loop {
            let page = listing.page(restart, context)?;
            restart = false;
            if page.is_empty() {
                break;
            }
            leaves
                .extend(page.into_iter().filter(|entry| !entry.directory).map(|entry| entry.name));
        }
        let mut live = std::collections::BTreeSet::new();
        let mut unreferenced = Vec::new();
        for leaf in &leaves {
            if leaf.strip_suffix(".reference").is_some_and(digest_name) {
                if kept.contains(leaf) {
                    let digest = read(&self.directory.join(leaf), MAX_REFERENCE_BYTES, context)?
                        .ok_or_else(invalid)?;
                    live.insert(String::from_utf8(digest).map_err(|_| invalid())?);
                } else {
                    unreferenced.push(leaf.clone());
                }
            }
        }
        unreferenced.extend(
            leaves
                .iter()
                .filter(|leaf| {
                    leaf.strip_suffix(".image")
                        .is_some_and(|digest| digest_name(digest) && !live.contains(digest))
                })
                .cloned(),
        );
        unreferenced.sort();
        Ok(unreferenced)
    }

    fn reference(&self, reference: &str) -> PathBuf {
        self.directory.join(reference_leaf(reference))
    }

    /// Read `path` and return its bytes only when they hash to `digest`.
    fn read_verified(
        &self,
        path: &Path,
        digest: &str,
        maximum: u64,
        memory: &MemoryLease,
        context: &JobContext,
    ) -> Result<Option<(Vec<u8>, MemoryClaim)>, StoreError> {
        let read = with_artifact_input(path, maximum, context, |input| {
            let length = input.len();
            let claim = memory.claim(length.saturating_add(128)).map_err(io::Error::other)?;
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(usize::try_from(length).map_err(io::Error::other)?)
                .map_err(io::Error::other)?;
            input.read_to_end(&mut bytes)?;
            Ok::<_, io::Error>((bytes, claim))
        });
        let (bytes, claim) = match read {
            Ok(read) => read,
            Err(ArtifactError::Storage(cause)) if cause.kind() == io::ErrorKind::NotFound => {
                return Ok(None);
            }
            Err(cause) => return Err(storage(cause)),
        };
        Ok((hex_digest(&Sha256::digest(&bytes)) == digest).then_some((bytes, claim)))
    }
}

fn read(path: &Path, maximum: u64, context: &JobContext) -> Result<Option<Vec<u8>>, StoreError> {
    match with_artifact_input(path, maximum, context, |input| {
        let mut bytes = Vec::new();
        input.read_to_end(&mut bytes)?;
        Ok::<_, io::Error>(bytes)
    }) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(ArtifactError::Storage(cause)) if cause.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(cause) => Err(storage(cause)),
    }
}

/// The leaf of the record naming `reference`'s retained original.
fn reference_leaf(reference: &str) -> String {
    format!("{}.reference", hex_digest(&Sha256::digest(reference.as_bytes())))
}

/// Whether `name` is a lowercase SHA-256 spelling, the only names this owner writes.
fn digest_name(name: &str) -> bool {
    name.len() == 64 && name.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn invalid() -> StoreError {
    StoreError::Storage(io::Error::new(io::ErrorKind::InvalidData, "Unsupported retained base"))
}

fn storage<E: std::error::Error + Send + Sync + 'static>(cause: ArtifactError<E>) -> StoreError {
    match cause {
        ArtifactError::Storage(cause) => StoreError::Storage(cause),
        other => StoreError::Storage(io::Error::other(other)),
    }
}

fn hex_digest(digest: &[u8]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}
