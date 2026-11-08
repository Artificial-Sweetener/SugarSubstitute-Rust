// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Associate the supported SugarCubes input metadata record with actual native archive custody.
use super::*;
use sugarsubstitute_contracts::{
    desktop_session::wire::{SessionCheckpoint, SessionInput},
    execution::JobContext,
};

/// Real documents and mask keys observed by the native input owner for one workflow surface.
pub(crate) struct WorkspaceSurface {
    pub(crate) workflow: String,
    pub(crate) surface: String,
    pub(crate) document: String,
    pub(crate) masks: Vec<String>,
}
/// One native input generation captured while its live document owner is retained.
pub(crate) struct WorkspaceInputs {
    pub(crate) archive: InputDocumentArchive,
    pub(crate) generation: u64,
    pub(crate) surfaces: Vec<WorkspaceSurface>,
}
#[derive(Debug, PartialEq)]
pub(super) struct Selection {
    pub(super) workflow: String,
    pub(super) surface: Option<String>,
    pub(super) mask: Option<String>,
}
pub(super) fn selection(root: &serde_json::Value) -> Result<Option<Selection>, StoreError> {
    let Some(value) = root
        .get("extra")
        .and_then(|extra| extra.as_object())
        .and_then(|extra| extra.get("sugarcubes_input_canvas"))
    else {
        return Ok(None);
    };
    let invalid = || {
        StoreError::Storage(io::Error::new(
            io::ErrorKind::InvalidData,
            "Unsupported workflow input association metadata",
        ))
    };
    let record = value.as_object().ok_or_else(invalid)?;
    let workflow = record.get("id").and_then(serde_json::Value::as_str).ok_or_else(invalid)?;
    let suffix = workflow.strip_prefix("workflow-").ok_or_else(invalid)?;
    let key = |name| -> Result<Option<String>, StoreError> {
        match record.get(name) {
            Some(serde_json::Value::Null) => Ok(None),
            Some(serde_json::Value::String(value))
                if !value.is_empty() && value.encode_utf16().count() <= 256 =>
            {
                Ok(Some(value.clone()))
            }
            _ => Err(invalid()),
        }
    };
    if record.get("version").and_then(serde_json::Value::as_u64) != Some(1)
        || suffix.is_empty()
        || suffix.len() > 100
        || !suffix.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        || !matches!(
            record.get("route").and_then(serde_json::Value::as_str),
            Some("input" | "output")
        )
    {
        return Err(invalid());
    }
    Ok(Some(Selection { workflow: workflow.into(), surface: key("surface")?, mask: key("mask")? }))
}
/// Select a content-addressed restore archive only after its existing codec and native metadata
/// independently validate every document association. Invalid originals remain untouched.
pub(crate) fn restore(
    directory: &std::path::Path,
    checkpoint: &SessionCheckpoint,
    context: &JobContext,
) -> Result<Option<InputDocumentStore>, StoreError> {
    let mut references = checkpoint.entries.iter().filter_map(|entry| entry.input.as_ref());
    let Some(first) = references.next() else {
        return Ok(None);
    };
    if references.any(|input| input.signature != first.signature) {
        return Err(StoreError::Unverified);
    }
    let mut digest = [0u8; 32];
    if first.signature.len() != 64 {
        return Err(StoreError::Unverified);
    }
    for (index, bytes) in first.signature.as_bytes().chunks_exact(2).enumerate() {
        let text = std::str::from_utf8(bytes).map_err(io::Error::other)?;
        digest[index] = u8::from_str_radix(text, 16).map_err(io::Error::other)?;
    }
    let store = InputDocumentStore::for_reference(
        directory.join("checkpoints").join(&first.signature),
        digest,
    )?;
    let restored = store.load_worker(context.memory(), context)?;
    let archive = &restored.archive.as_ref().ok_or(StoreError::Unverified)?.1;
    for entry in &checkpoint.entries {
        if let Some(input) = &entry.input {
            let graph =
                serde_json::from_str(&entry.document.graph_json).map_err(io::Error::other)?;
            let selected = selection(&graph)?.ok_or(StoreError::Unverified)?;
            if input.surface != selected.surface || input.mask != selected.mask {
                return Err(StoreError::Unverified);
            }
            if !crate::native_canvas::session::Session::archive_association(
                archive,
                &selected.workflow,
                input,
            ) {
                return Err(StoreError::Unverified);
            }
        }
    }
    Ok(Some(store))
}
impl WorkspaceInputs {
    /// Add only independently matched native references, then retain their exact archive in
    /// the existing codec under its content fingerprint. No graph nodes or edges are inspected.
    pub(crate) fn retain(
        &self,
        directory: &std::path::Path,
        checkpoint: &mut SessionCheckpoint,
        context: &JobContext,
    ) -> Result<(), StoreError> {
        let fingerprint = self.archive.fingerprint();
        let signature: String = fingerprint.iter().map(|byte| format!("{byte:02x}")).collect();
        let mut referenced = false;
        for entry in &mut checkpoint.entries {
            let graph: serde_json::Value = serde_json::from_str(&entry.document.graph_json)
                .map_err(|cause| StoreError::Storage(io::Error::other(cause)))?;
            let Some(selected) = selection(&graph)? else {
                entry.input = None;
                continue;
            };
            let native = self
                .surfaces
                .iter()
                .filter(|surface| surface.workflow == selected.workflow)
                .filter(|surface| {
                    selected.surface.as_ref().is_none_or(|key| key == &surface.surface)
                })
                .min_by(|left, right| left.document.cmp(&right.document));
            let Some(native) = native else {
                if entry.input.is_some() || selected.surface.is_some() {
                    return Err(StoreError::Unverified);
                }
                entry.input = None;
                continue;
            };
            if selected.mask.as_ref().is_some_and(|mask| !native.masks.contains(mask)) {
                return Err(StoreError::Unverified);
            }
            entry.input = Some(SessionInput {
                document_id: native.document.clone(),
                generation: self.generation,
                signature: signature.clone(),
                surface: selected.surface,
                mask: selected.mask,
            });
            referenced = true;
        }
        if referenced {
            files::retain(&directory.join("checkpoints").join(signature), &self.archive, context)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
