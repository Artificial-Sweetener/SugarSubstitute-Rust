// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Name the typed outcomes of the registry install format.
//!
//! The spellings are part of the format contract shared with SugarCubes' own Python
//! implementation through the install vectors, so they never change for a refactor.

use std::{fmt, io};

/// One reason the install format rejects an archive, folder, record or transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    /// The download is not a readable zip.
    NotAZip,
    /// The archive holds no files.
    EmptyArchive,
    /// An archive member is encrypted.
    EncryptedMember,
    /// An archive member is a link, device or of a type its name contradicts.
    LinkMember,
    /// An entry name is absolute, escapes its root or is not portable.
    UnsafePath,
    /// An entry is Git metadata, a Windows device name or an ownership record.
    ReservedName,
    /// Two entries name the same path on a case-insensitive volume.
    DuplicateEntry,
    /// An entry sits inside a file entry.
    EntryConflict,
    /// A listing holds more entries than allowed.
    TooManyEntries,
    /// An archive expands beyond its bounds.
    TooLarge,
    /// The `.tracking` record is unreadable.
    TrackingUnreadable,
    /// The `.tracking` record exceeds its size bound.
    TrackingTooLarge,
    /// The folder or archive is not the expected node pack.
    PackIdentity,
    /// A transaction record is unreadable.
    StagedCorrupt,
    /// The installed folder changed since the update was planned.
    TargetChanged,
    /// An earlier update has not been settled.
    Busy,
}

impl ErrorCode {
    /// Return the shared spelling of this code.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotAZip => "not-a-zip",
            Self::EmptyArchive => "empty-archive",
            Self::EncryptedMember => "encrypted-member",
            Self::LinkMember => "link-member",
            Self::UnsafePath => "unsafe-path",
            Self::ReservedName => "reserved-name",
            Self::DuplicateEntry => "duplicate-entry",
            Self::EntryConflict => "entry-conflict",
            Self::TooManyEntries => "too-many-entries",
            Self::TooLarge => "too-large",
            Self::TrackingUnreadable => "tracking-unreadable",
            Self::TrackingTooLarge => "tracking-too-large",
            Self::PackIdentity => "pack-identity",
            Self::StagedCorrupt => "staged-corrupt",
            Self::TargetChanged => "target-changed",
            Self::Busy => "busy",
        }
    }
}

/// A rejected archive, ownership record or install transaction, or the I/O that failed it.
#[derive(Debug)]
pub enum PackInstallError {
    /// The format refused the input.
    Format {
        /// The shared outcome code.
        code: ErrorCode,
        /// A diagnostic detail that names no user path.
        detail: &'static str,
    },
    /// A filesystem operation failed; its cause is preserved.
    Io(io::Error),
}

impl PackInstallError {
    /// Build one format refusal.
    pub(crate) fn format(code: ErrorCode, detail: &'static str) -> Self {
        Self::Format { code, detail }
    }

    /// Return the shared code of a format refusal.
    pub fn code(&self) -> Option<ErrorCode> {
        match self {
            Self::Format { code, .. } => Some(*code),
            Self::Io(_) => None,
        }
    }
}

impl fmt::Display for PackInstallError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Format { code, detail } => write!(formatter, "{}: {detail}", code.as_str()),
            Self::Io(cause) => write!(formatter, "pack install I/O failed: {cause}"),
        }
    }
}

impl std::error::Error for PackInstallError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Format { .. } => None,
            Self::Io(cause) => Some(cause),
        }
    }
}

impl From<io::Error> for PackInstallError {
    fn from(cause: io::Error) -> Self {
        Self::Io(cause)
    }
}

/// Results of the install format.
pub type Result<T> = std::result::Result<T, PackInstallError>;
