// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exchange the application memory budget with the embedded SugarCubes page.
//!
//! The host tells the page how many bytes its caches may hold and how pressed the system is;
//! the page reports what its caches hold and the user's memory preference, which ComfyUI's
//! settings persist. Messages are addressed to the current panel session. Decoding validates
//! shape and bounds only; it grants no host authority.

use crate::panel::{PanelError, PanelSession};
use std::{error::Error, fmt};

mod wire;

/// Maximum encoded memory message admitted before JSON decoding, in UTF-8 bytes.
pub const MAX_MEMORY_MESSAGE_BYTES: usize = 4096;
const PROTOCOL: &str = "sugarcubes.memory";
const VERSION: u64 = 1;

/// How hard the system is pressed for memory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryLevel {
    /// Caches may grow to their allowance.
    Normal,
    /// Caches stop growing and shed to 85% of their allowance.
    Elevated,
    /// Caches shed to half their allowance.
    High,
    /// Caches keep only pinned work.
    Critical,
}

/// The user's application memory ceiling as the page reports and displays it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryPreferenceChoice {
    /// A percentage of physical memory, between 10 and 75.
    Share {
        /// Percentage of physical memory.
        percent: u8,
    },
    /// A fixed number of bytes; the host clamps it to the machine.
    Fixed {
        /// Ceiling in bytes.
        bytes: u64,
    },
}

/// One cache family's reported bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryConsumer {
    /// Stable lowercase identifier, 1 to 48 of `a-z`, `0-9`, `.`, `-`.
    pub id: String,
    /// Bytes the consumer holds.
    pub bytes: u64,
}

/// The host's current allowance for the page's caches.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryAllowance {
    /// Increases with every allowance the host sends; the page ignores older ones.
    pub generation: u64,
    /// Current pressure.
    pub level: MemoryLevel,
    /// Bytes the page's caches may hold.
    pub allowance_bytes: u64,
    /// The application ceiling the preference resolves to on this machine.
    pub ceiling_bytes: u64,
    /// Installed physical memory.
    pub total_bytes: u64,
    /// The preference the host is applying.
    pub preference: MemoryPreferenceChoice,
}

/// One validated memory message. Budget decisions remain host-owned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryMessage {
    /// The page reports what its caches hold.
    Usage {
        /// The current panel navigation.
        session: PanelSession,
        /// Bytes all page caches hold.
        used_bytes: u64,
        /// Bytes the page's caches would hold without any limit.
        demand_bytes: u64,
        /// Bytes per cache family.
        consumers: Vec<MemoryConsumer>,
    },
    /// The page reports the user's stored preference, at load and when it changes.
    Preference {
        /// The current panel navigation.
        session: PanelSession,
        /// The stored preference.
        preference: MemoryPreferenceChoice,
    },
}

impl MemoryMessage {
    /// Borrow the identifier addressed by this message.
    pub fn session(&self) -> &PanelSession {
        match self {
            Self::Usage { session, .. } | Self::Preference { session, .. } => session,
        }
    }

    /// Decode one page-direction memory message without authorizing its sender.
    ///
    /// # Errors
    /// Rejects oversized, malformed, extended or out-of-bounds messages and foreign protocols.
    pub fn decode(bytes: &[u8]) -> Result<Self, MemoryError> {
        if bytes.len() > MAX_MEMORY_MESSAGE_BYTES {
            return Err(MemoryError::TooLarge);
        }
        let message: wire::MemoryClientMessage =
            serde_json::from_slice(bytes).map_err(MemoryError::Malformed)?;
        if !message.valid() {
            return Err(MemoryError::Invalid);
        }
        match message {
            wire::MemoryClientMessage::Usage(usage) => Ok(Self::Usage {
                session: session_of(usage.session)?,
                used_bytes: usage.used_bytes,
                demand_bytes: usage.demand_bytes,
                consumers: usage
                    .consumers
                    .into_iter()
                    .map(|consumer| MemoryConsumer { id: consumer.id, bytes: consumer.bytes })
                    .collect(),
            }),
            wire::MemoryClientMessage::Preference(preference) => Ok(Self::Preference {
                session: session_of(preference.session)?,
                preference: MemoryPreferenceChoice::from_wire(preference.preference)?,
            }),
        }
    }
}

impl MemoryAllowance {
    /// Encode this allowance for `session` as one host-direction message.
    ///
    /// # Errors
    /// Rejects byte counts JavaScript cannot represent exactly.
    pub fn encode(&self, session: &PanelSession) -> Result<Vec<u8>, MemoryError> {
        let message = wire::MemoryAllowanceMessage {
            protocol: PROTOCOL.to_owned(),
            version: VERSION,
            session: session.as_str().to_owned(),
            r#type: "allowance".to_owned(),
            generation: self.generation,
            level: self.level.to_wire(),
            allowance_bytes: self.allowance_bytes,
            ceiling_bytes: self.ceiling_bytes,
            total_bytes: self.total_bytes,
            preference: self.preference.to_wire(),
        };
        if !message.valid() {
            return Err(MemoryError::Invalid);
        }
        serde_json::to_vec(&message).map_err(MemoryError::Malformed)
    }
}

impl MemoryLevel {
    fn to_wire(self) -> wire::MemoryLevel {
        match self {
            Self::Normal => wire::MemoryLevel::Normal,
            Self::Elevated => wire::MemoryLevel::Elevated,
            Self::High => wire::MemoryLevel::High,
            Self::Critical => wire::MemoryLevel::Critical,
        }
    }
}

impl MemoryPreferenceChoice {
    fn to_wire(self) -> wire::MemoryPreference {
        match self {
            Self::Share { percent } => wire::MemoryPreference::Share { percent: percent.into() },
            Self::Fixed { bytes } => wire::MemoryPreference::Fixed { bytes },
        }
    }

    fn from_wire(preference: wire::MemoryPreference) -> Result<Self, MemoryError> {
        match preference {
            wire::MemoryPreference::Share { percent } => Ok(Self::Share {
                percent: u8::try_from(percent).map_err(|_| MemoryError::Invalid)?,
            }),
            wire::MemoryPreference::Fixed { bytes } => Ok(Self::Fixed { bytes }),
        }
    }
}

fn session_of(value: String) -> Result<PanelSession, MemoryError> {
    PanelSession::new(value).map_err(MemoryError::Session)
}

/// Identify a bounded wire failure while retaining actual JSON errors as causes.
#[derive(Debug)]
pub enum MemoryError {
    /// The encoded message exceeded its fixed admission bound.
    TooLarge,
    /// JSON does not match a closed message shape.
    Malformed(serde_json::Error),
    /// A field violates the protocol's bounds.
    Invalid,
    /// The addressed session is not a valid panel session.
    Session(PanelError),
}

impl fmt::Display for MemoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TooLarge => "Memory message exceeds its byte bound",
            Self::Malformed(_) => "Memory message is malformed",
            Self::Invalid => "Memory message violates its bounds",
            Self::Session(_) => "Memory message addresses an invalid session",
        })
    }
}

impl Error for MemoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Malformed(cause) => Some(cause),
            Self::Session(cause) => Some(cause),
            Self::TooLarge | Self::Invalid => None,
        }
    }
}

#[cfg(test)]
mod tests;
