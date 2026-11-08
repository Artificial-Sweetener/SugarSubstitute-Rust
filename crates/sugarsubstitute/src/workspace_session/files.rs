// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve originals and publish complete bounded checkpoints through checked artifact I/O.
use super::*;
use sha2::{Digest, Sha256};
use std::io::Read;
use sugarsubstitute_contracts::{
    desktop_session::{decode_checkpoint, encode_checkpoint},
    execution::{JobContext, MemoryClaim},
};
use sugarsubstitute_supervision::{
    ArtifactError, ArtifactPublication, with_artifact_input, write_artifact,
};
const SCRATCH: usize = 64 * 1024;
const MAX_ORIGINAL_BYTES: u64 = 4 * 1024 * 1024 * 1024;
struct Candidate {
    bytes: Vec<u8>,
    _claim: MemoryClaim,
    digest: String,
    decoded: Result<SessionCheckpoint, SessionError>,
    protected: bool,
}
pub(super) fn load(
    store: &WorkspaceStore,
    context: &JobContext,
) -> Result<RestoredWorkspace, StoreError> {
    let mut result =
        RestoredWorkspace { checkpoint: None, rejected: vec![], protected: false, _memory: None };
    for (file, name) in [
        (CheckpointFile::Current, CURRENT_CHECKPOINT),
        (CheckpointFile::Previous, PREVIOUS_CHECKPOINT),
    ] {
        let candidate = match read(&store.directory.join(name), &store.scope, context) {
            Ok(candidate) => candidate,
            Err(StoreError::Protected) => {
                result.protected = true;
                tracing::warn!(operation="workspace_checkpoint_load",file=?file,"Oversized checkpoint preserved; attempting verified recovery");
                continue;
            }
            Err(cause) => return Err(cause),
        };
        let Some(candidate) = candidate else {
            continue;
        };
        result.protected |= candidate.protected;
        match candidate.decoded {
            Ok(checkpoint) => {
                result.checkpoint = Some((file, checkpoint));
                result._memory = Some(candidate._claim);
                break;
            }
            Err(_) => result.rejected.push(RejectedCheckpoint {
                file,
                digest: candidate.digest,
                protected: candidate.protected,
            }),
        }
    }
    Ok(result)
}
pub(super) fn save(
    store: &WorkspaceStore,
    checkpoint: &SessionCheckpoint,
    context: &JobContext,
) -> Result<SaveReceipt, StoreError> {
    if checkpoint.scope != store.scope {
        return Err(StoreError::Protected);
    }
    let _encoding =
        context.memory().claim(MAX_CHECKPOINT_BYTES as u64 * 2).map_err(io::Error::other)?;
    let bytes = encode_checkpoint(checkpoint)?;
    if decode_checkpoint(&bytes)? != *checkpoint {
        return Err(StoreError::Changed);
    }
    for asset in &checkpoint.assets {
        assets::verify(&store.assets, asset, context)?;
    }
    let current = store.directory.join(CURRENT_CHECKPOINT);
    let previous = store.directory.join(PREVIOUS_CHECKPOINT);
    let old = read(&current, &store.scope, context)?;
    let backup = read(&previous, &store.scope, context)?;
    if old.as_ref().is_some_and(|value| value.protected)
        || backup.as_ref().is_some_and(|value| value.protected)
    {
        return Err(StoreError::Protected);
    }
    for existing in [&old, &backup].into_iter().flatten() {
        if let Ok(value) = &existing.decoded
            && (value.revision >= checkpoint.revision || value.id == checkpoint.id)
        {
            return Err(SessionError::Stale.into());
        }
    }
    let mut receipt = SaveReceipt {
        checkpoint_id: checkpoint.id.clone(),
        revision: checkpoint.revision,
        bytes: bytes.len() as u64,
        kept_previous: false,
        evidence: vec![],
    };
    if let Some(candidate) = &backup
        && candidate.decoded.is_err()
    {
        receipt.evidence.push(preserve(&store.directory, &previous, candidate, context)?);
    }
    if let Some(candidate) = &old {
        if candidate.decoded.is_ok() {
            copy(&current, &previous, candidate, ArtifactPublication::ReplaceSelected, context)?;
            receipt.kept_previous = true;
        } else {
            receipt.evidence.push(preserve(&store.directory, &current, candidate, context)?);
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
    tracing::info!(operation="workspace_checkpoint_saved", checkpoint=%checkpoint.id,
        revision=checkpoint.revision, bytes=receipt.bytes, "Workspace checkpoint durably published");
    Ok(receipt)
}
fn read(
    path: &Path,
    scope: &SessionScope,
    context: &JobContext,
) -> Result<Option<Candidate>, StoreError> {
    let result = with_artifact_input(path, MAX_ORIGINAL_BYTES, context, |input| {
        if input.len() > MAX_CHECKPOINT_BYTES as u64 {
            return Err(StoreError::Protected);
        }
        let claim = context.memory().claim(input.len() * 4).map_err(io::Error::other)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(input.len() as usize).map_err(io::Error::other)?;
        input.read_to_end(&mut bytes)?;
        let digest = hex(&Sha256::digest(&bytes));
        let version = serde_json::from_slice::<serde_json::Value>(&bytes)
            .ok()
            .and_then(|value| value.get("version").and_then(serde_json::Value::as_u64));
        let mut decoded = decode_checkpoint(&bytes);
        let protected = version.is_some_and(|version| version != 1)
            || decoded.as_ref().is_ok_and(|value| value.scope != *scope);
        if protected {
            decoded = Err(SessionError::Stale);
        }
        Ok::<_, StoreError>(Candidate { bytes, _claim: claim, digest, decoded, protected })
    });
    match result {
        Ok(candidate) => Ok(Some(candidate)),
        Err(ArtifactError::Storage(cause)) if cause.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(cause) => Err(artifact(cause)),
    }
}
fn preserve(
    directory: &Path,
    source: &Path,
    candidate: &Candidate,
    context: &JobContext,
) -> Result<PathBuf, StoreError> {
    let path = directory.join(format!("workspace.rejected-{}.json", candidate.digest));
    match with_artifact_input(&path, MAX_ORIGINAL_BYTES, context, |input| digest(input, context)) {
        Ok(existing) if existing == candidate.digest => return Ok(path),
        Ok(_) => return Err(StoreError::Changed),
        Err(ArtifactError::Storage(cause)) if cause.kind() == io::ErrorKind::NotFound => {}
        Err(cause) => return Err(artifact(cause)),
    }
    copy(source, &path, candidate, ArtifactPublication::CreateOnly, context)?;
    Ok(path)
}
fn copy(
    source: &Path,
    target: &Path,
    candidate: &Candidate,
    publication: ArtifactPublication,
    context: &JobContext,
) -> Result<(), StoreError> {
    with_artifact_input(source, MAX_ORIGINAL_BYTES, context, |input| {
        if input.len() != candidate.bytes.len() as u64 {
            return Err(StoreError::Changed);
        }
        write_artifact(target, true, publication, candidate.bytes.len().max(1), context, |output| {
            let _scratch = context.memory().claim(SCRATCH as u64).map_err(io::Error::other)?;
            let mut buffer = vec![0; SCRATCH];
            let mut hash = Sha256::new();
            loop {
                let count = input.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                hash.update(&buffer[..count]);
                output.write_all(&buffer[..count])?;
            }
            if hex(&hash.finalize()) != candidate.digest {
                return Err(StoreError::Changed);
            }
            Ok::<_, StoreError>(())
        })
        .map_err(artifact)
    })
    .map_err(artifact)
}
fn digest(input: &mut dyn Read, context: &JobContext) -> Result<String, StoreError> {
    let _scratch = context.memory().claim(SCRATCH as u64).map_err(io::Error::other)?;
    let mut buffer = vec![0; SCRATCH];
    let mut hash = Sha256::new();
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            return Ok(hex(&hash.finalize()));
        }
        hash.update(&buffer[..count]);
    }
}
pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
pub(super) fn artifact<E: Error + Send + Sync + 'static>(cause: ArtifactError<E>) -> StoreError {
    match cause {
        ArtifactError::Storage(cause) => StoreError::Storage(cause),
        ArtifactError::Producer(cause) => {
            let cause: Box<dyn Error + Send + Sync> = Box::new(cause);
            match cause.downcast::<StoreError>() {
                Ok(cause) => *cause,
                Err(cause) => StoreError::Storage(io::Error::other(cause)),
            }
        }
        other => StoreError::Storage(io::Error::other(other)),
    }
}
