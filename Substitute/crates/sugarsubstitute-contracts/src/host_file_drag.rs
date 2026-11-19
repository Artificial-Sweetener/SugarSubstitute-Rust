// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Tell the embedded SugarCubes page what a drag of files over the window carries
//! (`sugarcubes.file-drag` v1).
//!
//! A page learns that a drag carries files but not what they are called until the drop. The
//! host sees the drag first, so it sends `entered` with the files' names when such a drag comes
//! over its window and `left` when the drag goes away or ends in a drop. The page asks for
//! nothing and answers nothing. Names are file names alone: a folder or a path never leaves
//! the host.

use crate::panel::{PanelError, PanelSession};
use std::{error::Error, fmt};

mod wire;

#[cfg(test)]
mod tests;

/// Maximum encoded file drag message produced, in UTF-8 bytes.
pub const MAX_FILE_DRAG_MESSAGE_BYTES: usize = 16384;
/// Most files one message names; a drag that carries more is named by its first ones.
pub const MAX_DRAGGED_FILES: usize = 16;
/// Longest file name a message carries, in UTF-16 code units.
pub const MAX_FILE_NAME_UNITS: usize = 255;
const PROTOCOL: &str = "sugarcubes.file-drag";
const VERSION: u64 = 1;

/// One host-direction file drag message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileDragHostMessage {
    /// A drag of files came over the window.
    Entered {
        /// The files' names in the order the drag carries them, 1 to [`MAX_DRAGGED_FILES`].
        names: Vec<String>,
    },
    /// The drag went away or ended in a drop.
    Left,
}

impl FileDragHostMessage {
    /// Encode this message addressed to `session`.
    ///
    /// # Errors
    /// Rejects no names or too many, an empty or overlong name, a name that holds a path
    /// separator, generated-contract drift, encoding failure or an exceeded byte bound.
    pub fn encode(&self, session: &PanelSession) -> Result<Vec<u8>, FileDragError> {
        if let Self::Entered { names } = self
            && (names.is_empty()
                || names.len() > MAX_DRAGGED_FILES
                || names.iter().any(|name| !file_name(name)))
        {
            return Err(FileDragError::Invalid);
        }
        let value = wire::encode(self, session.as_str());
        if !value.valid() {
            return Err(FileDragError::Invalid);
        }
        let encoded = serde_json::to_vec(&value).map_err(FileDragError::Malformed)?;
        if encoded.len() > MAX_FILE_DRAG_MESSAGE_BYTES {
            return Err(FileDragError::TooLarge);
        }
        Ok(encoded)
    }
}

/// Whether `name` is one file's name: not empty, within the bound and free of path separators.
pub fn file_name(name: &str) -> bool {
    !name.is_empty()
        && name.encode_utf16().count() <= MAX_FILE_NAME_UNITS
        && !name.contains(['/', '\\', '\0'])
}

/// Identify a bounded wire failure while retaining actual JSON errors as causes.
#[derive(Debug)]
pub enum FileDragError {
    /// The message exceeded its fixed byte bound.
    TooLarge,
    /// JSON encoding failed.
    Malformed(serde_json::Error),
    /// A field violates the shared schema.
    Invalid,
    /// The addressed panel session is invalid.
    Session(PanelError),
}

impl fmt::Display for FileDragError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => formatter.write_str("File drag message exceeds its byte bound"),
            Self::Malformed(_) => formatter.write_str("File drag message is malformed"),
            Self::Invalid => formatter.write_str("File drag message violates its schema"),
            Self::Session(_) => formatter.write_str("File drag message names an invalid session"),
        }
    }
}

impl Error for FileDragError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Malformed(cause) => Some(cause),
            Self::Session(cause) => Some(cause),
            Self::TooLarge | Self::Invalid => None,
        }
    }
}
