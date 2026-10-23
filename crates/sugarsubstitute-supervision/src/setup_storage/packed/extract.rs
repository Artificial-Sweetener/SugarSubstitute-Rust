// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Stream one validated inventory into fresh files and verify each expanded payload.
use super::*;
use std::io::Write;

/// Retain each parent while creating its child; flush and check every file before publication.
pub(super) fn extract(
    reader: &mut impl Read,
    package: &Manifest,
    root: &Path,
    cancellation: impl Fn() -> io::Result<()>,
) -> io::Result<()> {
    for entry in package.entries() {
        cancellation()?;
        let path = root.join(entry.path());
        let _parent =
            tracing::trace_span!(target: "sugarsubstitute::storage", "archive_parent_open")
                .in_scope(|| {
                    Directory::open(
                        path.parent().ok_or_else(|| invalid("Archive parent missing"))?,
                        true,
                    )
                })?;
        let mut output =
            tracing::trace_span!(target: "sugarsubstitute::storage", "archive_file_create")
                .in_scope(|| files::create_new(&path))?;
        copy(reader, entry, &mut output, &cancellation)?;
        tracing::trace_span!(target: "sugarsubstitute::storage", "archive_file_flush")
            .in_scope(|| output.sync_all())?;
    }
    if reader.read(&mut [0])? != 0 {
        return Err(invalid("Unexpected installer payload data"));
    }
    Ok(())
}

/// Verify the supplied archive again before reusing independently authenticated cached output.
pub(super) fn verify(
    reader: &mut impl Read,
    package: &Manifest,
    cancellation: impl Fn() -> io::Result<()>,
) -> io::Result<()> {
    for entry in package.entries() {
        copy(reader, entry, &mut io::sink(), &cancellation)?;
    }
    if reader.read(&mut [0])? != 0 {
        return Err(invalid("Unexpected installer payload data"));
    }
    Ok(())
}
/// Share exact expanded-length and hash checks between extraction and cache reuse.
fn copy(
    reader: &mut impl Read,
    entry: &sugarsubstitute_update::package::Entry,
    output: &mut impl Write,
    cancellation: &impl Fn() -> io::Result<()>,
) -> io::Result<()> {
    let mut remaining = entry.bytes();
    let mut digest = sugarsubstitute_update::package::PayloadHasher::new();
    let mut buffer = [0; 65536];
    while remaining > 0 {
        cancellation()?;
        let limit = remaining.min(buffer.len() as u64) as usize;
        let count = reader.read(&mut buffer[..limit])?;
        if count == 0 {
            return Err(invalid("Installer payload is truncated"));
        }
        tracing::trace_span!(target: "sugarsubstitute::storage", "archive_file_write")
            .in_scope(|| output.write_all(&buffer[..count]))?;
        digest.update(&buffer[..count]);
        remaining -= count as u64;
    }
    if digest.finish_hex() != entry.digest() {
        return Err(invalid("Extracted installer payload failed verification"));
    }
    Ok(())
}
