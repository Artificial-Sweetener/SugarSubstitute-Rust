// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Show the operating system's file dialog for the embedded SugarCubes page, starting in the
//! folder it names, and tell it what the person chose (`sugarcubes.file-choice` v1).
//!
//! The page asks with `open` or `save`, each naming its own request. The host answers every
//! request exactly once: `chosen` with where the person chose, `dismissed`, or
//! `failed`. The host reads and writes nothing; the page uses its own storage. Decoding
//! validates shape and bounds only; the folder a request names is only where the dialog starts,
//! and whether a dialog is shown is the host's decision.

use crate::panel::{PanelError, PanelSession};
use std::{error::Error, fmt};

mod wire;

#[cfg(test)]
mod tests;

/// Maximum encoded file choice message admitted or produced, in UTF-8 bytes.
pub const MAX_FILE_CHOICE_MESSAGE_BYTES: usize = 8192;
/// Longest folder or relative path, in UTF-16 code units.
pub const MAX_FILE_CHOICE_PATH_UNITS: usize = 1024;
/// Longest file name, in UTF-16 code units.
pub const MAX_FILE_CHOICE_NAME_UNITS: usize = 255;
const PROTOCOL: &str = "sugarcubes.file-choice";
const VERSION: u64 = 1;

/// The page's own name for one request, repeated in its answer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileChoiceRequestName(String);

impl FileChoiceRequestName {
    /// Accept one to forty-eight lowercase letters, digits, dots or hyphens.
    ///
    /// # Errors
    /// Rejects any other text.
    pub fn new(value: &str) -> Result<Self, FileChoiceError> {
        if (1..=48).contains(&value.len())
            && value.bytes().all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-'))
        {
            Ok(Self(value.to_owned()))
        } else {
            Err(FileChoiceError::Invalid)
        }
    }

    /// Borrow the request's name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One file extension a dialog offers, without its dot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileChoiceExtension(String);

impl FileChoiceExtension {
    /// Accept one to sixteen lowercase letters or digits.
    ///
    /// # Errors
    /// Rejects any other text, including a dot or a wildcard.
    pub fn new(value: &str) -> Result<Self, FileChoiceError> {
        if (1..=16).contains(&value.len())
            && value.bytes().all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9'))
        {
            Ok(Self(value.to_owned()))
        } else {
            Err(FileChoiceError::Invalid)
        }
    }

    /// Borrow the extension.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Why a request ended without a choice or a dismissal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileChoiceFailure {
    /// Another dialog is showing.
    Busy,
    /// No dialog can be shown, or the folder to start in does not exist.
    Unavailable,
}

/// One host-direction file choice message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileChoiceHostMessage {
    /// The person chose a file to open, or where to save.
    Chosen {
        /// The request this answers.
        request: FileChoiceRequestName,
        /// The chosen file's name alone.
        name: String,
        /// Its path below the request's folder with forward slashes, or empty when outside.
        relative: String,
    },
    /// The person closed the dialog without choosing.
    Dismissed {
        /// The request this answers.
        request: FileChoiceRequestName,
    },
    /// The request ended without a choice.
    Failed {
        /// The request this answers.
        request: FileChoiceRequestName,
        /// Why.
        reason: FileChoiceFailure,
    },
}

impl FileChoiceHostMessage {
    /// Encode this message addressed to `session`.
    ///
    /// # Errors
    /// Rejects text beyond its bounds, generated-contract drift, encoding failure or an
    /// exceeded byte bound.
    pub fn encode(&self, session: &PanelSession) -> Result<Vec<u8>, FileChoiceError> {
        let units = |value: &str, maximum: usize| value.encode_utf16().count() <= maximum;
        let bounded = match self {
            Self::Chosen { name, relative, .. } => {
                units(name, MAX_FILE_CHOICE_NAME_UNITS)
                    && units(relative, MAX_FILE_CHOICE_PATH_UNITS)
            }
            Self::Dismissed { .. } | Self::Failed { .. } => true,
        };
        if !bounded {
            return Err(FileChoiceError::Invalid);
        }
        let value = wire::encode(self, session.as_str());
        if !value.valid() {
            return Err(FileChoiceError::Invalid);
        }
        let encoded = serde_json::to_vec(&value).map_err(FileChoiceError::Malformed)?;
        if encoded.len() > MAX_FILE_CHOICE_MESSAGE_BYTES {
            return Err(FileChoiceError::TooLarge);
        }
        Ok(encoded)
    }
}

/// What a dialog is asked to do.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileChoicePurpose {
    /// Choose one existing file.
    Open,
    /// Choose where a file is to be written, suggesting this file name.
    Save {
        /// The suggested file name; may be empty.
        name: String,
    },
}

/// One page-direction request for a dialog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileChoiceClientMessage {
    /// The current panel navigation.
    pub session: PanelSession,
    /// The page's name for this request.
    pub request: FileChoiceRequestName,
    /// Where the dialog starts, as the page named it; never trusted beyond that.
    pub folder: String,
    /// The one extension the dialog offers.
    pub extension: FileChoiceExtension,
    /// Open or save.
    pub purpose: FileChoicePurpose,
}

impl FileChoiceClientMessage {
    /// Borrow the identifier addressed by this message.
    pub fn session(&self) -> &PanelSession {
        &self.session
    }

    /// Decode one page-direction message without authorizing its sender.
    ///
    /// # Errors
    /// Rejects oversized, malformed, extended or out-of-bounds messages and foreign protocols.
    pub fn decode(bytes: &[u8]) -> Result<Self, FileChoiceError> {
        if bytes.len() > MAX_FILE_CHOICE_MESSAGE_BYTES {
            return Err(FileChoiceError::TooLarge);
        }
        let message: wire::FileChoiceClientMessage =
            serde_json::from_slice(bytes).map_err(FileChoiceError::Malformed)?;
        if !message.valid() {
            return Err(FileChoiceError::Invalid);
        }
        wire::decode(message)
    }
}

/// Identify a bounded wire failure while retaining actual JSON errors as causes.
#[derive(Debug)]
pub enum FileChoiceError {
    /// The message exceeded its fixed byte bound.
    TooLarge,
    /// JSON does not match a closed message shape.
    Malformed(serde_json::Error),
    /// A field violates the shared schema.
    Invalid,
    /// The addressed panel session is invalid.
    Session(PanelError),
}

impl fmt::Display for FileChoiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => formatter.write_str("File choice message exceeds its byte bound"),
            Self::Malformed(_) => formatter.write_str("File choice message is malformed"),
            Self::Invalid => formatter.write_str("File choice message violates its schema"),
            Self::Session(_) => formatter.write_str("File choice message names an invalid session"),
        }
    }
}

impl Error for FileChoiceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Malformed(cause) => Some(cause),
            Self::Session(cause) => Some(cause),
            Self::TooLarge | Self::Invalid => None,
        }
    }
}
