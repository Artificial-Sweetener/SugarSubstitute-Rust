// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Authenticate inherited exclusion against the pinned rendezvous without reopening its data.
use super::*;
use windows::Win32::Foundation::ERROR_SHARING_VIOLATION;

impl Protection {
    /// Verify actual file identity, protection, empty contents and all three sharing exclusions.
    /// The caller pins the canonical parent and retains the supplied inherited file throughout.
    /// Metadata-only access is permitted by Windows even when data/delete sharing is denied.
    pub(crate) fn verify_exclusive_rendezvous(
        &self,
        file: &File,
        path: &Path,
        context: &JobContext,
    ) -> io::Result<()> {
        check(context)?;
        self.verify_file(file)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0 || metadata.len() != 0 {
            return Err(super::super::invalid("Inherited run rendezvous is not an empty file"));
        }
        let named = open_existing(path, 0).map_err(io::Error::other)?;
        if crate::durable_storage::files::file_identity(file)?
            != crate::durable_storage::files::file_identity(&named)?
        {
            return Err(super::super::invalid("Inherited run belongs to another native object"));
        }
        for access in [FILE_READ_DATA.0, FILE_WRITE_DATA.0, DELETE.0] {
            match open_existing(path, access) {
                Err(cause)
                    if cause.code()
                        == windows::core::HRESULT::from_win32(ERROR_SHARING_VIOLATION.0) => {}
                Err(cause) => return Err(io::Error::other(cause)),
                Ok(_) => {
                    return Err(super::super::invalid(
                        "Inherited run does not retain exclusive sharing",
                    ));
                }
            }
        }
        check(context)
    }
}

/// Probe only access rights; OPEN_EXISTING never creates, truncates, writes or deletes anything.
fn open_existing(path: &Path, access: u32) -> windows::core::Result<File> {
    // SAFETY: the caller pins the parent. The path lives through this synchronous open, and
    // successful unique ownership is adopted immediately. No handle is made inheritable.
    let handle = unsafe {
        CreateFileW(
            &files::native_path(path),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_OPEN_REPARSE_POINT,
            None,
        )
    }?;
    // SAFETY: this is the sole owner of the successful CreateFileW result.
    Ok(unsafe { File::from_raw_handle(handle.0) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup_storage::test_worker::{self, TestResult};
    use std::io::Write;

    #[test]
    fn inherited_exclusion_rejects_each_shared_access_even_with_a_byte_range_lock() -> TestResult {
        test_worker::run(|context, _| {
            let temporary = tempfile::tempdir()?;
            let parent = Directory::open(temporary.path(), false)?;
            let path = parent.path().join("run");
            with(context, |protection| {
                for sharing in [
                    FILE_SHARE_READ,
                    FILE_SHARE_WRITE,
                    FILE_SHARE_DELETE,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                ] {
                    let file = protection.open_file(&path, OPEN_ALWAYS, sharing, context)?;
                    file.try_lock().map_err(io::Error::from)?;
                    assert!(protection.verify_exclusive_rendezvous(&file, &path, context).is_err());
                }
                let exclusive = protection.exclusive_rendezvous(&path, context)?;
                protection.verify_exclusive_rendezvous(&exclusive, &path, context)?;
                Ok(())
            })?;
            Ok(())
        })
    }

    #[test]
    fn inherited_exclusion_requires_the_same_native_empty_protected_object() -> TestResult {
        test_worker::run(|context, _| {
            let temporary = tempfile::tempdir()?;
            let parent = Directory::open(temporary.path(), false)?;
            let path = parent.path().join("run");
            with(context, |protection| {
                let original = protection.exclusive_rendezvous(&path, context)?;
                let duplicate = original.try_clone()?;
                drop(original);
                protection.verify_exclusive_rendezvous(&duplicate, &path, context)?;
                let foreign_path = parent.path().join("foreign");
                let foreign = protection.exclusive_rendezvous(&foreign_path, context)?;
                assert!(protection.verify_exclusive_rendezvous(&foreign, &path, context).is_err());
                drop(duplicate);
                let mut nonempty = protection.file(&parent.path().join("nonempty"), context)?;
                nonempty.write_all(b"unknown")?;
                assert!(
                    protection
                        .verify_exclusive_rendezvous(
                            &nonempty,
                            &parent.path().join("nonempty"),
                            context
                        )
                        .is_err()
                );
                drop(nonempty);
                assert_eq!(std::fs::read(parent.path().join("nonempty"))?, b"unknown");
                Ok(())
            })?;
            Ok(())
        })
    }
}
