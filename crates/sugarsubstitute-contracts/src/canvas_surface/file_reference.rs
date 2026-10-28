// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode bounded shared output references without authorizing paths or connections.

use super::wire;
use std::{error::Error, fmt};

/// Maximum serialized file-reference size, in UTF-8 bytes, before JSON decoding.
pub const MAX_FILE_REFERENCE_BYTES: usize = 32768;

/// The Comfy storage category declared by a result reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanvasFileStorage {
    /// Comfy's configured input directory.
    Input,
    /// Comfy's configured output directory.
    Output,
    /// Comfy's configured temporary directory.
    Temporary,
}

/// Schema-valid data only; the Comfy adapter must authorize its path and connection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanvasFileReference {
    filename: String,
    subfolder: String,
    storage: CanvasFileStorage,
}

impl CanvasFileReference {
    /// Adopt a schema-valid file reference without granting filesystem or network authority.
    pub(super) fn from_wire(value: wire::CanvasSurfaceFileReference) -> Option<Self> {
        if !value.valid() {
            return None;
        }
        let storage = match value.r#type.as_str() {
            "input" => CanvasFileStorage::Input,
            "output" => CanvasFileStorage::Output,
            "temp" => CanvasFileStorage::Temporary,
            _ => return None,
        };
        Some(Self { filename: value.filename, subfolder: value.subfolder, storage })
    }
    /// Decode the closed shared wire shape and retain its original Unicode text.
    ///
    /// This is not a filesystem path, URL, network permission, or evidence that a result exists.
    /// # Errors
    /// Rejects oversized, malformed, extended, or schema-invalid data.
    pub fn decode(bytes: &[u8]) -> Result<Self, CanvasFileError> {
        if bytes.len() > MAX_FILE_REFERENCE_BYTES {
            return Err(CanvasFileError::TooLarge);
        }
        let value: wire::CanvasSurfaceFileReference =
            serde_json::from_slice(bytes).map_err(CanvasFileError::Malformed)?;
        Self::from_wire(value).ok_or(CanvasFileError::OutOfBounds)
    }

    /// Borrow the original filename; its contents are not path-authorized.
    pub fn filename(&self) -> &str {
        &self.filename
    }

    /// Borrow the original subfolder; its contents are not path-authorized.
    pub fn subfolder(&self) -> &str {
        &self.subfolder
    }

    /// Return the explicitly declared Comfy storage category.
    pub const fn storage(&self) -> CanvasFileStorage {
        self.storage
    }
}

/// Preserve malformed JSON causes while distinguishing schema and byte-bound failures.
#[derive(Debug)]
pub enum CanvasFileError {
    /// The encoded value exceeds the byte admission bound.
    TooLarge,
    /// The value is not a valid closed JSON shape.
    Malformed(serde_json::Error),
    /// A recognized field violates its schema-defined bounds.
    OutOfBounds,
}

impl fmt::Display for CanvasFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => formatter.write_str("Canvas file reference exceeds its byte bound"),
            Self::Malformed(_) => formatter.write_str("Canvas file reference is malformed"),
            Self::OutOfBounds => formatter.write_str("Canvas file reference violates its bounds"),
        }
    }
}

impl Error for CanvasFileError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Malformed(cause) => Some(cause),
            Self::TooLarge | Self::OutOfBounds => None,
        }
    }
}

#[cfg(test)]
mod tests;
