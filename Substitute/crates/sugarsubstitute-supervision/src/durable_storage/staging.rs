// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Allocate independent install attempts and publish complete directories without replacement.
use super::invalid;
use crate::durable_storage::files::{self, Directory};
use std::{
    io,
    path::{Path, PathBuf},
};
use windows::Win32::{
    Storage::FileSystem::{MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW},
    System::Com::CoCreateGuid,
};

/// Existing attempts never consume the namespace of a subsequent retry.
pub(crate) fn directory(parent: &Directory, prefix: &str) -> io::Result<PathBuf> {
    for _ in 0..8 {
        let path = unique_path(parent, prefix)?;
        match std::fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(cause) if cause.kind() == io::ErrorKind::AlreadyExists => {}
            Err(cause) => return Err(cause),
        }
    }
    Err(invalid("Could not allocate a unique installation attempt"))
}

/// Allocate a generation name for an owner that creates its directory atomically.
pub(crate) fn unique_path(parent: &Directory, prefix: &str) -> io::Result<PathBuf> {
    files::relative(prefix)?;
    if prefix.contains('/') {
        return Err(invalid("Staging prefix must be one child component"));
    }
    // SAFETY: This synchronous OS UUID generator owns no thread or COM apartment.
    let identity = unsafe { CoCreateGuid() }.map_err(io::Error::other)?;
    Ok(parent.path().join(format!("{prefix}-{identity:?}")))
}

/// OS-generated identifiers use the canonical lowercase 32-digit contract representation.
pub(crate) fn identity() -> io::Result<String> {
    // SAFETY: This synchronous OS UUID generator owns no thread or COM apartment.
    let identity = unsafe { CoCreateGuid() }.map_err(io::Error::other)?;
    Ok(format!("{:032x}", identity.to_u128()))
}

/// The caller retains both parent-directory chains through the same-volume rename.
/// Refuse every existing destination, including an empty directory.
pub(crate) fn publish(source: &Path, destination: &Path) -> io::Result<()> {
    let _operation =
        tracing::trace_span!(target: "sugarsubstitute::storage", "publish_rename").entered();
    files::lexical(source)?;
    files::lexical(destination)?;
    // SAFETY: Both UTF-16 paths stay live for this synchronous call. Omitting
    // REPLACE_EXISTING and COPY_ALLOWED preserves every existing destination.
    unsafe {
        MoveFileExW(
            &files::native_path(source),
            &files::native_path(destination),
            MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(io::Error::other)
}

/// Replace one verified selection record after flushing its complete staged bytes.
/// The caller retains the installation directory and exclusive selection-writer lock,
/// and has compared the original record under that lock. Both paths share one parent.
pub(crate) fn replace_record(source: &Path, destination: &Path) -> io::Result<()> {
    let _operation =
        tracing::trace_span!(target: "sugarsubstitute::storage", "replace_record").entered();
    files::lexical(source)?;
    files::lexical(destination)?;
    if source.parent() != destination.parent() || source == destination {
        return Err(invalid("Selection publication requires distinct sibling files"));
    }
    // SAFETY: Both strings outlive this synchronous call. The caller holds verified
    // ancestry and writer custody. Same-volume replacement never enables a copy fallback.
    unsafe {
        MoveFileExW(
            &files::native_path(source),
            &files::native_path(destination),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retries_preserve_prior_attempts_and_publication_never_replaces_unknown_content()
    -> io::Result<()> {
        let temporary = tempfile::tempdir()?;
        let parent = Directory::open(temporary.path(), false)?;
        assert!(unique_path(&parent, "nested/attempt").is_err());
        let first = directory(&parent, "attempt")?;
        std::fs::write(first.join("complete"), b"first")?;
        let second = directory(&parent, "attempt")?;
        std::fs::write(second.join("complete"), b"second")?;
        assert_ne!(first, second);
        let destination = parent.path().join("active");
        publish(&first, &destination)?;
        assert!(!first.exists());
        assert!(publish(&second, &destination).is_err());
        assert_eq!(std::fs::read(destination.join("complete"))?, b"first");
        assert_eq!(std::fs::read(second.join("complete"))?, b"second");
        let empty = parent.path().join("unknown-empty");
        std::fs::create_dir(&empty)?;
        assert!(publish(&second, &empty).is_err());
        assert!(empty.exists());
        Ok(())
    }
}
