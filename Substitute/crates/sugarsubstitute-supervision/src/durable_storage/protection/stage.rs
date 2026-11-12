// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire exact staged objects and make journaled temporary handles durable.
use super::*;
use std::{fs::OpenOptions, os::windows::fs::OpenOptionsExt};

impl Protection {
    /// Cancel the original handle's on-close deletion only after its ownership journal is flushed.
    /// The caller retains the same writable handle and its pinned parent through this operation.
    pub(crate) fn retain_transient(&self, file: &File, context: &JobContext) -> io::Result<()> {
        check(context)?;
        self.verify_file(file)?;
        let disposition = FILE_DISPOSITION_INFO_EX { Flags: FILE_DISPOSITION_FLAG_ON_CLOSE };
        // SAFETY: The original DELETE-capable handle and initialized fixed-size structure
        // remain live. ON_CLOSE without DELETE clears this handle's creation-time flag.
        unsafe {
            SetFileInformationByHandle(
                HANDLE(file.as_raw_handle()),
                FileDispositionInfoEx,
                std::ptr::from_ref(&disposition).cast(),
                size_of::<FILE_DISPOSITION_INFO_EX>() as u32,
            )
        }
        .map_err(io::Error::other)?;
        files::sync(file, context)
    }

    /// Remove only an exact journaled regular file through exclusive native object custody.
    /// A missing name is already settled. Changed identities, links and permissions are preserved.
    pub(crate) fn remove_journaled(
        &mut self,
        path: &Path,
        expected: (u32, u32, u32),
        context: &JobContext,
    ) -> io::Result<()> {
        check(context)?;
        let _parent = Directory::open(
            path.parent().ok_or_else(|| super::super::invalid("Stage parent missing"))?,
            false,
        )?;
        let file = match OpenOptions::new()
            .access_mode((DELETE | FILE_READ_ATTRIBUTES | READ_CONTROL | SYNCHRONIZE).0)
            .share_mode(0)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
            .open(path)
        {
            Ok(file) => file,
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(cause) => return Err(cause),
        };
        let result = (|| {
            let mut information = BY_HANDLE_FILE_INFORMATION::default();
            // SAFETY: The retained file and initialized output outlive this synchronous query.
            unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut information) }
                .map_err(io::Error::other)?;
            if information.dwFileAttributes
                & (FILE_ATTRIBUTE_DIRECTORY.0 | FILE_ATTRIBUTE_REPARSE_POINT.0)
                != 0
                || information.nNumberOfLinks != 1
                || (
                    information.dwVolumeSerialNumber,
                    information.nFileIndexHigh,
                    information.nFileIndexLow,
                ) != expected
            {
                return Err(super::super::invalid(
                    "Journaled stage identity changed; file preserved",
                ));
            }
            self.verify_file(&file)?;
            check(context)?;
            let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
            // SAFETY: This exclusively retained, verified DELETE-capable regular file is the
            // journaled object. Closing its handle completes removal without reopening its name.
            unsafe {
                SetFileInformationByHandle(
                    HANDLE(file.as_raw_handle()),
                    FileDispositionInfo,
                    std::ptr::from_ref(&disposition).cast(),
                    size_of::<FILE_DISPOSITION_INFO>() as u32,
                )
            }
            .map_err(io::Error::other)
        })();
        let closed = self.close_file(file);
        match (result, closed) {
            (Ok(()), closed) => closed,
            (Err(cause), Ok(())) => Err(cause),
            (Err(cause), Err(close)) => Err(io::Error::other(DomainFailure::Native {
                cause: close,
                operation: Some(cause),
            })),
        }
    }
}
