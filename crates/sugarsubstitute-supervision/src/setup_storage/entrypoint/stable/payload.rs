// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Copy and verify the independently owned root executable without replacing unknown bytes.
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
    record: &record::Record,
    protection: &mut Protection,
    context: &JobContext,
) -> io::Result<File> {
    match root.read(EXECUTABLE) {
        Ok(mut file) => {
            protection.verify_file(&file)?;
            verify(&mut file, record, context)?;
            return Ok(file);
        }
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
        Err(cause) => return Err(cause),
    }
    replace(root, record, false, protection, context)
}

/// Copy a verified source and atomically publish it after the caller establishes replacement authority.
pub(super) fn replace(
    root: &Directory,
    record: &record::Record,
    replacing: bool,
    protection: &mut Protection,
    context: &JobContext,
) -> io::Result<File> {
    let source =
        Directory::open(&root.path().join("releases").join(record.source.id.as_str()), false)?;
    let mut input = source.read(EXECUTABLE)?;
    verify(&mut input, record, context)?;
    let mut stage = publication::Stage::create(root, protection, context)?;
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
    stage.publish(&root.path().join(EXECUTABLE), replacing, protection, context)?;
    let mut file = root.read(EXECUTABLE)?;
    protection.verify_file(&file)?;
    verify(&mut file, record, context)?;
    Ok(file)
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
