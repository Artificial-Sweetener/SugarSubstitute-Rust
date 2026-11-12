// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Versioned, bounded wire form of update-attempt messages between the preparation helper and
//! the process that launched it. The helper only writes; its parent validates every frame.
use super::{
    AttemptOutcome, AttemptProgress, MAX_CAUSE_BYTES, MAX_CAUSES, MAX_VERSION_BYTES, UpdateReport,
};
use serde::{Deserialize, Serialize};
use std::{fmt, num::NonZeroU64};

/// Registered protocol name of the update-attempt channel.
pub const PROTOCOL: &str = "sugarsubstitute.update-attempt";
/// Protocol version written and accepted by this build.
pub const PROTOCOL_VERSION: u32 = 1;
/// Largest encoded message, in bytes, accepted before decoding.
pub const MAX_MESSAGE_BYTES: usize = 8 * 1024;

/// Identity of one attempt, chosen by the launching process and passed to its helper. A frame
/// for any other attempt is rejected, so a stale or foreign writer cannot speak for this one.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct UpdateAttemptKey(NonZeroU64);
impl UpdateAttemptKey {
    /// Wrap a nonzero attempt identity.
    pub const fn new(attempt: NonZeroU64) -> Self {
        Self(attempt)
    }
    /// The attempt identity.
    pub const fn get(self) -> NonZeroU64 {
        self.0
    }
}

/// One message on the channel.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "message", rename_all = "snake_case", deny_unknown_fields)]
pub enum AttemptMessage {
    /// Live progress; the latest replaces earlier progress.
    Progress {
        /// Current stage and byte counts.
        progress: AttemptProgress,
    },
    /// The attempt's terminal result; nothing may follow it.
    Finished {
        /// How the attempt ended.
        result: AttemptOutcome,
    },
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    protocol_version: u32,
    attempt: UpdateAttemptKey,
    body: AttemptMessage,
}

/// A frame was malformed, oversized, for another attempt, or outside its bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttemptCodecError {
    /// The encoded frame exceeds `MAX_MESSAGE_BYTES`.
    Oversized,
    /// The bytes are not a frame of this protocol.
    Malformed,
    /// The frame names an unsupported protocol version.
    Version,
    /// The frame belongs to a different attempt.
    Attempt,
    /// A field exceeds its bound or contradicts another field.
    Bounds,
}
impl fmt::Display for AttemptCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Oversized => "Update attempt frame is oversized",
            Self::Malformed => "Update attempt frame is malformed",
            Self::Version => "Update attempt frame has an unsupported protocol version",
            Self::Attempt => "Update attempt frame belongs to another attempt",
            Self::Bounds => "Update attempt frame exceeds its bounds",
        })
    }
}
impl std::error::Error for AttemptCodecError {}

/// Encode one message for `attempt` after checking the same bounds the reader enforces.
/// # Errors
/// Rejects out-of-bounds content and frames that would exceed `MAX_MESSAGE_BYTES`.
pub fn encode(
    attempt: UpdateAttemptKey,
    message: &AttemptMessage,
) -> Result<Vec<u8>, AttemptCodecError> {
    validate(message)?;
    let bytes = serde_json::to_vec(&Frame {
        protocol_version: PROTOCOL_VERSION,
        attempt,
        body: message.clone(),
    })
    .map_err(|_| AttemptCodecError::Malformed)?;
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(AttemptCodecError::Oversized);
    }
    Ok(bytes)
}

/// Decode and validate one frame expected from `attempt`'s helper.
/// # Errors
/// Rejects oversized or malformed bytes, other versions, other attempts and bound violations.
pub fn decode(
    attempt: UpdateAttemptKey,
    bytes: &[u8],
) -> Result<AttemptMessage, AttemptCodecError> {
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(AttemptCodecError::Oversized);
    }
    let frame: Frame = serde_json::from_slice(bytes).map_err(|_| AttemptCodecError::Malformed)?;
    if frame.protocol_version != PROTOCOL_VERSION {
        return Err(AttemptCodecError::Version);
    }
    if frame.attempt != attempt {
        return Err(AttemptCodecError::Attempt);
    }
    validate(&frame.body)?;
    Ok(frame.body)
}

fn validate(message: &AttemptMessage) -> Result<(), AttemptCodecError> {
    let valid = match message {
        AttemptMessage::Progress { progress } => {
            progress.bytes.is_none_or(|(received, total)| received <= total)
        }
        AttemptMessage::Finished { result: AttemptOutcome::Prepared { version } } => {
            !version.is_empty() && version.len() <= MAX_VERSION_BYTES
        }
        AttemptMessage::Finished { result: AttemptOutcome::Failed(report) } => valid_report(report),
        AttemptMessage::Finished { .. } => true,
    };
    if valid { Ok(()) } else { Err(AttemptCodecError::Bounds) }
}

/// A report is accepted only in the form `UpdateReport::from_error` produces: bounded and
/// already sanitized, so a misbehaving helper cannot smuggle paths or addresses into it.
fn valid_report(report: &UpdateReport) -> bool {
    report.version.as_ref().is_none_or(|version| version.len() <= MAX_VERSION_BYTES)
        && report.causes.len() <= MAX_CAUSES
        && report
            .causes
            .iter()
            .all(|cause| cause.len() <= MAX_CAUSE_BYTES && super::sanitize(cause) == *cause)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::attempt::AttemptPhase;
    type Result = std::result::Result<(), Box<dyn std::error::Error>>;

    fn key(value: u64) -> std::result::Result<UpdateAttemptKey, &'static str> {
        NonZeroU64::new(value).map(UpdateAttemptKey::new).ok_or("zero attempt")
    }

    #[test]
    fn progress_and_every_result_round_trip_for_their_own_attempt() -> Result {
        let attempt = key(7)?;
        let messages = [
            AttemptMessage::Progress {
                progress: AttemptProgress {
                    phase: AttemptPhase::Downloading,
                    bytes: Some((10, 100)),
                },
            },
            AttemptMessage::Finished { result: AttemptOutcome::Current },
            AttemptMessage::Finished {
                result: AttemptOutcome::Prepared { version: "2.0.0".into() },
            },
            AttemptMessage::Finished { result: AttemptOutcome::NeedsInstaller },
            AttemptMessage::Finished { result: AttemptOutcome::AlreadyPreparing },
            AttemptMessage::Finished {
                result: AttemptOutcome::Failed(UpdateReport {
                    version: Some("2.0.0".into()),
                    phase: AttemptPhase::Checking,
                    causes: vec!["Update repository transport failed".into()],
                }),
            },
        ];
        for message in messages {
            assert_eq!(decode(attempt, &encode(attempt, &message)?)?, message);
        }
        Ok(())
    }

    #[test]
    fn frames_for_other_attempts_versions_or_shapes_are_rejected() -> Result {
        let message = AttemptMessage::Finished { result: AttemptOutcome::Current };
        let bytes = encode(key(7)?, &message)?;
        assert_eq!(decode(key(8)?, &bytes), Err(AttemptCodecError::Attempt));
        let mut value: serde_json::Value = serde_json::from_slice(&bytes)?;
        value["protocol_version"] = 2.into();
        assert_eq!(decode(key(7)?, &serde_json::to_vec(&value)?), Err(AttemptCodecError::Version));
        value["protocol_version"] = 1.into();
        value["unexpected"] = true.into();
        assert_eq!(
            decode(key(7)?, &serde_json::to_vec(&value)?),
            Err(AttemptCodecError::Malformed)
        );
        assert_eq!(decode(key(7)?, b"not json"), Err(AttemptCodecError::Malformed));
        assert_eq!(
            decode(key(7)?, &vec![b' '; MAX_MESSAGE_BYTES + 1]),
            Err(AttemptCodecError::Oversized)
        );
        Ok(())
    }

    #[test]
    fn unsanitized_or_contradictory_content_is_rejected_on_both_sides() -> Result {
        let attempt = key(3)?;
        let leaking = AttemptMessage::Finished {
            result: AttemptOutcome::Failed(UpdateReport {
                version: None,
                phase: AttemptPhase::Preparing,
                causes: vec![r"Cannot open C:\Users\someone\file".into()],
            }),
        };
        assert_eq!(encode(attempt, &leaking), Err(AttemptCodecError::Bounds));
        let frame = serde_json::json!({
            "protocol_version": 1, "attempt": 3, "body": {"message": "finished",
            "result": {"outcome": "failed", "version": null, "phase": "preparing",
                "causes": [r"Cannot open C:\Users\someone\file"]}}
        });
        assert_eq!(decode(attempt, &serde_json::to_vec(&frame)?), Err(AttemptCodecError::Bounds));
        let backwards = AttemptMessage::Progress {
            progress: AttemptProgress { phase: AttemptPhase::Downloading, bytes: Some((5, 4)) },
        };
        assert_eq!(encode(attempt, &backwards), Err(AttemptCodecError::Bounds));
        Ok(())
    }
}
