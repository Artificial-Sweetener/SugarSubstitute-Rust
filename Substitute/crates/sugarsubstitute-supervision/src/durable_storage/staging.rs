// SPDX-License-Identifier: AGPL-3.0-or-later
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

/// Digits of the ordinal that numbers each folder [`numbered_directory`] creates.
const ORDINAL_DIGITS: usize = 20;

/// Create a folder in `parent` with `create`, named `<prefix>-<ordinal>` one after the highest
/// such folder there, so names order folders by when they were made. When the ordinals are
/// exhausted or that name cannot be created, the folder is named `<prefix>-<GUID>` instead,
/// which [`retain_numbered`] never removes. The caller serializes every writer of these folders.
pub(crate) fn numbered_directory(
    parent: &Directory,
    prefix: &str,
    mut create: impl FnMut(&Path) -> io::Result<()>,
    context: &JobContext,
) -> io::Result<PathBuf> {
    files::flat_leaf(prefix)?;
    let next = numbered(parent, prefix, context)?
        .iter()
        .map(|(ordinal, _)| *ordinal)
        .max()
        .map_or(Some(1), |highest| highest.checked_add(1));
    if let Some(next) = next {
        let path = parent.path().join(format!("{prefix}-{next:0ORDINAL_DIGITS$}"));
        match create(&path) {
            Ok(()) => return Ok(path),
            Err(cause) => {
                check(context)?;
                tracing::warn!(operation = "numbered_folder_renamed", folder = %path.display(),
                    error = %cause, "The next numbered folder could not be created; a unique name is used");
            }
        }
    }
    let path = unique_path(parent, prefix)?;
    create(&path)?;
    Ok(path)
}

/// Remove every numbered folder `prefix` names in `parent` except the `keep` newest and
/// `spared`, with all they hold. Links inside are removed, never followed. Entries that are
/// not plain folders, and folders that cannot be removed now, are kept and reported; a later
/// call retries them. Only cancellation fails.
pub(crate) fn retain_numbered(
    parent: &Directory,
    prefix: &str,
    keep: usize,
    spared: &Path,
    context: &JobContext,
) -> io::Result<()> {
    let mut folders = numbered(parent, prefix, context)?;
    folders.sort_unstable_by_key(|(ordinal, _)| std::cmp::Reverse(*ordinal));
    for (_, path) in folders.into_iter().skip(keep).filter(|(_, path)| path != spared) {
        check(context)?;
        if let Err(cause) = std::fs::remove_dir_all(&path) {
            check(context)?;
            tracing::warn!(operation = "numbered_folder_preserved", folder = %path.display(),
                error = %cause, "An older folder was kept for a later attempt");
        }
    }
    Ok(())
}

/// The plain folders named `<prefix>-<ordinal>` in `parent`, with their ordinals.
fn numbered(
    parent: &Directory,
    prefix: &str,
    context: &JobContext,
) -> io::Result<Vec<(u64, PathBuf)>> {
    let mut folders = Vec::new();
    for entry in parent.entries(context)? {
        check(context)?;
        let Ok(entry) = entry else { continue };
        let name = entry.file_name();
        let Some(ordinal) = name
            .to_str()
            .and_then(|name| name.strip_prefix(prefix))
            .and_then(|rest| rest.strip_prefix('-'))
            .filter(|digits| {
                digits.len() == ORDINAL_DIGITS && digits.bytes().all(|byte| byte.is_ascii_digit())
            })
            .and_then(|digits| digits.parse().ok())
        else {
            continue;
        };
        if entry.file_type().is_ok_and(|kind| kind.is_dir() && !kind.is_symlink()) {
            folders.push((ordinal, parent.path().join(name)));
        }
    }
    Ok(folders)
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
    let parent = Directory::parent_of(path)?;
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

    fn plain(path: &Path) -> io::Result<()> {
        std::fs::create_dir(path)
    }

    #[test]
    fn numbered_folders_count_up_and_only_the_newest_are_retained()
    -> crate::setup_storage::test_worker::TestResult {
        crate::setup_storage::test_worker::run(|context, _| {
            let temporary = tempfile::tempdir()?;
            let parent = Directory::open(temporary.path(), false)?;
            let outside = tempfile::tempdir()?;
            std::fs::write(outside.path().join("elsewhere"), b"never followed")?;
            let unrelated = parent.path().join("set-aside-notes");
            std::fs::create_dir(&unrelated)?;
            let mut made = Vec::new();
            for _ in 0..4 {
                let folder = numbered_directory(&parent, "set-aside", plain, context)?;
                std::fs::create_dir(folder.join("nested"))?;
                std::fs::write(folder.join("nested").join("record"), b"kept for diagnosis")?;
                crate::durable_storage::files::reparse_fixture::junction(
                    &folder.join("link"),
                    outside.path(),
                    context,
                )?;
                made.push(folder);
            }
            assert!(made[3].ends_with("set-aside-00000000000000000004"));
            retain_numbered(&parent, "set-aside", 2, &made[0], context)?;
            assert!(made[0].join("nested").join("record").is_file(), "the spared folder went");
            assert!(!made[1].exists());
            assert!(made[2].join("nested").join("record").is_file());
            assert!(made[3].join("nested").join("record").is_file());
            assert!(unrelated.is_dir());
            assert_eq!(std::fs::read(outside.path().join("elsewhere"))?, b"never followed");
            let next = numbered_directory(&parent, "set-aside", plain, context)?;
            assert!(next.ends_with("set-aside-00000000000000000005"));
            Ok(())
        })
    }

    #[test]
    fn exhausted_or_occupied_numbers_fall_back_to_a_unique_name()
    -> crate::setup_storage::test_worker::TestResult {
        crate::setup_storage::test_worker::run(|context, _| {
            let temporary = tempfile::tempdir()?;
            let parent = Directory::open(temporary.path(), false)?;
            let last = parent.path().join(format!("set-aside-{}", u64::MAX));
            std::fs::create_dir(&last)?;
            let exhausted = numbered_directory(&parent, "set-aside", plain, context)?;
            assert!(exhausted.is_dir() && exhausted != last);
            let name = exhausted.file_name().and_then(|name| name.to_str()).unwrap_or_default();
            assert!(is_attempt(name, "set-aside"), "{name} is not a unique name");
            std::fs::remove_dir(&last)?;
            // A file already holds the next number's name.
            std::fs::write(parent.path().join("set-aside-00000000000000000001"), b"in the way")?;
            let occupied = numbered_directory(&parent, "set-aside", plain, context)?;
            let name = occupied.file_name().and_then(|name| name.to_str()).unwrap_or_default();
            assert!(occupied.is_dir() && is_attempt(name, "set-aside"), "{name}");
            retain_numbered(&parent, "set-aside", 0, &occupied, context)?;
            assert!(exhausted.is_dir() && occupied.is_dir(), "unique names are never removed");
            Ok(())
        })
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
