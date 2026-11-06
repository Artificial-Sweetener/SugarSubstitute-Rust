// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Copy and verify the independently owned root entry without replacing unknown bytes.
use super::*;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
};
use sugarsubstitute_update::package::PayloadHasher;

/// Hold the root executable against mutation through shortcut publication.
/// A missing owned file is recovered from its recorded source; changed files are preserved.
pub(super) fn ensure(
    root: &Directory,
    state: &Directory,
    record: &record::Record,
    protection: &mut Protection,
    context: &JobContext,
) -> io::Result<File> {
    match root.read(ENTRY) {
        Ok(mut file) => {
            protection.verify_file(&file)?;
            verify(&mut file, record, context)?;
            return Ok(file);
        }
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
        Err(cause) => return Err(cause),
    }
    replace(root, state, record, false, protection, context)
}

/// Copy a verified source and atomically publish it after the caller establishes replacement authority.
/// A conflicting root-directory capture leaves the journaled stage intact for recovery and retry
/// after that capture retires; publication never weakens an existing sharing exclusion.
pub(super) fn replace(
    root: &Directory,
    state: &Directory,
    record: &record::Record,
    replacing: bool,
    protection: &mut Protection,
    context: &JobContext,
) -> io::Result<File> {
    let source =
        Directory::open(&root.path().join("releases").join(record.source.id.as_str()), false)?;
    let mut input = source.read(SOURCE)?;
    verify(&mut input, record, context)?;
    let mut stage = publication::Stage::create(state, protection, context)?;
    let output = stage.file();
    let mut buffer = [0; 65536];
    loop {
        check(context)?;
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        output.write_all(&buffer[..count])?;
    }
    files::sync(output, context)?;
    verify(output, record, context)?;
    check(context)?;
    stage.publish_entry(root, replacing, protection, context)?;
    let mut file = root.read(ENTRY)?;
    protection.verify_file(&file)?;
    verify(&mut file, record, context)?;
    Ok(file)
}

/// Retire an earlier root launcher copy only while it is a protected single-link file with
/// exactly the recorded bytes. A copy that is still running cannot be deleted yet; it is kept
/// and retried on the next publication. Unknown or changed files are preserved.
pub(super) fn retire_earlier(
    root: &Directory,
    leaf: &str,
    record: &record::Record,
    protection: &Protection,
    context: &JobContext,
) -> io::Result<()> {
    let mut file = match root.read(leaf) {
        Ok(file) => file,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(cause) => return Err(cause),
    };
    if protection.verify_file(&file).is_err()
        || files::single_link(&file).is_err()
        || verify(&mut file, record, context).is_err()
    {
        tracing::warn!(
            operation = "earlier_launcher_retirement",
            "The earlier root launcher differs from protected ownership; it has been preserved"
        );
        return Ok(());
    }
    let identity = files::file_identity(&file)?;
    drop(file);
    match crate::durable_storage::retirement::file(root, leaf, identity) {
        Ok(()) => {
            tracing::info!(
                operation = "earlier_launcher_retirement",
                "Retired the earlier root launcher"
            );
            Ok(())
        }
        Err(cause)
            if matches!(
                cause.kind(),
                io::ErrorKind::PermissionDenied | io::ErrorKind::ResourceBusy
            ) || cause.raw_os_error() == Some(32) =>
        {
            tracing::warn!(
                operation = "earlier_launcher_retirement",
                cause = %cause,
                "The earlier root launcher is in use; it will be retired on a later publication"
            );
            Ok(())
        }
        Err(cause) => Err(cause),
    }
}

/// Match the retained handle to protected ownership without reopening its path.
pub(super) fn verify(
    file: &mut File,
    record: &record::Record,
    context: &JobContext,
) -> io::Result<()> {
    if files::length(file, context)? != record.bytes || hash(file, context)? != record.sha256 {
        return Err(invalid(
            "Stable launcher bytes differ from protected ownership; existing files have been preserved",
        ));
    }
    Ok(())
}

/// Stream with the caller's cancellation and deadline, restoring the retained input cursor.
pub(super) fn hash(file: &mut File, context: &JobContext) -> io::Result<String> {
    file.seek(SeekFrom::Start(0))?;
    let mut hasher = PayloadHasher::new();
    let mut buffer = [0; 65536];
    loop {
        check(context)?;
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    file.seek(SeekFrom::Start(0))?;
    Ok(hasher.finish_hex())
}
