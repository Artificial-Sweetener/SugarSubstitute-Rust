// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the unchanged schema-one installer/runtime provenance through read-only consumers.
use super::{RuntimePaths, Selection, check, invalid};
use crate::durable_storage::files::{self, Directory};
use serde::Deserialize;
use std::{fs::File, io};
use sugarsubstitute_contracts::{execution::JobContext, installation::PackageDigest};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadyRecord {
    schema_version: u32,
    bundle_sha256: String,
    pub(super) selection: Selection,
    pub(super) runtime: RuntimePaths,
}

/// Validate the original persisted format and native installation association without writing.
/// The returned file pins the exact original bytes through its consumer's entire operation.
pub(super) fn read(
    root: &Directory,
    directory: &Directory,
    expected: Option<&PackageDigest>,
    context: &JobContext,
) -> io::Result<(ReadyRecord, File)> {
    check(context)?;
    let mut file = directory.read("installation-ready.json")?;
    files::single_link(&file)?;
    let record: ReadyRecord = serde_json::from_slice(&files::read_bounded(&mut file, 1024 * 1024)?)
        .map_err(io::Error::other)?;
    if record.schema_version != 1
        || record.bundle_sha256.len() != 64
        || !record.bundle_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(invalid("Unsupported native installation record"));
    }
    if expected.is_some_and(|package| package.as_str() != record.bundle_sha256) {
        return Err(invalid("Saved configuration differs from its recorded package identity"));
    }
    record.selection.validate()?;
    if files::canonical_path(&record.selection.install_root, context)?
        != files::canonical_path(root.path(), context)?
    {
        return Err(invalid("Saved installation choices belong to another folder"));
    }
    for path in [
        &record.runtime.directory,
        &record.runtime.python,
        &record.runtime.frontend,
        &record.runtime.metadata,
    ] {
        files::lexical(path)?;
    }
    check(context)?;
    Ok((record, file))
}
