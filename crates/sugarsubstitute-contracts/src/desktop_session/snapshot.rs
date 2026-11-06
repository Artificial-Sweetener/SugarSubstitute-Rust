// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit coherent checkpoint membership and semantic final-output coordinates.
use super::{
    MAX_ASSET_BYTES, MAX_CHECKPOINT_BYTES, SessionError,
    primitives::{digest, file, scope, uuid},
    wire::*,
};
use std::collections::{BTreeMap, BTreeSet};

/// Match the exact retained output including scene/source/list/batch coordinates.
fn selection(value: Option<&SessionSelection>, outputs: &BTreeMap<&str, &SessionOutput>) -> bool {
    value.is_none_or(|value| {
        outputs.get(value.output_id.as_str()).is_some_and(|output| {
            output.source_id == value.source_id
                && output.scene.as_ref().map(|scene| &scene.scene_id) == value.scene_id.as_ref()
                && output.batch_index == value.batch_index
                && output.list_index == value.list_index
        })
    })
}

/// Validate a receipt independently of current file availability or embedded recipe metadata.
pub(super) fn output(value: &SessionOutput) -> bool {
    [&value.id, &value.entry_id, &value.backend, &value.run_id, &value.source_id]
        .into_iter()
        .all(|id| uuid(id))
        && digest(&value.digest)
        && value.asset_digest.as_deref().is_none_or(digest)
        && value.prompt_id.as_ref().is_none_or(|prompt| !prompt.is_empty())
        && file(&value.file)
        && (value.file.storage != "temp" || value.asset_digest.as_ref() == Some(&value.digest))
        && value.asset_digest.as_ref().is_none_or(|asset| asset == &value.digest)
        && value.scene.as_ref().is_none_or(|scene| {
            uuid(&scene.group_id) && uuid(&scene.scene_id) && scene.order < scene.count
        })
}

/// Validate a document occurrence without matching by title, saved path or graph contents.
fn entry(value: &SessionEntry, checkpoint: &SessionCheckpoint) -> bool {
    if !super::submissions::valid(value, &checkpoint.scope)
        || !super::scenes::valid(value)
        || !super::navigation::valid(value)
        || !uuid(&value.id)
        || super::validate_document(&value.document).is_err()
        || serde_json::to_vec(&value.view).map_or(true, |bytes| bytes.len() > 1024 * 1024)
        || value
            .input
            .as_ref()
            .is_some_and(|input| input.document_id.is_empty() || !digest(&input.signature))
    {
        return false;
    }
    let mut sources = BTreeSet::new();
    for source in &value.sources {
        if !uuid(&source.id)
            || !sources.insert(&source.id)
            || source.node_id.is_empty()
            || source.execution_path.is_empty()
            || source.graph_revision > value.document.graph_revision
        {
            return false;
        }
    }
    let mut outputs = BTreeMap::new();
    let mut positions = BTreeSet::new();
    for item in &value.outputs {
        let position = (
            &item.run_id,
            item.scene.as_ref().map(|scene| &scene.scene_id),
            &item.source_id,
            item.list_index,
            item.batch_index,
            item.image_index,
        );
        if !output(item)
            || item.entry_id != value.id
            || item.backend != checkpoint.scope.backend
            || !sources.contains(&item.source_id)
            || outputs.insert(item.id.as_str(), item).is_some()
            || !positions.insert(position)
        {
            return false;
        }
    }
    if !selection(value.view.selection.as_ref(), &outputs)
        || !selection(value.view.comparison.selection.as_ref(), &outputs)
        || (value.view.comparison.enabled && value.view.comparison.selection.is_none())
    {
        return false;
    }
    let mut cameras = BTreeSet::new();
    value.view.cameras.iter().all(|camera| {
        outputs.contains_key(camera.output_id.as_str()) && cameras.insert(&camera.output_id)
    })
}

/// Validate a v1 checkpoint's shape, bounds, identities and complete reference relationships.
///
/// Encoded storage bytes require [`decode_checkpoint`] admission before parsing. Callers
/// construct immutable accepted snapshots after this check; mutable DTOs must be revalidated.
/// # Errors
/// Rejects unknown versions, oversized fields, unsafe descriptors and inconsistent references.
pub fn validate_checkpoint(value: &SessionCheckpoint) -> Result<(), SessionError> {
    if !value.valid()
        || !scope(&value.scope)
        || !uuid(&value.id)
        || value
            .previous_id
            .as_ref()
            .is_some_and(|previous| !uuid(previous) || previous == &value.id)
        || value.producer.is_empty()
        || value.frontend.is_empty()
        || value.sugarcubes.is_empty()
        || value
            .placement
            .as_ref()
            .is_some_and(|placement| super::validate_placement(placement).is_err())
    {
        return Err(SessionError::Invalid);
    }
    let mut entries = BTreeSet::new();
    let mut outputs = BTreeSet::new();
    let mut prompts = BTreeSet::new();
    let mut count = 0;
    for item in &value.entries {
        if !entries.insert(&item.id) || !entry(item, value) {
            return Err(SessionError::Invalid);
        }
        count += item.outputs.len();
        for submission in &item.submissions {
            if !prompts.insert(&submission.prompt_id) {
                return Err(SessionError::Invalid);
            }
        }
        for output in &item.outputs {
            if !outputs.insert(&output.id) {
                return Err(SessionError::Invalid);
            }
        }
    }
    if count > 65536
        || value
            .active_entry
            .as_ref()
            .map_or(!entries.is_empty(), |active| !entries.contains(active))
    {
        return Err(SessionError::Invalid);
    }
    let assets: BTreeMap<_, _> = value.assets.iter().map(|asset| (&asset.digest, asset)).collect();
    if assets.len() != value.assets.len()
        || value.assets.iter().any(|asset| {
            !digest(&asset.digest)
                || if matches!(asset.extension.as_str(), "mp4" | "webm") {
                    asset.media != "video"
                } else {
                    asset.media != "image"
                }
        })
        || value.assets.iter().map(|asset| asset.bytes).sum::<u64>() > MAX_ASSET_BYTES
    {
        return Err(SessionError::Invalid);
    }
    let mut referenced = BTreeSet::new();
    for item in &value.entries {
        for output in &item.outputs {
            let Some(asset_digest) = &output.asset_digest else {
                continue;
            };
            let Some(asset) = assets.get(asset_digest) else {
                return Err(SessionError::Invalid);
            };
            if asset.digest != output.digest
                || asset.bytes != output.bytes
                || asset.media != output.media
            {
                return Err(SessionError::Invalid);
            }
            referenced.insert(asset_digest);
        }
    }
    if referenced.len() != assets.len() {
        return Err(SessionError::Invalid);
    }
    if serde_json::to_vec(value).map_err(|_| SessionError::Invalid)?.len() > MAX_CHECKPOINT_BYTES {
        return Err(SessionError::TooLarge);
    }
    Ok(())
}

/// Decode a bounded checkpoint; stores preserve rejected bytes before attempting recovery.
/// # Errors
/// Rejects malformed JSON, duplicate/unknown members, newer versions and invalid semantics.
pub fn decode_checkpoint(bytes: &[u8]) -> Result<SessionCheckpoint, SessionError> {
    if bytes.len() > MAX_CHECKPOINT_BYTES {
        return Err(SessionError::TooLarge);
    }
    let value = serde_json::from_slice(bytes).map_err(|_| SessionError::Invalid)?;
    validate_checkpoint(&value)?;
    Ok(value)
}

/// Encode only a fully validated v1 checkpoint for storage or a dedicated bounded transfer.
/// # Errors
/// Rejects invalid DTO construction or encoded bytes exceeding checkpoint admission.
pub fn encode_checkpoint(value: &SessionCheckpoint) -> Result<Vec<u8>, SessionError> {
    validate_checkpoint(value)?;
    serde_json::to_vec(value).map_err(|_| SessionError::Invalid)
}
