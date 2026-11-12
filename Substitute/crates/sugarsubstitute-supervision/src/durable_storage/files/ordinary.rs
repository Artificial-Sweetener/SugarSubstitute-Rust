// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Select finite native sharing policies beneath retained parents without exporting open options.
use super::*;
use crate::durable_storage::{
    check,
    control::{InterruptedEffect, effect},
    record_read,
};
use sugarsubstitute_contracts::execution::JobContext;
use windows::Win32::Storage::FileSystem::{FILE_GENERIC_READ, FILE_WRITE_ATTRIBUTES};

/// Existing regular-file sharing and access policies with distinct native ownership contracts.
#[derive(Clone, Copy)]
pub(crate) enum ExistingAccess {
    /// Read immutable record bytes with read/delete sharing; retire before publication.
    ReplaceableRead,
    /// Read a shared lease and update its retention metadata without granting data writes.
    SharedMetadata,
    /// Inspect a closed legacy writer without acquiring data-write access or creating a lock.
    ExclusiveRead,
    /// Retain exclusive read/write custody for a feature-authorized reusable record stage.
    Exclusive,
}

/// Own actual OS file-lock exclusion until this guard drops; bytes remain private to storage.
pub(crate) struct FileExclusion {
    _file: File,
}

impl Directory {
    /// Open one existing regular child under the selected fixed sharing policy.
    /// Native ancestry stays pinned by the caller. Reparse points, ambiguous names and original
    /// ended job controls are rejected. Features retain schema and protection verification.
    pub(crate) fn open_existing(
        &self,
        leaf: &str,
        access: ExistingAccess,
        context: &JobContext,
    ) -> io::Result<File> {
        flat_leaf(leaf)?;
        let mut options = OpenOptions::new();
        match access {
            ExistingAccess::ReplaceableRead => {
                options.read(true).share_mode(5);
            }
            ExistingAccess::SharedMetadata => {
                options.access_mode((FILE_GENERIC_READ | FILE_WRITE_ATTRIBUTES).0).share_mode(1);
            }
            ExistingAccess::Exclusive => {
                options.read(true).write(true).share_mode(0);
            }
            ExistingAccess::ExclusiveRead => {
                options.read(true).share_mode(0);
            }
        }
        options.custom_flags(OPEN_REPARSE_POINT);
        let file = effect(context, || options.open(self.path().join(leaf)))?;
        regular(&file, context)?;
        Ok(file)
    }

    /// Acquire the original create-if-absent, nontruncating OS-lock contract for a child.
    /// Existing bytes and permissions remain intact. This creates no installation authority;
    /// the caller first verifies its feature-owned installation marker and retains this parent.
    pub(crate) fn lock(&self, leaf: &str, context: &JobContext) -> io::Result<FileExclusion> {
        flat_leaf(leaf)?;
        let file = effect(context, || {
            OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .share_mode(3)
                .custom_flags(OPEN_REPARSE_POINT)
                .open(self.path().join(leaf))
        })?;
        regular(&file, context)?;
        effect(context, || file.try_lock().map_err(io::Error::from))?;
        Ok(FileExclusion { _file: file })
    }
}

/// Validate the actual retained handle, never a second path observation.
fn regular(file: &File, context: &JobContext) -> io::Result<()> {
    let metadata = effect(context, || file.metadata())?;
    if !metadata.is_file() || metadata.file_attributes() & REPARSE_POINT != 0 {
        return Err(invalid("Storage child must be a regular file"));
    }
    Ok(())
}

/// Read one already retained record within the admitted job, observing controls at each read.
/// Features own the schema limit and its workspace reservation; no raw handle escapes.
pub(crate) fn read_admitted(
    file: &mut File,
    maximum: usize,
    context: &JobContext,
) -> io::Result<Vec<u8>> {
    use record_read::{Allocation, InterruptedReads, ReadFailure};
    if u64::try_from(maximum).ok().is_none_or(|bytes| bytes > context.memory().bytes()) {
        return Err(io::Error::new(
            io::ErrorKind::OutOfMemory,
            "Record bound exceeds admitted workspace",
        ));
    }
    let length = effect(context, || file.metadata())?.len();
    if length > maximum as u64 {
        return Err(invalid("Record exceeds its byte bound"));
    }
    let bytes = record_read::bounded(
        file,
        maximum,
        Allocation::Incremental { observed: length as usize },
        InterruptedReads::Resume,
        || check(context),
        || check(context),
    )
    .map_err(|failure| match failure {
        ReadFailure::Control(cause) | ReadFailure::Read { source: cause, after: None } => cause,
        ReadFailure::Read { source, after: Some(control) } => {
            io::Error::new(control.kind(), InterruptedEffect { source, control })
        }
        ReadFailure::Capacity(cause) => io::Error::new(io::ErrorKind::OutOfMemory, cause),
        ReadFailure::TooLarge => invalid("Record exceeded its byte bound"),
    })?;
    if bytes.len() as u64 != length {
        return Err(if (bytes.len() as u64) < length {
            io::Error::new(io::ErrorKind::UnexpectedEof, "Record shortened during read")
        } else {
            invalid("Record grew during read")
        });
    }
    Ok(bytes)
}

/// Observe native modification time on the current admitted storage worker.
pub(crate) fn modified(file: &File, context: &JobContext) -> io::Result<std::time::SystemTime> {
    effect(context, || file.metadata()?.modified())
}

/// Observe the retained file's current length in bytes under original current job controls.
pub(crate) fn length(file: &File, context: &JobContext) -> io::Result<u64> {
    effect(context, || Ok(file.metadata()?.len()))
}

/// Flush the retained file's contents and metadata under the original job controls.
/// Native completion is observed before returning, including when controls end during the flush.
pub(crate) fn sync(file: &File, context: &JobContext) -> io::Result<()> {
    effect(context, || file.sync_all())
}

/// Update only metadata through the retained handle and original current job controls.
pub(crate) fn set_modified(
    file: &File,
    time: std::time::SystemTime,
    context: &JobContext,
) -> io::Result<()> {
    effect(context, || file.set_times(std::fs::FileTimes::new().set_modified(time)))
}

#[cfg(test)]
mod tests;
