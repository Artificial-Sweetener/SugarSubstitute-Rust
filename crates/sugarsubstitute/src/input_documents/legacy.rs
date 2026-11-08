// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Associate a preserved legacy native archive once, through actual captured workflow metadata.
use super::*;
use std::{io::Read, path::Path};
use sugarsubstitute_contracts::{
    desktop_session::wire::{SessionCheckpoint, SessionScope},
    execution::JobContext,
};
use sugarsubstitute_supervision::{
    ArtifactError, ArtifactPublication, with_artifact_input, write_artifact,
};
const RECORD: &str = "workspace-association.json";
const RECORD_BYTES: u64 = 4096;
#[derive(serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Association {
    version: u32,
    scope: SessionScope,
    fingerprint: String,
}

/// Hold native workflow sync only when an unassociated preserved archive exists and the
/// selected namespace has no archive of its own. Presence never grants association.
pub(crate) fn plan(
    user: &Path,
    selected: &Path,
    scope: &SessionScope,
    context: &JobContext,
) -> Result<InputRestorePlan, StoreError> {
    for name in [CURRENT_ARCHIVE, PREVIOUS_ARCHIVE] {
        if present(&selected.join(name), context)? {
            return Ok(InputRestorePlan::Current);
        }
    }
    for name in [CURRENT_ARCHIVE, PREVIOUS_ARCHIVE] {
        if present(&user.join("input-canvas").join(name), context)? {
            match read_association(&user.join("input-canvas").join(RECORD), context) {
                Ok(Some(existing)) if existing.scope != *scope => {
                    return Ok(InputRestorePlan::Current);
                }
                Ok(_) | Err(StoreError::Unverified) => {}
                Err(cause) => return Err(cause),
            }
            return Ok(InputRestorePlan::AwaitingAssociation);
        }
    }
    Ok(InputRestorePlan::Current)
}
/// Decode the existing archive and admit its real identities only if every supplied supported
/// workflow association matches. Keep originals and reject competing profile/backend adoption.
/// The immutable selected copy is verified before native loading; marker creation is idempotent.
pub(crate) fn adopt(
    user: &Path,
    selected: &Path,
    checkpoint: &SessionCheckpoint,
    context: &JobContext,
) -> Result<InputRestorePlan, StoreError> {
    let passive = user.join("input-canvas");
    let source = InputDocumentStore::new(passive.clone())?;
    let loaded = source.load_worker(context.memory(), context)?;
    let Some((_, archive)) = loaded.archive else {
        return Ok(InputRestorePlan::Current);
    };
    let mut matches = false;
    for entry in &checkpoint.entries {
        let root = serde_json::from_str(&entry.document.graph_json).map_err(io::Error::other)?;
        let Some(selection) = workspace::selection(&root)? else {
            continue;
        };
        if !crate::native_canvas::session::Session::archive_selection(
            &archive,
            &selection.workflow,
            selection.surface.as_deref(),
            selection.mask.as_deref(),
        ) {
            return Ok(InputRestorePlan::Current);
        }
        matches = true;
    }
    if !matches {
        return Ok(InputRestorePlan::Current);
    }
    let fingerprint = archive.fingerprint();
    let signature: String = fingerprint.iter().map(|byte| format!("{byte:02x}")).collect();
    let expected =
        Association { version: 1, scope: checkpoint.scope.clone(), fingerprint: signature.clone() };
    let record = passive.join(RECORD);
    match read_association(&record, context)? {
        Some(existing) if existing != expected => return Err(StoreError::Unverified),
        Some(_) => {}
        None => {
            let bytes = serde_json::to_vec(&expected).map_err(io::Error::other)?;
            match write_artifact(
                &record,
                true,
                ArtifactPublication::CreateOnly,
                RECORD_BYTES as usize,
                context,
                |output| output.write_all(&bytes),
            ) {
                Ok(()) => {}
                Err(ArtifactError::Storage(cause))
                    if cause.kind() == io::ErrorKind::AlreadyExists =>
                {
                    if read_association(&record, context)?.as_ref() != Some(&expected) {
                        return Err(StoreError::Unverified);
                    }
                }
                Err(cause) => return Err(artifact(cause)),
            }
        }
    }
    let retained = selected.join("checkpoints").join(signature);
    files::retain(&retained, &archive, context)?;
    Ok(InputRestorePlan::Verified(InputDocumentStore::for_reference(retained, fingerprint)?))
}
fn present(path: &Path, context: &JobContext) -> Result<bool, StoreError> {
    match with_artifact_input(path, 4 * 1024 * 1024 * 1024, context, |_| Ok::<_, io::Error>(())) {
        Ok(()) => Ok(true),
        Err(ArtifactError::Storage(cause)) if cause.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(cause) => Err(artifact(cause)),
    }
}
fn read_association(path: &Path, context: &JobContext) -> Result<Option<Association>, StoreError> {
    match with_artifact_input(path, RECORD_BYTES, context, |input| {
        let _claim = context.memory().claim(RECORD_BYTES * 2).map_err(io::Error::other)?;
        let mut bytes = Vec::new();
        input.read_to_end(&mut bytes)?;
        let record: Association =
            serde_json::from_slice(&bytes).map_err(|_| StoreError::Unverified)?;
        use sugarsubstitute_contracts::desktop_session::{
            BackendNamespaceId, InstallationId, ProfileId, WorkspaceId,
        };
        if record.version != 1
            || InstallationId::new(record.scope.installation.clone()).is_err()
            || ProfileId::new(record.scope.profile.clone()).is_err()
            || WorkspaceId::new(record.scope.workspace.clone()).is_err()
            || BackendNamespaceId::new(record.scope.backend.clone()).is_err()
            || record.fingerprint.len() != 64
            || !record
                .fingerprint
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(StoreError::Unverified);
        }
        Ok::<_, StoreError>(record)
    }) {
        Ok(record) => Ok(Some(record)),
        Err(ArtifactError::Producer(cause)) => Err(cause),
        Err(ArtifactError::Storage(cause)) if cause.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(cause) => Err(artifact(cause)),
    }
}
fn artifact<E: Error + Send + Sync + 'static>(cause: ArtifactError<E>) -> StoreError {
    StoreError::Storage(io::Error::other(cause))
}
