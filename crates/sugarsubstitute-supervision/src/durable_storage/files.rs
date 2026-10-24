// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pin each resolved directory against replacement and reject reparse-point traversal.
mod canonical;
mod child;
#[allow(unsafe_code)]
mod identity;
#[cfg(test)]
mod lexical_tests;
use super::invalid;
pub(crate) use identity::{file_identity, single_link};
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    os::windows::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Component, Path, PathBuf, Prefix},
};
use windows::core::HSTRING;
const REPARSE_POINT: u32 = 0x400;
const OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const BACKUP_SEMANTICS: u32 = 0x0200_0000;

/// Encode rooted DOS drive paths with extended Win32 syntax, preserving original UTF-16 units.
/// Callers retain and validate the original parent/name under their own storage policy.
/// Existing native prefixes retain their original representation.
pub(crate) fn native_path(path: &Path) -> HSTRING {
    HSTRING::from_wide(&native_units(path).collect::<Vec<_>>())
}
/// Lend the same encoding to admitted native buffers without allocating an intermediate name.
pub(crate) fn native_units(path: &Path) -> impl Iterator<Item = u16> + '_ {
    let dos = path.has_root()
        && matches!(path.components().next(), Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_)));
    [92_u16, 92, 63, 92]
        .into_iter()
        .take(if dos { 4 } else { 0 })
        .chain(dos.then_some(path.components()).into_iter().flatten().enumerate().flat_map(
            |(index, part)| {
                (index > 2).then_some(92).into_iter().chain(part.as_os_str().encode_wide())
            },
        ))
        .chain((!dos).then_some(path.as_os_str()).into_iter().flat_map(OsStrExt::encode_wide))
}

/// Keep every ancestor pinned while exposing only an immutable observed location.
pub(crate) struct Directory {
    path: PathBuf,
    _pins: Vec<File>,
}

impl Directory {
    /// Borrow the observed location without allowing callers to detach it from its native pins.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Retain this exact native parent independently of its current borrow or originating job.
    pub(crate) fn retained(&self) -> io::Result<Self> {
        let retained = Self::open(self.path(), false)?;
        if retained.identity()? != self.identity()? {
            return Err(invalid("Retained directory identity changed"));
        }
        Ok(retained)
    }

    /// Pin and validate each component before traversing its children. Existing directories
    /// need only read access; creation is attempted after an actual missing-component result.
    pub(crate) fn open(path: &Path, create: bool) -> io::Result<Self> {
        Self::open_with_creator(path, create, &mut |path| std::fs::create_dir(path))
    }

    /// Keep the external creation effect observable independently of diagnostic subscribers.
    fn open_with_creator(
        path: &Path,
        create: bool,
        creator: &mut dyn FnMut(&Path) -> io::Result<()>,
    ) -> io::Result<Self> {
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
                creator,
            )?);
        }
        Ok(Self { path: path.to_owned(), _pins: pins })
    }

    /// Retain a regular read-only child. Nested inputs require separately pinned child directories.
    pub(crate) fn read(&self, leaf: &str) -> io::Result<File> {
        let _operation =
            tracing::trace_span!(target: "sugarsubstitute::storage", "file_open_read").entered();
        flat_leaf(leaf)?;
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
fn open_component(
    path: &Path,
    create: bool,
    creator: &mut dyn FnMut(&Path) -> io::Result<()>,
) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).share_mode(3).custom_flags(OPEN_REPARSE_POINT | BACKUP_SEMANTICS);
    let pin = match options.open(path) {
        Ok(pin) => pin,
        Err(cause) if create && cause.kind() == io::ErrorKind::NotFound => {
            match tracing::trace_span!(target: "sugarsubstitute::storage", "directory_create")
                .in_scope(|| creator(path))
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

/// Reject nonlocal, unbounded or ambiguous ancestry before native traversal.
/// Native account discovery supplies verbatim local drive paths; retain their original spelling.
pub(crate) fn lexical(path: &Path) -> io::Result<()> {
    if !path.is_absolute()
        || path.as_os_str().len() > 4096
        || path.parent().is_none()
        || path.components().any(|part| matches!(part, Component::ParentDir | Component::CurDir))
        || path.to_string_lossy().split(['\\', '/']).any(|part| part == "." || part == "..")
        || !matches!(path.components().next(),Some(Component::Prefix(prefix)) if matches!(prefix.kind(),Prefix::Disk(_)|Prefix::VerbatimDisk(_)))
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

/// Apply the supported Windows-safe package component rules to relative names.
pub(crate) fn relative(value: &str) -> io::Result<()> {
    sugarsubstitute_update::package::relative_path(value).map_err(io::Error::other)
}

/// Validate one Windows-safe child without granting nested traversal or alternate streams.
pub(crate) fn flat_leaf(value: &str) -> io::Result<()> {
    relative(value)?;
    if value.contains('/') {
        return Err(invalid("Storage access requires one child of its retained parent"));
    }
    Ok(())
}

/// Bound the retained record read, reserving one sentinel byte to detect concurrent growth.
pub(crate) fn read_bounded(file: &mut File, maximum: usize) -> io::Result<Vec<u8>> {
    let read_limit = maximum
        .checked_add(1)
        .and_then(|bound| u64::try_from(bound).ok())
        .ok_or_else(|| invalid("Record bound must leave room for overflow detection"))?;
    let observed = file.metadata()?.len();
    if observed >= read_limit {
        return Err(invalid("Installation record is too large"));
    }
    use super::record_read::{self, Allocation, InterruptedReads, ReadFailure};
    record_read::bounded(
        file,
        maximum,
        Allocation::Incremental { observed: observed as usize },
        InterruptedReads::Resume,
        || Ok::<_, std::convert::Infallible>(()),
        || Ok(()),
    )
    .map_err(|failure| match failure {
        ReadFailure::Read { source, after: None } => source,
        ReadFailure::Control(never) | ReadFailure::Read { after: Some(never), .. } => {
            match never {}
        }
        ReadFailure::Capacity(cause) => io::Error::new(io::ErrorKind::OutOfMemory, cause),
        ReadFailure::TooLarge => invalid("Installation record exceeded its bound"),
    })
}

/// Flush complete bytes into an absent name while the caller retains its parent ancestry.
pub(crate) fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let _operation =
        tracing::trace_span!(target: "sugarsubstitute::storage", "record_write_flush").entered();
    let mut file = create_new(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Complete a known immutable generation record, accepting only byte-identical earlier output.
/// Flush a unique sibling before publication so interruption never exposes a partial final file.
pub(crate) fn write_immutable(
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
    let staged = crate::durable_storage::staging::unique_path(&parent, ".record-stage")?;
    let mut file = create(&staged)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    crate::durable_storage::staging::publish(&staged, path)
}

/// Create an ordinary output only when its name is absent.
pub(crate) fn create_new(path: &Path) -> io::Result<File> {
    let _operation =
        tracing::trace_span!(target: "sugarsubstitute::storage", "file_create").entered();
    OpenOptions::new().write(true).create_new(true).share_mode(0).open(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_encoding_normalizes_dos_separators_and_preserves_native_utf16_names() {
        use std::ffi::OsString;
        use std::os::windows::ffi::{OsStrExt, OsStringExt};
        assert_eq!(
            native_path(Path::new("C:/owned/record")).to_string_lossy(),
            r"\\?\C:\owned\record"
        );
        let native = Path::new(r"\\?\C:\owned\record");
        assert_eq!(&*native_path(native), native.as_os_str().encode_wide().collect::<Vec<_>>());
        let mut units: Vec<u16> = r"C:\owned\".encode_utf16().collect();
        units.extend([0xd800, b'x' as u16, 0xdc00]);
        let path = PathBuf::from(OsString::from_wide(&units));
        let mut expected: Vec<u16> = r"\\?\".encode_utf16().collect();
        expected.extend(units);
        assert_eq!(&*native_path(&path), expected);
    }
    #[test]
    fn literal_dot_components_are_rejected_before_path_normalization() {
        for path in [r"C:\owned\.\nested", "C:/owned/./nested", r"C:\owned\..\other"] {
            assert!(lexical(Path::new(path)).is_err(), "accepted {path}");
        }
        assert!(lexical(Path::new(r"C:\owned\nested")).is_ok());
    }
    #[test]
    fn nested_reads_require_pinned_descendants() -> io::Result<()> {
        let temporary = tempfile::tempdir()?;
        std::fs::create_dir(temporary.path().join("nested"))?;
        std::fs::write(temporary.path().join("nested/input"), b"preserved")?;
        let parent = Directory::open(temporary.path(), false)?;
        assert!(parent.read("nested/input").is_err());
        let child = parent.child("nested")?;
        assert_eq!(read_bounded(&mut child.read("input")?, 9)?, b"preserved");
        Ok(())
    }
    #[test]
    fn record_reads_enforce_zero_exact_and_overflow_bounds() -> io::Result<()> {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("record");
        write_new(&path, b"record")?;
        let parent = Directory::open(temporary.path(), false)?;
        assert_eq!(read_bounded(&mut parent.read("record")?, 6)?, b"record");
        assert!(read_bounded(&mut parent.read("record")?, 5).is_err());
        assert!(read_bounded(&mut parent.read("record")?, 0).is_err());
        assert!(read_bounded(&mut parent.read("record")?, usize::MAX).is_err());
        write_new(&temporary.path().join("empty"), b"")?;
        assert!(read_bounded(&mut parent.read("empty")?, 0)?.is_empty());
        Ok(())
    }
    #[test]
    fn directory_creation_opens_existing_ancestors_without_creation_attempts() -> io::Result<()> {
        let temporary = tempfile::tempdir()?;
        let attempts = std::cell::Cell::new(0);
        let mut creator = |path: &Path| {
            attempts.set(attempts.get() + 1);
            std::fs::create_dir(path)
        };
        let existing = Directory::open_with_creator(temporary.path(), true, &mut creator)?;
        assert_eq!(attempts.get(), 0);
        let path = existing.path.join("new").join("nested");
        let created = Directory::open_with_creator(&path, true, &mut creator)?;
        assert!(created.path.is_dir());
        assert_eq!(attempts.get(), 2);
        let reopened = Directory::open_with_creator(&path, true, &mut creator)?;
        assert_eq!(reopened.path, created.path);
        assert_eq!(attempts.get(), 2);
        assert!(std::fs::rename(&path, existing.path.join("moved")).is_err());
        Ok(())
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
