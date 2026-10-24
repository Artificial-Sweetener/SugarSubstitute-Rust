// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain protected resumable bytes without exposing writable files or deciding content authority.
use super::{
    check,
    files::{self, Directory},
    invalid, protection,
};
use std::{
    error::Error,
    fmt,
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    os::windows::fs::{MetadataExt, OpenOptionsExt},
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim, MemoryLease};
const CHUNK: usize = 256 * 1024;
const CUSTODY_BYTES: u64 = 128 * 1024;

/// Exact file and independent parent custody, retained before reading or mutating payload bytes.
/// Call filesystem operations on admitted blocking workers. The feature supplies a byte extent
/// already admitted by its storage grant and retains schemas, authentication and invalidation.
/// Read/seek access uses the same native file through subsequent feature verification.
pub(crate) struct Payload {
    file: File,
    _parent: Directory,
    maximum: u64,
    _memory: MemoryClaim,
}

/// A transfer failed without granting authenticity or interpreting feature response framing.
#[derive(Debug)]
pub(crate) enum TransferFailure {
    /// The requested interval exceeds admission or does not resume at the current physical end.
    Extent,
    /// The source shortened, returned excess bytes or reported an impossible read count.
    Length,
    /// Native I/O, allocation or operation control failed.
    Storage(io::Error),
    /// Both the original transfer and mandatory partial-file flush failed.
    Flush {
        /// Original transfer failure, retained independently of the persistence failure.
        transfer: Box<TransferFailure>,
        /// Actual flush cause; durability must not be inferred.
        flush: io::Error,
    },
}
impl fmt::Display for TransferFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Extent => "Payload interval exceeds its admitted extent",
            Self::Length => "Payload source length disagrees with its interval",
            Self::Storage(_) => "Payload storage operation failed",
            Self::Flush { .. } => "Payload transfer and partial-file flush failed",
        })
    }
}
impl Error for TransferFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Storage(cause) => Some(cause),
            Self::Flush { transfer, .. } => Some(transfer.as_ref()),
            Self::Extent | Self::Length => None,
        }
    }
}
impl From<io::Error> for TransferFailure {
    fn from(cause: io::Error) -> Self {
        Self::Storage(cause)
    }
}

impl Payload {
    /// Retain an existing protected, singly-linked file, or create one absent flat child.
    /// Existing bytes remain intact even when larger than `maximum`; only the feature may
    /// decide to reset them. Links, directories, changed parents and failed controls reject.
    /// The storage grant supplies its original allocation lease separately from current job control.
    /// This capability keeps that original 128 KiB custody allocation across job retirement.
    pub(crate) fn open(
        parent: &Directory,
        name: &str,
        maximum: u64,
        memory: &MemoryLease,
        context: &JobContext,
    ) -> io::Result<Self> {
        check(context)?;
        files::flat_leaf(name)?;
        let memory = memory.claim(CUSTODY_BYTES).map_err(io::Error::other)?;
        let parent = parent.retained()?;
        let path = parent.path().join(name);
        let file = protection::with(context, |protection| {
            let file = match OpenOptions::new()
                .read(true)
                .write(true)
                .share_mode(0)
                .custom_flags(0x0020_0000)
                .open(&path)
            {
                Ok(file) => file,
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
                    protection.file(&path, context)?
                }
                Err(cause) => return Err(cause),
            };
            let metadata = file.metadata()?;
            if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0 {
                return Err(invalid("Payload must be a regular file"));
            }
            files::single_link(&file)?;
            protection.verify_file(&file)?;
            check(context)?;
            Ok(file)
        })?;
        Ok(Self { file, _parent: parent, maximum, _memory: memory })
    }
    /// Observe the actual retained file length; no pathname is reopened.
    pub(crate) fn length(&self) -> io::Result<u64> {
        Ok(self.file.metadata()?.len())
    }
    /// Discard only this retained payload after the feature explicitly invalidates its bytes.
    /// A failed reset/flush remains observable and grants no successful recovery outcome.
    pub(crate) fn reset(&mut self, context: &JobContext) -> io::Result<()> {
        check(context)?;
        self.file.set_len(0)?;
        self.file.sync_all()?;
        check(context)
    }
    /// Flush the retained prefix even during cancellation cleanup. This is a durability effect,
    /// not permission to append bytes, and keeps the same file and original custody claim alive.
    pub(crate) fn flush(&self) -> io::Result<()> {
        self.file.sync_all()
    }
    /// Copy an exact admitted interval, probing one extra source byte before success.
    /// Offset zero restarts this payload. Other offsets must match its actual physical end.
    /// The caller supplies bounded source I/O; this method creates no tasks or transport.
    /// Control checks surround reads/writes, and every outcome attempts to flush the actual prefix.
    pub(crate) fn receive_exact(
        &mut self,
        offset: u64,
        length: u64,
        source: &mut impl Read,
        context: &JobContext,
    ) -> Result<(), TransferFailure> {
        let outcome = self.copy_exact(offset, length, source, context);
        match (outcome, self.flush()) {
            (result, Ok(())) => result,
            (Ok(()), Err(flush)) => Err(flush.into()),
            (Err(transfer), Err(flush)) => {
                Err(TransferFailure::Flush { transfer: Box::new(transfer), flush })
            }
        }
    }
    /// Reserve the actual fixed buffer before truncation, then write only permitted source bytes.
    fn copy_exact(
        &mut self,
        offset: u64,
        length: u64,
        source: &mut impl Read,
        context: &JobContext,
    ) -> Result<(), TransferFailure> {
        check(context)?;
        if offset.checked_add(length).is_none_or(|end| end > self.maximum)
            || offset != 0 && self.length()? != offset
        {
            return Err(TransferFailure::Extent);
        }
        let _scratch = context.memory().claim(CHUNK as u64).map_err(io::Error::other)?;
        let mut scratch = Vec::new();
        scratch.try_reserve_exact(CHUNK).map_err(io::Error::other)?;
        if scratch.capacity() > CHUNK {
            return Err(invalid("Payload buffer exceeds its admission").into());
        }
        scratch.resize(CHUNK, 0);
        if offset == 0 {
            self.file.set_len(0)?;
        }
        self.file.seek(SeekFrom::Start(offset))?;
        let mut copied = 0;
        loop {
            check(context)?;
            let maximum = (length - copied).saturating_add(1).min(CHUNK as u64) as usize;
            let count = source.read(&mut scratch[..maximum])?;
            check(context)?;
            if count == 0 {
                break;
            }
            if count > maximum || count as u64 > length - copied {
                return Err(TransferFailure::Length);
            }
            self.file.write_all(&scratch[..count])?;
            copied += count as u64;
            check(context)?;
        }
        if copied != length {
            return Err(TransferFailure::Length);
        }
        Ok(())
    }
}
impl Read for Payload {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.file.read(output)
    }
}
impl Seek for Payload {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.file.seek(position)
    }
}

#[cfg(test)]
mod tests;
