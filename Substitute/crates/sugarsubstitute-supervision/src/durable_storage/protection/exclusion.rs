// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Authenticate inherited exclusion against the pinned rendezvous without reopening its data.
use super::*;

impl Protection {
    /// Verify a transferred witness: actual identity, protection and empty contents of the
    /// canonical rendezvous, and that some live owner holds an exclusion slot on it right now.
    /// The caller pins the canonical parent and retains the supplied witness throughout.
    /// # Errors
    /// Rejects foreign, unprotected, nonempty or link objects and a released exclusion.
    pub(crate) fn verify_exclusive_rendezvous(
        &self,
        file: &File,
        path: &Path,
        context: &JobContext,
    ) -> io::Result<()> {
        self.verify_rendezvous_identity(file, path, context)?;
        let probe = open_existing(path, FILE_GENERIC_READ.0).map_err(io::Error::other)?;
        if !super::slots::slots_held(&probe)? {
            return Err(super::super::invalid("Inherited exclusion is no longer held"));
        }
        check(context)
    }

    /// The witness is this protection's empty regular file at the canonical rendezvous path.
    fn verify_rendezvous_identity(
        &self,
        file: &File,
        path: &Path,
        context: &JobContext,
    ) -> io::Result<()> {
        check(context)?;
        self.verify_file(file)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0 || metadata.len() != 0 {
            return Err(super::super::invalid("Inherited rendezvous is not an empty file"));
        }
        let named = open_existing(path, 0).map_err(io::Error::other)?;
        if crate::durable_storage::files::file_identity(file)?
            != crate::durable_storage::files::file_identity(&named)?
        {
            return Err(super::super::invalid(
                "Inherited rendezvous belongs to another native object",
            ));
        }
        Ok(())
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
    fn a_witness_proves_exclusion_only_for_its_own_empty_protected_rendezvous() -> TestResult {
        test_worker::run(|context, _| {
            let temporary = tempfile::tempdir()?;
            let parent = Directory::open(temporary.path(), false)?;
            let path = parent.path().join("run");
            with(context, |protection| {
                let held = protection.exclusion(&path, context)?;
                let duplicate = held.witness().try_clone()?;
                protection.verify_exclusive_rendezvous(&duplicate, &path, context)?;
                // A shared handle on the same file proves nothing once no slot is held.
                let unheld = protection.rendezvous(&parent.path().join("unheld"), context)?;
                assert!(
                    protection
                        .verify_exclusive_rendezvous(
                            &unheld,
                            &parent.path().join("unheld"),
                            context
                        )
                        .is_err()
                );
                let foreign = protection.exclusion(&parent.path().join("foreign"), context)?;
                assert!(
                    protection
                        .verify_exclusive_rendezvous(foreign.witness(), &path, context)
                        .is_err()
                );
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
                drop(held);
                assert!(
                    protection.verify_exclusive_rendezvous(&duplicate, &path, context).is_err()
                );
                Ok(())
            })?;
            Ok(())
        })
    }
}
