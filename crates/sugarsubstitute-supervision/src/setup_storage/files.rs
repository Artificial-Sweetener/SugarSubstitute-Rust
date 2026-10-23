// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pin each resolved directory against replacement and reject reparse-point traversal.
mod canonical;
mod child;
#[allow(unsafe_code)]
mod identity;
use super::invalid;
pub(super) use identity::{file_identity, single_link};
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::{Component, Path, PathBuf},
};
const REPARSE_POINT: u32 = 0x400;
const OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const BACKUP_SEMANTICS: u32 = 0x0200_0000;

pub(super) struct Directory {
    pub(super) path: PathBuf,
    pub(super) _pins: Vec<File>,
}

impl Directory {
    /// Pin and validate each component before traversing its children. Existing directories
    /// need only read access; creation is attempted after an actual missing-component result.
    pub(super) fn open(path: &Path, create: bool) -> io::Result<Self> {
        let _operation =
            tracing::trace_span!(target: "sugarsubstitute::storage", "directory_open").entered();
        lexical(path)?;
        let mut current = PathBuf::new();
        let mut pins = Vec::new();
        for component in path.components() {
            current.push(component);
            if matches!(component, Component::Prefix(_)) {
                continue;
            }
            pins.push(open_component(
                &current,
                create && matches!(component, Component::Normal(_)),
            )?);
        }
        Ok(Self { path: path.to_owned(), _pins: pins })
    }

    pub(super) fn read(&self, leaf: &str) -> io::Result<File> {
        let _operation =
            tracing::trace_span!(target: "sugarsubstitute::storage", "file_open_read").entered();
        relative(leaf)?;
        let file = OpenOptions::new()
            .read(true)
            .share_mode(1)
            .custom_flags(OPEN_REPARSE_POINT)
            .open(self.path.join(leaf))?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.file_attributes() & REPARSE_POINT != 0 {
            return Err(invalid("Installation input must be a regular file"));
        }
        Ok(file)
    }
}

/// The caller pins all preceding components. A creation race still requires opening and
/// validating the actual resulting handle; AlreadyExists never grants directory custody.
fn open_component(path: &Path, create: bool) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).share_mode(3).custom_flags(OPEN_REPARSE_POINT | BACKUP_SEMANTICS);
    let pin = match options.open(path) {
        Ok(pin) => pin,
        Err(cause) if create && cause.kind() == io::ErrorKind::NotFound => {
            match tracing::trace_span!(target: "sugarsubstitute::storage", "directory_create")
                .in_scope(|| std::fs::create_dir(path))
            {
                Ok(()) => {}
                Err(cause) if cause.kind() == io::ErrorKind::AlreadyExists => {}
                Err(cause) => return Err(cause),
            }
            options.open(path)?
        }
        Err(cause) => return Err(cause),
    };
    let metadata = pin.metadata()?;
    if !metadata.is_dir() || metadata.file_attributes() & REPARSE_POINT != 0 {
        return Err(invalid("Installation directories must not traverse links or junctions"));
    }
    Ok(pin)
}

pub(super) fn lexical(path: &Path) -> io::Result<()> {
    if !path.is_absolute()
        || path.as_os_str().len() > 4096
        || path.parent().is_none()
        || path.components().any(|part| matches!(part, Component::ParentDir | Component::CurDir))
        || path.to_string_lossy().starts_with(r"\\")
    {
        return Err(invalid("Select an absolute local folder below a drive root"));
    }
    for part in path.components() {
        if let Component::Normal(value) = part {
            relative(value.to_str().ok_or_else(|| invalid("Folder name is not Unicode"))?)?;
        }
    }
    Ok(())
}

pub(super) fn relative(value: &str) -> io::Result<()> {
    sugarsubstitute_update::package::relative_path(value).map_err(io::Error::other)
}

pub(super) fn read_bounded(file: &mut File, maximum: usize) -> io::Result<Vec<u8>> {
    if file.metadata()?.len() > maximum as u64 {
        return Err(invalid("Installation record is too large"));
    }
    let mut bytes = Vec::new();
    file.take(maximum as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(invalid("Installation record exceeded its bound"));
    }
    Ok(bytes)
}

pub(super) fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let _operation =
        tracing::trace_span!(target: "sugarsubstitute::storage", "record_write_flush").entered();
    let mut file = create_new(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Complete a known immutable generation record, accepting only byte-identical earlier output.
/// Flush a unique sibling before publication so interruption never exposes a partial final file.
pub(super) fn write_immutable(
    path: &Path,
    bytes: &[u8],
    create: impl FnOnce(&Path) -> io::Result<File>,
) -> io::Result<()> {
    let parent = Directory::open(
        path.parent().ok_or_else(|| invalid("Immutable record parent absent"))?,
        false,
    )?;
    let leaf = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid("Immutable record leaf absent"))?;
    match parent.read(leaf) {
        Ok(mut original) => {
            return if read_bounded(&mut original, bytes.len())? == bytes {
                Ok(())
            } else {
                Err(invalid("Immutable generation record differs from the expected bytes"))
            };
        }
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
        Err(cause) => return Err(cause),
    }
    let staged = super::staging::unique_path(&parent, ".record-stage")?;
    let mut file = create(&staged)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    super::staging::publish(&staged, path)
}

/// Create an ordinary output only when its name is absent.
pub(super) fn create_new(path: &Path) -> io::Result<File> {
    let _operation =
        tracing::trace_span!(target: "sugarsubstitute::storage", "file_create").entered();
    OpenOptions::new().write(true).create_new(true).share_mode(0).open(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directory_creation_opens_existing_ancestors_without_creation_attempts() -> io::Result<()> {
        let temporary = tempfile::tempdir()?;
        let trace = super::super::test_worker::trace::StorageTrace::default();
        tracing::subscriber::with_default(trace.clone(), || -> io::Result<()> {
            let existing = Directory::open(temporary.path(), true)?;
            assert_eq!(trace.completed("directory_create"), 0);
            let path = existing.path.join("new").join("nested");
            let created = Directory::open(&path, true)?;
            assert!(created.path.is_dir());
            assert_eq!(trace.completed("directory_create"), 2);
            let reopened = Directory::open(&path, true)?;
            assert_eq!(reopened.path, created.path);
            assert_eq!(trace.completed("directory_create"), 2);
            assert!(std::fs::rename(&path, existing.path.join("moved")).is_err());
            Ok(())
        })
    }

    #[test]
    fn creating_a_directory_preserves_an_existing_file_collision() -> io::Result<()> {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("authored");
        std::fs::write(&path, b"preserved")?;
        assert!(Directory::open(&path, true).is_err());
        assert_eq!(std::fs::read(&path)?, b"preserved");
        Ok(())
    }

    #[test]
    fn immutable_records_reuse_exact_bytes_and_preserve_changes_and_abandoned_staging()
    -> io::Result<()> {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("record");
        let orphan = temporary.path().join(".record-stage-interrupted");
        write_new(&orphan, b"partial")?;
        write_immutable(&path, b"complete", create_new)?;
        write_immutable(&path, b"complete", create_new)?;
        assert!(write_immutable(&path, b"different", create_new).is_err());
        assert_eq!(std::fs::read(&path)?, b"complete");
        assert_eq!(std::fs::read(orphan)?, b"partial");
        Ok(())
    }
    #[test]
    fn package_names_reject_windows_aliases_and_traversal() {
        for name in
            ["../x", "a/../x", "a//b", "a\\b", "a:stream", "NUL.txt", "x.", "x ", "/x", "a/?"]
        {
            assert!(relative(name).is_err(), "accepted {name}");
        }
        assert!(relative("sugarcubes/web/糖.js").is_ok());
    }
    #[test]
    fn custody_prevents_replacement_and_source_mutation() -> io::Result<()> {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("owned");
        std::fs::create_dir(&path)?;
        std::fs::write(path.join("input"), b"preserve")?;
        let directory = Directory::open(&path, false)?;
        let input = directory.read("input")?;
        assert!(std::fs::rename(&path, temporary.path().join("moved")).is_err());
        assert!(std::fs::write(path.join("input"), b"changed").is_err());
        drop(input);
        drop(directory);
        assert_eq!(std::fs::read(path.join("input"))?, b"preserve");
        Ok(())
    }
    #[test]
    fn create_only_publication_preserves_existing_bytes() -> io::Result<()> {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("record");
        write_new(&path, b"original")?;
        assert!(write_new(&path, b"replacement").is_err());
        assert_eq!(std::fs::read(path)?, b"original");
        Ok(())
    }
    #[test]
    fn directory_creation_traverses_the_existing_drive_root() -> io::Result<()> {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("new").join("nested");
        let directory = Directory::open(&path, true)?;
        assert!(directory.path.is_dir());
        let second = Directory::open(&path, true)?;
        assert_eq!(directory.path, second.path);
        Ok(())
    }
}
