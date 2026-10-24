// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish complete initial generation custody before exposing its canonical directory.
use super::*;
use crate::protected_storage::directories::Mode;
use crate::setup_storage::{protection, staging};
use sugarsubstitute_contracts::installation::{Generation, GenerationId, PackageDigest};

pub(in crate::setup_storage) const INITIAL_PREFIX: &str = ".generation-init-";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema_version: u32,
    bundle_sha256: PackageDigest,
    state: String,
}

/// Verify existing canonical metadata; absence or damage is never permission to replace it.
pub(super) fn open(
    path: &Path,
    digest: &str,
    context: &JobContext,
) -> io::Result<(Directory, File)> {
    let (directory, marker, _) = inspect(path, Some(digest), context)?;
    Ok((directory, marker))
}

/// Pin protected generation identity for saved-data recovery without admitting executable bytes.
/// The completed configuration must independently match this package before being preserved.
pub(in crate::setup_storage) fn configuration(
    path: &Path,
    context: &JobContext,
) -> io::Result<(Directory, File, Generation)> {
    let id = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid("Configuration generation identity is absent"))?;
    let id = GenerationId::new(id.to_owned()).map_err(io::Error::other)?;
    let (directory, marker, package) = inspect(path, None, context)?;
    Ok((directory, marker, Generation { id, package }))
}

/// Inspect a protected generation without granting publication, execution or cleanup authority.
/// An optional digest binds the selected candidate; older unfinished generations are only
/// classified so an explicit first-install retry can preserve them and continue.
fn inspect(
    path: &Path,
    digest: Option<&str>,
    context: &JobContext,
) -> io::Result<(Directory, File, PackageDigest)> {
    protection::with(context, |protection| {
        protection.directory(path, Mode::Existing, context)?;
        let directory = Directory::open(path, false)?;
        let mut marker = directory.read("setup-generation.json")?;
        protection.verify_file(&marker)?;
        let record: Record = serde_json::from_slice(&files::read_bounded(&mut marker, 4096)?)
            .map_err(io::Error::other)?;
        if record.schema_version != 1
            || record.state != "staging"
            || digest.is_some_and(|digest| record.bundle_sha256.as_str() != digest)
        {
            return Err(invalid("Candidate generation metadata differs from its package"));
        }
        Ok((directory, marker, record.bundle_sha256))
    })
}

/// Require valid protected generation metadata and the absence of completed configuration.
/// Missing or damaged metadata stays an error. This never adopts the generation's payloads.
pub(in crate::setup_storage) fn unconfigured(
    path: &Path,
    digest: Option<&str>,
    context: &JobContext,
) -> io::Result<bool> {
    let (directory, _marker, _) = inspect(path, digest, context)?;
    check(context)?;
    match directory.read("installation-ready.json") {
        Ok(_) => Ok(false),
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => Ok(true),
        Err(cause) => Err(cause),
    }
}

/// Stage and flush complete initial custody, then publish without replacing any destination.
/// Interrupted private staging stays nonauthoritative; each retry uses a separate directory.
pub(super) fn create(
    path: &Path,
    digest: &str,
    context: &JobContext,
) -> io::Result<(Directory, File)> {
    let parent =
        Directory::open(path.parent().ok_or_else(|| invalid("Generation parent missing"))?, true)?;
    let (stage, identity) = protection::with(context, |protection| {
        let stage = staging::unique_path(&parent, INITIAL_PREFIX.trim_end_matches('-'))?;
        protection.directory(&stage, Mode::CreateNew, context)?;
        protection.directory(&stage.join("capture"), Mode::CreateNew, context)?;
        let directory = Directory::open(&stage, false)?;
        let bytes = serde_json::to_vec(&serde_json::json!({
            "schema_version": 1, "bundle_sha256": digest, "state": "staging",
        }))
        .map_err(io::Error::other)?;
        let mut file = protection.file(&stage.join("setup-generation.json"), context)?;
        use std::io::Write;
        file.write_all(&bytes)?;
        file.sync_all()?;
        check(context)?;
        Ok((stage, directory.identity()?))
    })?;
    protection::with(context, |protection| {
        protection.publish(&stage, path, identity, protection::Publication::Directory, context)
    })?;
    let result = open(path, digest, context)?;
    if result.0.identity()? != identity {
        return Err(invalid("Published generation directory changed"));
    }
    Ok(result)
}
