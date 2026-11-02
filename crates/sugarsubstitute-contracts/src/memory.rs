// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exchange the application memory budget with the embedded SugarCubes page.
//!
//! The host tells the page how many bytes its caches may hold and how pressed the system is;
//! the page reports what its caches hold and the user's memory preference, which ComfyUI's
//! settings persist. Messages are addressed to the current panel session. Decoding validates
//! shape and bounds only; it grants no host authority.

use crate::panel::{PanelError, PanelSession};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

/// Maximum encoded memory message admitted before JSON decoding, in UTF-8 bytes.
pub const MAX_MEMORY_MESSAGE_BYTES: usize = 4096;
/// Maximum cache consumers one usage report may itemize.
pub const MAX_MEMORY_CONSUMERS: usize = 16;
const PROTOCOL: &str = "sugarcubes.memory";
const VERSION: u64 = 1;
/// Largest byte count JavaScript represents exactly.
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// How hard the system is pressed for memory.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase", deny_unknown_fields)]
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

impl MemoryPreferenceChoice {
    fn valid(self) -> bool {
        match self {
            Self::Share { percent } => (10..=75).contains(&percent),
            Self::Fixed { bytes } => (1..=MAX_SAFE_INTEGER).contains(&bytes),
        }
    }
}

/// One cache family's reported bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
        let wire: WireClient = serde_json::from_slice(bytes).map_err(MemoryError::Malformed)?;
        match wire {
            WireClient::Usage {
                protocol,
                version,
                session,
                used_bytes,
                demand_bytes,
                consumers,
            } => {
                let valid = header(&protocol, version)
                    && used_bytes <= MAX_SAFE_INTEGER
                    && demand_bytes <= MAX_SAFE_INTEGER
                    && consumers.len() <= MAX_MEMORY_CONSUMERS
                    && consumers.iter().all(|consumer| {
                        valid_consumer_id(&consumer.id) && consumer.bytes <= MAX_SAFE_INTEGER
                    });
                if !valid {
                    return Err(MemoryError::Invalid);
                }
                Ok(Self::Usage {
                    session: session_of(session)?,
                    used_bytes,
                    demand_bytes,
                    consumers,
                })
            }
            WireClient::Preference { protocol, version, session, preference } => {
                if !header(&protocol, version) || !preference.valid() {
                    return Err(MemoryError::Invalid);
                }
                Ok(Self::Preference { session: session_of(session)?, preference })
            }
        }
    }
}

impl MemoryAllowance {
    /// Encode this allowance for `session` as one host-direction message.
    ///
    /// # Errors
    /// Rejects byte counts JavaScript cannot represent exactly.
    pub fn encode(&self, session: &PanelSession) -> Result<Vec<u8>, MemoryError> {
        let exact = [self.generation, self.allowance_bytes, self.ceiling_bytes, self.total_bytes]
            .into_iter()
            .all(|value| value <= MAX_SAFE_INTEGER);
        if !exact || !self.preference.valid() {
            return Err(MemoryError::Invalid);
        }
        serde_json::to_vec(&WireAllowance {
            protocol: PROTOCOL,
            version: VERSION,
            session: session.as_str(),
            kind: "allowance",
            generation: self.generation,
            level: self.level,
            allowance_bytes: self.allowance_bytes,
            ceiling_bytes: self.ceiling_bytes,
            total_bytes: self.total_bytes,
            preference: self.preference,
        })
        .map_err(MemoryError::Malformed)
    }
}

fn header(protocol: &str, version: u64) -> bool {
    protocol == PROTOCOL && version == VERSION
}

fn session_of(value: String) -> Result<PanelSession, MemoryError> {
    PanelSession::new(value).map_err(MemoryError::Session)
}

fn valid_consumer_id(id: &str) -> bool {
    (1..=48).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b".-".contains(&byte))
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
enum WireClient {
    #[serde(rename_all = "camelCase")]
    Usage {
        protocol: String,
        version: u64,
        session: String,
        used_bytes: u64,
        demand_bytes: u64,
        consumers: Vec<MemoryConsumer>,
    },
    #[serde(rename_all = "camelCase")]
    Preference {
        protocol: String,
        version: u64,
        session: String,
        preference: MemoryPreferenceChoice,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WireAllowance<'a> {
    protocol: &'static str,
    version: u64,
    session: &'a str,
    #[serde(rename = "type")]
    kind: &'static str,
    generation: u64,
    level: MemoryLevel,
    allowance_bytes: u64,
    ceiling_bytes: u64,
    total_bytes: u64,
    preference: MemoryPreferenceChoice,
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
