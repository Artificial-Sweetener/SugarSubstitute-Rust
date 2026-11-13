// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Allocate independent install attempts and publish complete directories without replacement.
use super::{check, invalid};
use crate::durable_storage::{
    files::{self, Directory},
    protection::Protection,
};
use std::{
    io,
    path::{Path, PathBuf},
};
use sugarsubstitute_contracts::execution::JobContext;
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

/// Most interrupted attempts one reclaim removes; a later write removes the rest.
const RECLAIM_LIMIT: usize = 64;

/// Whether `name` is an attempt [`unique_path`] allocated for `prefix`.
fn is_attempt(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix).and_then(|rest| rest.strip_prefix('-')).is_some_and(|guid| {
        guid.len() == 36
            && guid.char_indices().all(|(index, character)| {
                if matches!(index, 8 | 13 | 18 | 23) {
                    character == '-'
                } else {
                    character.is_ascii_hexdigit()
                }
            })
    })
}

/// Remove the stage files `prefix` names in `parent`, a protected folder only this account can
/// write, whatever protection the stages themselves carry. The caller holds the exclusion every
/// writer of these stages takes. Folders, links and anything that cannot be removed now are kept.
pub(crate) fn reclaim_in_protected_folder(
    parent: &Directory,
    prefix: &str,
    context: &JobContext,
) -> io::Result<()> {
    use std::os::windows::fs::MetadataExt;
    for attempt in attempts(parent, prefix, context)? {
        check(context)?;
        let removed = std::fs::symlink_metadata(&attempt).and_then(|metadata| {
            if metadata.is_file() && metadata.file_attributes() & 0x400 == 0 {
                std::fs::remove_file(&attempt)
            } else {
                Err(invalid("Stage is not a regular file"))
            }
        });
        if let Err(cause) = removed {
            check(context)?;
            tracing::warn!(operation = "stage_reclaim_preserved", attempt = %attempt.display(),
                error = %cause, "An interrupted write was kept for a later attempt");
        }
    }
    Ok(())
}

/// The attempts `prefix` names in `parent`, at most [`RECLAIM_LIMIT`].
fn attempts(parent: &Directory, prefix: &str, context: &JobContext) -> io::Result<Vec<PathBuf>> {
    let mut attempts = Vec::new();
    for entry in parent.entries(context)? {
        check(context)?;
        let Ok(entry) = entry else { continue };
        let name = entry.file_name();
        if name.to_str().is_some_and(|name| is_attempt(name, prefix)) {
            attempts.push(parent.path().join(name));
            if attempts.len() == RECLAIM_LIMIT {
                break;
            }
        }
    }
    Ok(attempts)
}

/// Remove `path`, a protected regular single-link file, by its current identity.
fn remove_file(path: &Path, protection: &mut Protection, context: &JobContext) -> io::Result<()> {
    let parent =
        Directory::open(path.parent().ok_or_else(|| invalid("Stage parent missing"))?, false)?;
    let leaf =
        path.file_name().and_then(|leaf| leaf.to_str()).ok_or_else(|| invalid("Stage leaf"))?;
    let identity = match parent.read(leaf) {
        Ok(file) => files::file_identity(&file)?,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(cause) => return Err(cause),
    };
    protection.remove_journaled(path, identity, context)
}

/// Remove the attempts `prefix` names in `parent` that ended before publication: stage files,
/// or stage folders holding only the `children` the writer creates. The caller holds the
/// exclusion every writer of these attempts takes, so none of them is still being written.
/// Unknown, changed, linked or foreign content is kept, and so is an attempt that cannot be
/// removed now, such as one a scanner holds; a later write retries it. Only cancellation fails.
pub(crate) fn reclaim(
    parent: &Directory,
    prefix: &str,
    children: &[&str],
    protection: &mut Protection,
    context: &JobContext,
) -> io::Result<()> {
    for attempt in attempts(parent, prefix, context)? {
        check(context)?;
        let removed = if children.is_empty() {
            remove_file(&attempt, protection, context)
        } else {
            children
                .iter()
                .try_for_each(|child| remove_file(&attempt.join(child), protection, context))
                .and_then(|()| std::fs::remove_dir(&attempt))
        };
        if let Err(cause) = removed {
            check(context)?;
            tracing::warn!(operation = "stage_reclaim_preserved", attempt = %attempt.display(),
                error = %cause, "An interrupted write was kept for a later attempt");
        }
    }
    Ok(())
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

    #[test]
    fn only_attempts_of_the_named_writer_are_recognised() -> io::Result<()> {
        let temporary = tempfile::tempdir()?;
        let parent = Directory::open(temporary.path(), false)?;
        let attempt = unique_path(&parent, ".record-stage")?;
        let name = attempt.file_name().and_then(|name| name.to_str()).unwrap_or_default();
        assert!(is_attempt(name, ".record-stage"));
        assert!(!is_attempt(name, ".record"));
        assert!(!is_attempt(".record-stage-user-file", ".record-stage"));
        assert!(!is_attempt(&format!("{name}.bak"), ".record-stage"));
        Ok(())
    }
}
