// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Tell the embedded SugarCubes page what the host owes the user about their session
//! (`sugarcubes.session-notice` v1 and v2).
//!
//! The page sends `watch` once the host description lists the protocol and it can show a
//! notice; the host answers every watch with a notice it owes, or with none. The page shows
//! the notice and answers `shown`, naming it; until then the host owes it to every later
//! watch. Version 2, which a host offers by declaring the capability `session-notice.lines`,
//! adds lines the host words itself and may send to a watching page at any time. The host
//! answers each page in the version its watch spoke. Decoding validates shape and bounds only;
//! it grants no host authority.

use crate::panel::{PanelError, PanelSession};
use std::{error::Error, fmt};

mod wire;
mod wire_v2;

#[cfg(test)]
mod tests;

/// Maximum encoded session notice message admitted or produced, in UTF-8 bytes.
pub const MAX_SESSION_NOTICE_MESSAGE_BYTES: usize = 4096;
/// Longest line a host sends, in Unicode code points.
pub const MAX_LINE_CHARACTERS: usize = 400;
/// Largest notice number the page's number type carries exactly.
pub const MAX_NOTICE_NUMBER: u64 = 9_007_199_254_740_991;
const PROTOCOL: &str = "sugarcubes.session-notice";

/// The protocol version a page speaks; the host answers in the same one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionNoticeVersion {
    /// The notice of unsaved work only.
    One,
    /// Also lines the host words itself, sent whenever the host has one.
    Two,
}

impl SessionNoticeVersion {
    /// The number the wire carries.
    pub(crate) const fn number(self) -> u64 {
        match self {
            Self::One => 1,
            Self::Two => 2,
        }
    }
}

/// How a line reads: plain information, something to mind, or something that went wrong.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoticeTone {
    /// Plain information.
    Info,
    /// Something the user should mind.
    Warn,
    /// Something went wrong.
    Error,
}

/// One notice's number within the host's run, 1 to [`MAX_NOTICE_NUMBER`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NoticeNumber(u64);

impl NoticeNumber {
    /// Accept a number the schema can carry.
    ///
    /// # Errors
    /// Rejects zero and numbers beyond [`MAX_NOTICE_NUMBER`].
    pub fn new(value: u64) -> Result<Self, SessionNoticeError> {
        if (1..=MAX_NOTICE_NUMBER).contains(&value) {
            Ok(Self(value))
        } else {
            Err(SessionNoticeError::Invalid)
        }
    }

    /// The number.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// One host-direction session notice message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionNoticeHostMessage {
    /// One sentence the host has for the user, already in the user's language. Version 2 only.
    Line {
        /// The notice the page names when it has shown it.
        notice: NoticeNumber,
        /// How the line reads.
        tone: NoticeTone,
        /// The sentence, at most [`MAX_LINE_CHARACTERS`] code points and not blank.
        text: String,
        /// Whether the page keeps the line on screen until the user closes it.
        lasting: bool,
    },
    /// The previous close did not keep the user's latest work; at least one flag is set.
    NotSaved {
        /// The notice the page names when it has shown it.
        notice: NoticeNumber,
        /// The latest workflow changes were lost.
        workspace: bool,
        /// The latest input canvas edits were lost.
        inputs: bool,
    },
    /// The host owes no notice.
    None,
}

impl SessionNoticeHostMessage {
    /// Encode this message addressed to `session`, in the `version` its page speaks.
    ///
    /// # Errors
    /// Rejects a notice that names nothing unsaved, a blank or overlong line, a line for a
    /// version 1 page, generated-contract drift, encoding failure or an exceeded byte bound.
    pub fn encode(
        &self,
        session: &PanelSession,
        version: SessionNoticeVersion,
    ) -> Result<Vec<u8>, SessionNoticeError> {
        if matches!(self, Self::NotSaved { workspace: false, inputs: false, .. }) {
            return Err(SessionNoticeError::Invalid);
        }
        if let Self::Line { text, .. } = self
            && (text.trim().is_empty() || text.chars().count() > MAX_LINE_CHARACTERS)
        {
            return Err(SessionNoticeError::Invalid);
        }
        let encoded = match version {
            SessionNoticeVersion::One => {
                let value = wire::encode(self, session.as_str())?;
                if !value.valid() {
                    return Err(SessionNoticeError::Invalid);
                }
                serde_json::to_vec(&value)
            }
            SessionNoticeVersion::Two => {
                let value = wire_v2::encode(self, session.as_str());
                if !value.valid() {
                    return Err(SessionNoticeError::Invalid);
                }
                serde_json::to_vec(&value)
            }
        }
        .map_err(SessionNoticeError::Malformed)?;
        if encoded.len() > MAX_SESSION_NOTICE_MESSAGE_BYTES {
            return Err(SessionNoticeError::TooLarge);
        }
        Ok(encoded)
    }
}

/// One page-direction session notice message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionNoticeClientMessage {
    /// The page can show a notice and asks for the one the host owes.
    Watch {
        /// The current panel navigation.
        session: PanelSession,
        /// The version the page speaks, in which the host answers it.
        version: SessionNoticeVersion,
    },
    /// The page showed the named notice to the user.
    Shown {
        /// The current panel navigation.
        session: PanelSession,
        /// The notice shown.
        notice: NoticeNumber,
    },
}

impl SessionNoticeClientMessage {
    /// Borrow the identifier addressed by this message.
    pub fn session(&self) -> &PanelSession {
        match self {
            Self::Watch { session, .. } | Self::Shown { session, .. } => session,
        }
    }

    /// Decode one page-direction message without authorizing its sender.
    ///
    /// # Errors
    /// Rejects oversized, malformed, extended or out-of-bounds messages and foreign protocols.
    pub fn decode(bytes: &[u8]) -> Result<Self, SessionNoticeError> {
        if bytes.len() > MAX_SESSION_NOTICE_MESSAGE_BYTES {
            return Err(SessionNoticeError::TooLarge);
        }
        // The two versions differ only in the number each message carries, so a message that
        // is not a valid version 1 message is read as version 2 before it is refused.
        match serde_json::from_slice::<wire::SessionNoticeClientMessage>(bytes) {
            Ok(message) if message.valid() => wire::decode(message),
            first => {
                let message: wire_v2::SessionNoticeClientMessage = serde_json::from_slice(bytes)
                    .map_err(|second| {
                        SessionNoticeError::Malformed(first.err().unwrap_or(second))
                    })?;
                if !message.valid() {
                    return Err(SessionNoticeError::Invalid);
                }
                wire_v2::decode(message)
            }
        }
    }
}

/// Identify a bounded wire failure while retaining actual JSON errors as causes.
#[derive(Debug)]
pub enum SessionNoticeError {
    /// The message exceeded its fixed byte bound.
    TooLarge,
    /// JSON does not match a closed message shape.
    Malformed(serde_json::Error),
    /// A field violates the shared schema.
    Invalid,
    /// The addressed panel session is invalid.
    Session(PanelError),
}

impl fmt::Display for SessionNoticeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => formatter.write_str("Session notice message exceeds its byte bound"),
            Self::Malformed(_) => formatter.write_str("Session notice message is malformed"),
            Self::Invalid => formatter.write_str("Session notice message violates its schema"),
            Self::Session(_) => {
                formatter.write_str("Session notice message names an invalid session")
            }
        }
    }
}

impl Error for SessionNoticeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Malformed(cause) => Some(cause),
            Self::Session(cause) => Some(cause),
            Self::TooLarge | Self::Invalid => None,
        }
    }
}
