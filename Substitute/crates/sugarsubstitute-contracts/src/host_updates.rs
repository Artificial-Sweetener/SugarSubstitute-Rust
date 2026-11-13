// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exchange the host's update state with the embedded SugarCubes page (`sugarcubes.updates` v1).
//!
//! The page sends `watch` once the host description lists the protocol; the host answers with
//! its update state and sends a new state, with a higher revision, whenever it changes. The page
//! asks to check now or to restart to apply, and the host answers each such request accepted or
//! rejected by name. Decoding validates shape and bounds only; it grants no host authority.

use crate::panel::{PanelError, PanelSession};
use std::{error::Error, fmt};

mod wire;

#[cfg(test)]
mod tests;

/// Maximum encoded updates message admitted or produced, in UTF-8 bytes.
pub const MAX_UPDATES_MESSAGE_BYTES: usize = 16384;
const PROTOCOL: &str = "sugarcubes.updates";
const VERSION: u64 = 1;
/// Most causes one state's report carries.
pub const MAX_REPORT_CAUSES: usize = 8;
/// Longest cause one state's report carries, in Unicode scalar values.
pub const MAX_CAUSE_CHARACTERS: usize = 1024;

/// A lowercase release token, 1 to 64 of `a-z`, `0-9`, `.`, `-`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdatesVersion(String);

impl UpdatesVersion {
    /// Accept a release label that is already a schema token.
    ///
    /// # Errors
    /// Rejects an empty, oversized or non-token label.
    pub fn new(value: &str) -> Result<Self, UpdatesError> {
        if token(value, 64) { Ok(Self(value.to_owned())) } else { Err(UpdatesError::Invalid) }
    }

    /// Borrow the token.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The page's own name for one request, 1 to 48 of `a-z`, `0-9`, `.`, `-`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdatesRequestName(String);

impl UpdatesRequestName {
    /// Accept a request name that is a schema token.
    ///
    /// # Errors
    /// Rejects an empty, oversized or non-token name.
    pub fn new(value: &str) -> Result<Self, UpdatesError> {
        if token(value, 48) { Ok(Self(value.to_owned())) } else { Err(UpdatesError::Invalid) }
    }

    /// Borrow the name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The release channel the host follows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdatesChannel {
    /// Released versions only.
    Stable,
    /// Beta versions as well.
    Beta,
}

/// What failed most recently.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdatesFailure {
    /// The host couldn't check for an update.
    Check,
    /// The host couldn't download, prepare or apply an update.
    Update,
}

/// Where an attempt stopped, for the report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdatesPhase {
    /// Checking for an update.
    Check,
    /// Downloading the update.
    Download,
    /// Verifying the download.
    Verify,
    /// Installing the update beside the running version.
    Install,
    /// Starting the new version.
    Start,
}

/// The host's update situation as the page shows it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdatesStatus {
    /// The running version is the newest the host knows of.
    UpToDate,
    /// The host is checking now.
    Checking,
    /// A newer version exists but is not ready to apply.
    Available(UpdatesVersion),
    /// A newer version is ready and applies on restart.
    Ready(UpdatesVersion),
    /// The latest check or update failed.
    Failed(UpdatesFailure),
}

/// A readable account of the latest failed attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdatesReport {
    version: UpdatesVersion,
    phase: UpdatesPhase,
    causes: Vec<String>,
}

impl UpdatesReport {
    /// Retain a report with 1 to [`MAX_REPORT_CAUSES`] causes, outermost first.
    ///
    /// Causes are plain sentences without secrets or unnecessary paths.
    ///
    /// # Errors
    /// Rejects no causes, too many causes or a cause beyond [`MAX_CAUSE_CHARACTERS`].
    pub fn new(
        version: UpdatesVersion,
        phase: UpdatesPhase,
        causes: Vec<String>,
    ) -> Result<Self, UpdatesError> {
        if causes.is_empty()
            || causes.len() > MAX_REPORT_CAUSES
            || causes.iter().any(|cause| cause.chars().count() > MAX_CAUSE_CHARACTERS)
        {
            return Err(UpdatesError::Invalid);
        }
        Ok(Self { version, phase, causes })
    }
}

/// Whether a restart is waiting for something.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdatesScheduledRestart {
    /// No restart is waiting.
    None,
    /// The host restarts once the running workflow finishes.
    AfterWorkflow,
}

/// When the page asks the host to restart.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdatesRestartTiming {
    /// Restart now.
    Now,
    /// Restart once the running workflow finishes.
    AfterWorkflow,
}

/// Why the host declined a request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdatesRejection {
    /// The host is already doing what was asked, or is closing.
    Busy,
    /// No update is ready to apply.
    NotReady,
    /// The host cannot do this now.
    Unavailable,
}

/// The host's complete update state for one revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdatesState {
    /// The running version.
    pub current_version: UpdatesVersion,
    /// The channel the host follows.
    pub channel: UpdatesChannel,
    /// When the last check finished, in milliseconds since the Unix epoch.
    pub last_successful_check_ms: Option<u64>,
    /// The current situation.
    pub status: UpdatesStatus,
    /// The latest failed attempt's report, while it is current.
    pub report: Option<UpdatesReport>,
    /// The previous version is running after the newer one failed to start.
    pub previous_version_running: bool,
    /// A workflow is running, so a restart can wait for it.
    pub workflow_running: bool,
    /// What a restart is waiting for.
    pub scheduled_restart: UpdatesScheduledRestart,
}

/// One host-direction updates message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdatesHostMessage {
    /// The complete state; `revision` increases with every change within a panel session.
    State {
        /// At least 1; the page ignores a state that is not newer than the last it applied.
        revision: u64,
        /// The state.
        update: Box<UpdatesState>,
    },
    /// The host accepted the named request.
    Accepted(UpdatesRequestName),
    /// The host declined the named request.
    Rejected {
        /// The page's request name.
        request: UpdatesRequestName,
        /// Why.
        reason: UpdatesRejection,
    },
}

impl UpdatesHostMessage {
    /// Encode this message addressed to `session`.
    ///
    /// # Errors
    /// Rejects a revision outside 1 to 2^53-1, generated-contract drift, encoding failure or an
    /// exceeded byte bound.
    pub fn encode(&self, session: &PanelSession) -> Result<Vec<u8>, UpdatesError> {
        let value = wire::encode(self, session.as_str());
        if !value.valid() {
            return Err(UpdatesError::Invalid);
        }
        let encoded = serde_json::to_vec(&value).map_err(UpdatesError::Malformed)?;
        if encoded.len() > MAX_UPDATES_MESSAGE_BYTES {
            return Err(UpdatesError::TooLarge);
        }
        Ok(encoded)
    }
}

/// One page-direction updates message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdatesClientMessage {
    /// The page starts following the host's update state.
    Watch(PanelSession),
    /// The page asks the host to check now.
    Check {
        /// The current panel navigation.
        session: PanelSession,
        /// The page's request name.
        request: UpdatesRequestName,
    },
    /// The page asks the host to apply a ready update by its normal close and relaunch.
    Restart {
        /// The current panel navigation.
        session: PanelSession,
        /// The page's request name.
        request: UpdatesRequestName,
        /// Now, or once the running workflow finishes.
        when: UpdatesRestartTiming,
    },
}

impl UpdatesClientMessage {
    /// Borrow the identifier addressed by this message.
    pub fn session(&self) -> &PanelSession {
        match self {
            Self::Watch(session) | Self::Check { session, .. } | Self::Restart { session, .. } => {
                session
            }
        }
    }

    /// Decode one page-direction message without authorizing its sender.
    ///
    /// # Errors
    /// Rejects oversized, malformed, extended or out-of-bounds messages and foreign protocols.
    pub fn decode(bytes: &[u8]) -> Result<Self, UpdatesError> {
        if bytes.len() > MAX_UPDATES_MESSAGE_BYTES {
            return Err(UpdatesError::TooLarge);
        }
        let message: wire::UpdatesClientMessage =
            serde_json::from_slice(bytes).map_err(UpdatesError::Malformed)?;
        if !message.valid() {
            return Err(UpdatesError::Invalid);
        }
        wire::decode(message)
    }
}

/// Identify a bounded wire failure while retaining actual JSON errors as causes.
#[derive(Debug)]
pub enum UpdatesError {
    /// The message exceeded its fixed byte bound.
    TooLarge,
    /// JSON does not match a closed message shape.
    Malformed(serde_json::Error),
    /// A field violates the shared schema.
    Invalid,
    /// The addressed panel session is invalid.
    Session(PanelError),
}

impl fmt::Display for UpdatesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => formatter.write_str("Updates message exceeds its byte bound"),
            Self::Malformed(_) => formatter.write_str("Updates message is malformed"),
            Self::Invalid => formatter.write_str("Updates message violates its schema"),
            Self::Session(_) => formatter.write_str("Updates message names an invalid session"),
        }
    }
}

impl Error for UpdatesError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Malformed(cause) => Some(cause),
            Self::Session(cause) => Some(cause),
            Self::TooLarge | Self::Invalid => None,
        }
    }
}

/// Whether `value` is a lowercase schema token of at most `maximum` bytes.
fn token(value: &str, maximum: usize) -> bool {
    (1..=maximum).contains(&value.len())
        && value.bytes().all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-'))
}
