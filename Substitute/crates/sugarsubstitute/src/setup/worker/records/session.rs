// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep record snapshots, upload extents and pinned selected roots with their admitted custody.
use super::{MemoryClaim, Request, wire::invalid};
use serde_json::{Value, json};
use std::{io, path::PathBuf};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_supervision::{RecordBytes, RecordDirectory, WORKER_RECORD_BYTES};

/// Last exact native read, usable only for its own path and one compare-and-replace.
struct Snapshot {
    child: Option<String>,
    leaf: String,
    revision: u64,
    bytes: RecordBytes,
}
/// Bounded staged wire input has no filesystem effect until its explicit commit.
struct Upload {
    child: Option<String>,
    leaf: String,
    expected: Option<RecordBytes>,
    bytes: Vec<u8>,
    _memory: MemoryClaim,
}
/// No worker message can choose another path or increase this command's write authority.
pub(crate) struct RecordSession {
    path: PathBuf,
    writable: bool,
    root: Option<RecordDirectory>,
    snapshot: Option<Snapshot>,
    upload: Option<Upload>,
    revision: u64,
}
impl RecordSession {
    /// Inert selected-root authority; the original host command supplies both facts.
    pub(super) fn new(path: PathBuf, writable: bool) -> Self {
        Self { path, writable, root: None, snapshot: None, upload: None, revision: 0 }
    }
    /// Execute exactly one validated finite request on the existing execution worker.
    pub(super) fn perform(
        &mut self,
        operation: Request,
        context: &JobContext,
    ) -> io::Result<Vec<Value>> {
        operation.validate()?;
        if self.upload.is_some() && !matches!(operation, Request::Chunk { .. } | Request::Commit) {
            return Err(invalid());
        }
        let result = match operation {
            Request::Open { create } => {
                if create && !self.writable {
                    return Err(io::ErrorKind::PermissionDenied.into());
                }
                if self.root.is_none()
                    && !self.writable
                    && RecordDirectory::record_absent(
                        &self.path,
                        ".substitute-managed.json",
                        context,
                    )?
                {
                    return Ok(vec![json!({"absent":true})]);
                }
                if self.root.is_none() {
                    self.root =
                        Some(RecordDirectory::open(&self.path, create, self.writable, context)?);
                }
                json!({"path":self.root()?.path()})
            }
            Request::Page { restart } => {
                let root = self.root.as_mut().ok_or_else(invalid)?;
                json!({"entries":root.page(restart, context)?.into_iter().map(|entry| json!({"name":entry.name,"directory":entry.directory})).collect::<Vec<_>>()})
            }
            Request::Read { child, leaf } => {
                let bytes = self.root()?.read(child.as_deref(), &leaf, context)?;
                self.revision = self.revision.checked_add(1).ok_or_else(invalid)?;
                let result = if bytes.as_slice().is_empty() {
                    vec![json!({"hex":"","revision":self.revision})]
                } else {
                    bytes
                        .as_slice()
                        .chunks(8192)
                        .map(|chunk| json!({"hex":encode(chunk),"revision":self.revision}))
                        .collect()
                };
                self.snapshot = Some(Snapshot { child, leaf, revision: self.revision, bytes });
                return Ok(result);
            }
            Request::CreateChild { name } => {
                json!({"path":self.root()?.create_child(&name, context)?})
            }
            Request::Begin { child, leaf, revision } => {
                if !self.writable {
                    return Err(io::ErrorKind::PermissionDenied.into());
                }
                let expected = match revision {
                    Some(revision) => {
                        let snapshot = self.snapshot.as_ref().ok_or_else(invalid)?;
                        if snapshot.child != child
                            || snapshot.leaf != leaf
                            || snapshot.revision != revision
                        {
                            return Err(invalid());
                        }
                        self.snapshot.take().map(|snapshot| snapshot.bytes)
                    }
                    None => None,
                };
                let memory =
                    context.memory().claim(WORKER_RECORD_BYTES as u64).map_err(io::Error::other)?;
                let mut bytes = Vec::new();
                bytes.try_reserve_exact(WORKER_RECORD_BYTES).map_err(io::Error::other)?;
                self.upload = Some(Upload { child, leaf, expected, bytes, _memory: memory });
                json!({})
            }
            Request::Chunk { offset, hex } => {
                let upload = self.upload.as_mut().ok_or_else(invalid)?;
                if offset != upload.bytes.len()
                    || offset
                        .checked_add(hex.len() / 2)
                        .is_none_or(|length| length > WORKER_RECORD_BYTES)
                {
                    return Err(invalid());
                }
                upload.bytes.extend(
                    hex.as_bytes().chunks_exact(2).map(|pair| digit(pair[0]) * 16 + digit(pair[1])),
                );
                json!({})
            }
            Request::Commit => {
                let upload = self.upload.take().ok_or_else(invalid)?;
                match upload.expected {
                    Some(expected) => self.root()?.replace(
                        ".substitute-managed.json",
                        upload.child.as_deref(),
                        &upload.leaf,
                        &expected,
                        &upload.bytes,
                        context,
                    )?,
                    None => self.root()?.create(
                        upload.child.as_deref(),
                        &upload.leaf,
                        &upload.bytes,
                        context,
                    )?,
                }
                json!({})
            }
        };
        Ok(vec![result])
    }
    fn root(&self) -> io::Result<&RecordDirectory> {
        self.root.as_ref().ok_or_else(invalid)
    }
}
/// Encode binary chunks without paths, schema interpretation or another transport dependency.
fn encode(bytes: &[u8]) -> String {
    const DIGITS: &[u8] = b"0123456789abcdef";
    bytes
        .iter()
        .flat_map(|byte| {
            [DIGITS[(byte >> 4) as usize] as char, DIGITS[(byte & 15) as usize] as char]
        })
        .collect()
}
/// Validation already restricts every input byte to canonical lowercase hexadecimal.
fn digit(byte: u8) -> u8 {
    if byte <= b'9' { byte - b'0' } else { byte - b'a' + 10 }
}
