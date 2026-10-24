// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Account for preserved content without adopting it or producing deletion authority.
use super::*;

/// Conservatively charge preserved bytes; unreadable or linked content blocks its namespace.
/// Cancellation and unexpected native failures remain errors with their original causes.
pub(in crate::setup_storage::cache) fn preserved_charge(
    path: &Path,
    namespace: Namespace,
    context: &JobContext,
) -> io::Result<u64> {
    preserved(path, namespace.budget(), namespace.directory(), context)
}
/// Unrecognized scope-level content consumes aggregate capacity without entering any namespace.
pub(in crate::setup_storage::cache) fn scope_charge(
    path: &Path,
    maximum: u64,
    context: &JobContext,
) -> io::Result<u64> {
    preserved(path, maximum, DIRECTORY, context)
}
/// Apply the same bounded preservation rules at either actual ownership level.
fn preserved(path: &Path, maximum: u64, namespace: &str, context: &JobContext) -> io::Result<u64> {
    match measure(path, context, &mut 0, 0) {
        Ok(bytes) => Ok(bytes),
        Err(cause) if record::kind(&cause) == io::ErrorKind::NotFound => Ok(0),
        Err(cause)
            if matches!(
                record::kind(&cause),
                io::ErrorKind::InvalidInput
                    | io::ErrorKind::InvalidData
                    | io::ErrorKind::PermissionDenied
            ) =>
        {
            tracing::warn!(operation="cache_unmeasured_content", namespace, cause=%cause, "Preserved content prevents bounded namespace admission");
            Ok(maximum)
        }
        Err(cause) => Err(cause),
    }
}
/// Read lengths only, reject reparse points and keep directory ancestry pinned during traversal.
fn measure(path: &Path, context: &JobContext, count: &mut usize, depth: usize) -> io::Result<u64> {
    check(context)?;
    *count += 1;
    if *count > MAX_OBJECTS || depth > MAX_DEPTH {
        return Err(invalid("Preserved cache inspection capacity exceeded"));
    }
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_attributes() & 0x400 != 0 {
        return Err(invalid("Preserved cache contains a link"));
    }
    if metadata.is_file() {
        return Ok(metadata.len());
    }
    if !metadata.is_dir() {
        return Err(invalid("Preserved cache contains unsupported content"));
    }
    let directory = Directory::open(path, false)?;
    measure_directory(&directory, context, count, depth)
}
/// Borrow pinned ancestors instead of multiplying native handles at every recursive depth.
fn measure_directory(
    directory: &Directory,
    context: &JobContext,
    count: &mut usize,
    depth: usize,
) -> io::Result<u64> {
    let mut bytes = 0u64;
    for entry in directory.entries(context)? {
        check(context)?;
        let entry = entry?;
        *count += 1;
        if *count > MAX_OBJECTS || depth + 1 > MAX_DEPTH {
            return Err(invalid("Preserved cache inspection capacity exceeded"));
        }
        let metadata = entry.current_metadata()?;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(invalid("Preserved cache contains a link"));
        }
        let next = if metadata.is_file() {
            metadata.len()
        } else if metadata.is_dir() {
            let leaf = entry.file_name();
            let child = directory.child(
                leaf.to_str().ok_or_else(|| invalid("Preserved cache directory is not Unicode"))?,
            )?;
            measure_directory(&child, context, count, depth + 1)?
        } else {
            return Err(invalid("Preserved cache contains unsupported content"));
        };
        bytes = bytes
            .checked_add(next)
            .ok_or_else(|| invalid("Preserved cache accounting overflowed"))?;
    }
    Ok(bytes)
}
