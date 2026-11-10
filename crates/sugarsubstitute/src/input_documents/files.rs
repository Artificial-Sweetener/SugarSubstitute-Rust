// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Worker-side archive files: validated atomic replacement, recovery copy and evidence.
//!
//! Every function runs on an admitted blocking worker and performs filesystem work only
//! through the supervision artifact port, which stages writes in fresh siblings and
//! replaces targets atomically after a flushed, checked close.
use super::{
    ArchiveFile, CURRENT_ARCHIVE, PREVIOUS_ARCHIVE, Rejection, Restored, SaveReceipt, StoreError,
};
use sha2::{Digest, Sha256};
use std::{
    io::{self, Read},
    path::{Path, PathBuf},
};
use sugarsubstitute_canvas::{
    ArchiveError, ArchiveIntegrity, InputDocumentArchive, MAX_ARCHIVE_BYTES,
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim, MemoryLease};
use sugarsubstitute_supervision::{
    ArtifactError, ArtifactInput, ArtifactPublication, with_artifact_input, write_artifact,
};

/// Streaming copy and inspection buffer.
const CHUNK_BYTES: usize = 64 * 1024;
/// Largest file inspected or preserved as evidence; anything larger was not written here and
/// is left untouched.
const MAX_INSPECTED_BYTES: u64 = 4 * 1024 * 1024 * 1024;

/// What one archive file holds, observed in a single streaming pass.
enum Observed {
    Absent,
    Archive { length: u64, digest: [u8; 32] },
    Rejected { length: u64, digest: [u8; 32] },
}

/// Reuse the current streaming integrity policy before a historical container is copied.
/// Rejected/unknown encodings remain original evidence; this grants no archive association.
#[cfg(all(windows, feature = "windows-native-host"))]
pub(crate) fn classify_migration_source(
    path: &Path,
    context: &JobContext,
) -> Result<sugarsubstitute_supervision::setup_storage::HistoricalArchiveDisposition, StoreError> {
    use sugarsubstitute_supervision::setup_storage::HistoricalArchiveDisposition;
    match observe(path, context)? {
        Observed::Archive { .. } => Ok(HistoricalArchiveDisposition::Validated),
        Observed::Rejected { .. } => Ok(HistoricalArchiveDisposition::PreservedOriginal),
        Observed::Absent => Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Historical archive disappeared before inspection",
        )
        .into()),
    }
}

/// Restore the current archive, or the recovery copy when the current archive is rejected.
pub(super) fn load(
    directory: &Path,
    memory: &MemoryLease,
    context: &JobContext,
) -> Result<Restored, StoreError> {
    let mut rejected = Vec::new();
    for (file, name) in
        [(ArchiveFile::Current, CURRENT_ARCHIVE), (ArchiveFile::Previous, PREVIOUS_ARCHIVE)]
    {
        let Some(bytes) = read(&directory.join(name), context)? else {
            continue;
        };
        let decoded =
            bytes.and_then(|(bytes, _claim)| InputDocumentArchive::decode(&bytes, memory, context));
        match decoded {
            Ok(archive) => return Ok(Restored { archive: Some((file, archive)), rejected }),
            Err(cause) if cause.interrupted() => return Err(cause.into()),
            Err(cause) => {
                tracing::warn!(operation = "input_documents_load", file = ?file, cause = %cause,
                    "Input-document archive rejected");
                rejected.push(Rejection { file, cause });
            }
        }
    }
    Ok(Restored { archive: None, rejected })
}

/// Validate the encoding, keep the replaced archive as the recovery copy, preserve rejected
/// files as evidence, then atomically replace the current archive.
pub(super) fn save(
    directory: &Path,
    archive: &InputDocumentArchive,
    context: &JobContext,
) -> Result<SaveReceipt, StoreError> {
    // The claim funds the encoded bytes while the verification decode charges its own tiles.
    let (bytes, _claim) = archive.encode(context)?;
    if InputDocumentArchive::decode(&bytes, context.memory(), context)? != *archive {
        return Err(StoreError::Unverified);
    }
    let current = directory.join(CURRENT_ARCHIVE);
    let previous = directory.join(PREVIOUS_ARCHIVE);
    let mut receipt =
        SaveReceipt { bytes: bytes.len() as u64, kept_previous: false, evidence: vec![] };
    match observe(&current, context)? {
        Observed::Absent => {}
        Observed::Archive { length, digest } => {
            if let Observed::Rejected { length, digest } = observe(&previous, context)? {
                receipt.evidence.push(preserve(directory, &previous, length, digest, context)?);
            }
            copy(
                &current,
                &previous,
                ArtifactPublication::ReplaceSelected,
                length,
                digest,
                context,
            )?;
            receipt.kept_previous = true;
        }
        Observed::Rejected { length, digest } => {
            receipt.evidence.push(preserve(directory, &current, length, digest, context)?);
        }
    }
    write_artifact(
        &current,
        true,
        ArtifactPublication::ReplaceSelected,
        bytes.len().max(1),
        context,
        |output| output.write_all(&bytes),
    )
    .map_err(artifact)?;
    tracing::info!(
        operation = "input_documents_save",
        bytes = receipt.bytes,
        kept_previous = receipt.kept_previous,
        evidence = receipt.evidence.len(),
        "Input documents saved"
    );
    Ok(receipt)
}

/// Archive bytes with the job memory claim that funds them.
type Charged = (Vec<u8>, MemoryClaim);

/// Retain one content-addressed native snapshot without replacing an existing original.
#[cfg(all(windows, feature = "native-composition"))]
pub(super) fn retain(
    directory: &Path,
    archive: &InputDocumentArchive,
    context: &JobContext,
) -> Result<SaveReceipt, StoreError> {
    let current = directory.join(CURRENT_ARCHIVE);
    if let Some(bytes) = read(&current, context)? {
        let (bytes, _claim) = bytes?;
        let existing = InputDocumentArchive::decode(&bytes, context.memory(), context)?;
        if existing != *archive {
            return Err(StoreError::Unverified);
        }
        return Ok(SaveReceipt {
            bytes: bytes.len() as u64,
            kept_previous: false,
            evidence: vec![],
        });
    }
    let (bytes, _claim) = archive.encode(context)?;
    if InputDocumentArchive::decode(&bytes, context.memory(), context)? != *archive {
        return Err(StoreError::Unverified);
    }
    write_artifact(
        &current,
        true,
        ArtifactPublication::CreateOnly,
        bytes.len().max(1),
        context,
        |output| output.write_all(&bytes),
    )
    .map_err(artifact)?;
    Ok(SaveReceipt { bytes: bytes.len() as u64, kept_previous: false, evidence: vec![] })
}

/// Read one archive file whole; `None` when absent, `Some(Err)` when larger than any archive.
fn read(
    path: &Path,
    context: &JobContext,
) -> Result<Option<Result<Charged, ArchiveError>>, StoreError> {
    let result = with_artifact_input(path, MAX_INSPECTED_BYTES, context, |input| {
        if input.len() > MAX_ARCHIVE_BYTES {
            return Ok(Err(ArchiveError::Oversized));
        }
        let length = input.len() as usize;
        let claim = context.memory().claim(input.len()).map_err(io::Error::other)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length).map_err(io::Error::other)?;
        input.read_to_end(&mut bytes)?;
        Ok::<_, io::Error>(Ok((bytes, claim)))
    });
    match result {
        Ok(bytes) => Ok(Some(bytes)),
        Err(ArtifactError::Storage(cause)) if cause.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(cause) => Err(artifact(cause)),
    }
}

/// Classify a file and digest its bytes without buffering it.
fn observe(path: &Path, context: &JobContext) -> Result<Observed, StoreError> {
    let result = with_artifact_input(path, MAX_INSPECTED_BYTES, context, |input| {
        let length = input.len();
        let mut integrity = ArchiveIntegrity::new();
        let mut verdict =
            if length > MAX_ARCHIVE_BYTES { Err(ArchiveError::Oversized) } else { Ok(()) };
        let digest = stream(input, context, |chunk| {
            if verdict.is_ok() {
                verdict = integrity.update(chunk);
            }
            Ok(())
        })?;
        let valid = verdict.and_then(|()| integrity.finish()).is_ok();
        Ok::<_, io::Error>(if valid {
            Observed::Archive { length, digest }
        } else {
            Observed::Rejected { length, digest }
        })
    });
    match result {
        Ok(observed) => Ok(observed),
        Err(ArtifactError::Storage(cause)) if cause.kind() == io::ErrorKind::NotFound => {
            Ok(Observed::Absent)
        }
        Err(cause) => Err(artifact(cause)),
    }
}

/// Copy a rejected file beside itself under its digest, unless an earlier save already did.
fn preserve(
    directory: &Path,
    source: &Path,
    length: u64,
    digest: [u8; 32],
    context: &JobContext,
) -> Result<PathBuf, StoreError> {
    let hex = digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    let evidence = directory.join(format!("input-documents.rejected-{hex}.ssdoc"));
    if matches!(observe(&evidence, context)?, Observed::Absent) {
        copy(source, &evidence, ArtifactPublication::CreateOnly, length, digest, context)?;
        tracing::warn!(
            operation = "input_documents_evidence",
            "Rejected input-document archive preserved"
        );
    }
    Ok(evidence)
}

/// Copy `source` to `target` only if it still holds the observed length and digest.
fn copy(
    source: &Path,
    target: &Path,
    publication: ArtifactPublication,
    length: u64,
    digest: [u8; 32],
    context: &JobContext,
) -> Result<(), StoreError> {
    let changed = || io::Error::other("Archive file changed while it was being copied");
    with_artifact_input(source, MAX_INSPECTED_BYTES, context, |input| {
        if input.len() != length {
            return Err(changed());
        }
        let maximum = usize::try_from(length).map_err(io::Error::other)?.max(1);
        write_artifact(target, false, publication, maximum, context, |output| {
            if stream(input, context, |chunk| output.write_all(chunk))? != digest {
                return Err(changed());
            }
            Ok(())
        })
        .map_err(io::Error::other)
    })
    .map_err(artifact)
}

/// Feed every remaining byte of `input` to `consume` in bounded chunks; returns their SHA-256.
fn stream(
    input: &mut ArtifactInput<'_>,
    context: &JobContext,
    mut consume: impl FnMut(&[u8]) -> io::Result<()>,
) -> io::Result<[u8; 32]> {
    let _claim = context.memory().claim(CHUNK_BYTES as u64).map_err(io::Error::other)?;
    let mut buffer = vec![0; CHUNK_BYTES];
    let mut hasher = Sha256::new();
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            return Ok(hasher.finalize().into());
        }
        hasher.update(&buffer[..count]);
        consume(&buffer[..count])?;
    }
}

/// Keep the first failure of a storage operation and any later retirement failure.
fn artifact<E: std::error::Error + Send + Sync + 'static>(cause: ArtifactError<E>) -> StoreError {
    match cause {
        ArtifactError::Storage(cause) => StoreError::Storage(cause),
        other => StoreError::Storage(io::Error::other(other)),
    }
}
