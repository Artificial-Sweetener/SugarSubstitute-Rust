// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve selection continuity independently of its mutable state directory.
use super::*;

const CONTENT: &[u8] = br#"{"schema_version":1,"namespace":".selection-state"}"#;

/// Pin complete selection state and its immutable continuity marker on an admitted worker.
/// Maintenance enables continuity for earlier complete protected state. Read-only inspection
/// leaves its marker untouched. Only the atomic initializer creates a state directory.
pub(super) fn open(
    installation: &Directory,
    enable: bool,
    context: &JobContext,
) -> io::Result<Option<(Directory, Option<File>)>> {
    protection::with(context, |protection| {
        let _transition =
            if enable { Some(transition(installation, protection, context)?) } else { None };
        let mut marker = read_marker(installation, protection)?;
        let path = installation.path.join(DIRECTORY);
        let root = match Directory::open(&path, false) {
            Ok(directory) => directory,
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
                if marker.is_some() {
                    return Err(super::super::invalid(
                        "Installation selection directory is missing; repair is required",
                    ));
                }
                return Ok(None);
            }
            Err(cause) => return Err(cause),
        };
        protection.directory(&path, Mode::Existing, context)?;
        verify_record(&root, protection)?;
        if enable && marker.is_none() {
            marker = Some(publish_marker(installation, &root, protection, context)?);
        }
        Ok(Some((root, marker)))
    })
}

/// Publish continuity only while the caller retains transition custody and complete state.
pub(super) fn publish_marker(
    installation: &Directory,
    root: &Directory,
    protection: &Protection,
    context: &JobContext,
) -> io::Result<File> {
    let _writer = writer(root, protection, context).map_err(io::Error::other)?;
    verify_record(root, protection)?;
    if let Some(marker) = read_marker(installation, protection)? {
        return Ok(marker);
    }
    let staged = staging::unique_path(root, ".selection-enable-stage")?;
    let mut file = protection.file(&staged, context)?;
    file.write_all(CONTENT)?;
    file.sync_all()?;
    drop(file);
    check(context)?;
    staging::publish(&staged, &installation.path.join(NAMESPACE_MARKER))?;
    read_marker(installation, protection)?
        .ok_or_else(|| super::super::invalid("Published selection continuity marker is absent"))
}

/// Missing or malformed authoritative records cannot enable or recreate installation state.
fn verify_record(root: &Directory, protection: &Protection) -> io::Result<()> {
    let mut file = root.read(RECORD)?;
    protection.verify_file(&file)?;
    InstallationRecord::decode(&files::read_bounded(&mut file, MAX_RECORD_BYTES)?)
        .map_err(io::Error::other)?;
    Ok(())
}

/// Serialize first selection enablement with publication by an earlier installer.
/// The protected rendezvous remains on disk; its OS lock has no stale-owner timeout.
pub(super) fn transition(
    installation: &Directory,
    protection: &Protection,
    context: &JobContext,
) -> io::Result<File> {
    let lock =
        protection.rendezvous(&installation.path.join(".selection-transition-lock"), context)?;
    lock.try_lock().map_err(io::Error::from)?;
    Ok(lock)
}

/// Validate the complete bounded marker and deny modification while its handle remains held.
fn read_marker(installation: &Directory, protection: &Protection) -> io::Result<Option<File>> {
    let mut file = match installation.read(NAMESPACE_MARKER) {
        Ok(file) => file,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(cause) => return Err(cause),
    };
    protection.verify_file(&file)?;
    let value: serde_json::Value =
        serde_json::from_slice(&files::read_bounded(&mut file, 4096)?).map_err(io::Error::other)?;
    if value != serde_json::json!({"schema_version":1,"namespace":DIRECTORY}) {
        return Err(super::super::invalid("Installation selection continuity marker is invalid"));
    }
    Ok(Some(file))
}

#[cfg(test)]
mod tests;
