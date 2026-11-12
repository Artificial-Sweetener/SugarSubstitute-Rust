// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Durable local subscription revisions, independent of signed release identities.
use super::{Channel, Rejection};
use serde::{Deserialize, Serialize};

/// Maximum serialized preference size, in bytes.
pub const MAX_BYTES: usize = 256;

/// A local choice whose revision prevents stale settings writes, including channel round trips.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Subscription {
    revision: u64,
    channel: Channel,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    schema_version: u32,
    revision: u64,
    channel: Channel,
}

impl Default for Subscription {
    fn default() -> Self {
        Self { revision: 0, channel: Channel::Release }
    }
}

impl Subscription {
    /// Current locally selected update channel.
    pub fn channel(self) -> Channel {
        self.channel
    }

    /// Monotonically increasing local choice revision; unrelated to package release sequences.
    pub fn revision(self) -> u64 {
        self.revision
    }

    /// Change a choice once; repeating the current choice is idempotent.
    /// # Errors
    /// Rejects revision exhaustion without wrapping or changing the channel.
    pub fn select(self, channel: Channel) -> Result<Self, Rejection> {
        if channel == self.channel {
            return Ok(self);
        }
        Ok(Self { revision: self.revision.checked_add(1).ok_or(Rejection::LocalState)?, channel })
    }

    /// Decode one bounded, versioned local preference, never remote update metadata.
    /// # Errors
    /// Rejects unknown fields/versions, malformed values and a non-default revision zero.
    pub fn decode(bytes: &[u8]) -> Result<Self, Rejection> {
        if bytes.len() > MAX_BYTES {
            return Err(Rejection::Bound);
        }
        let wire: Wire = serde_json::from_slice(bytes).map_err(|_| Rejection::Format)?;
        if wire.schema_version != 1 || (wire.revision == 0 && wire.channel != Channel::Release) {
            return Err(Rejection::Format);
        }
        Ok(Self { revision: wire.revision, channel: wire.channel })
    }

    /// Serialize the current version without filesystem or signing authority.
    /// # Errors
    /// Reports serialization failure without returning partial bytes.
    pub fn encode(self) -> Result<Vec<u8>, Rejection> {
        serde_json::to_vec(&Wire {
            schema_version: 1,
            revision: self.revision,
            channel: self.channel,
        })
        .map_err(|_| Rejection::Format)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trips_idempotence_and_channel_round_trip_revisions() -> Result<(), Rejection>
    {
        let release = Subscription::default();
        assert_eq!(release.channel(), Channel::Release);
        assert_eq!(release.select(Channel::Release)?, release);
        let canary = release.select(Channel::Canary)?;
        assert_eq!(Subscription::decode(&canary.encode()?)?, canary);
        let returned = canary.select(Channel::Release)?;
        assert_eq!(returned.revision(), 2);
        assert_ne!(returned, release);
        Ok(())
    }

    #[test]
    fn invalid_preferences_and_exhausted_revisions_are_rejected() {
        for value in [
            r#"{"schema_version":2,"revision":0,"channel":"release"}"#,
            r#"{"schema_version":1,"revision":0,"channel":"canary"}"#,
            r#"{"schema_version":1,"revision":1,"channel":"beta"}"#,
            r#"{"schema_version":1,"revision":1,"channel":"release","url":"foreign"}"#,
        ] {
            assert!(Subscription::decode(value.as_bytes()).is_err());
        }
        assert!(Subscription::decode(&[b' '; MAX_BYTES + 1]).is_err());
        assert!(
            Subscription { revision: u64::MAX, channel: Channel::Release }
                .select(Channel::Canary)
                .is_err()
        );
    }
}
