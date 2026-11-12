// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coordinate one SugarCubes panel handshake independently of its native transport.
//!
//! The host answers every page `ready` with its description first and its configure second.

use std::{error::Error, fmt, time::Duration};
use sugarsubstitute_contracts::{
    execution::Moment,
    host::HostDescription,
    panel::{PanelMessage, PanelPresentation, PanelSession},
};

#[cfg(test)]
mod tests;

const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
const REPLY_TIMEOUT: Duration = Duration::from_secs(15);
const PING_INTERVAL: Duration = Duration::from_secs(5);

/// The observed public panel state; it grants no native or backend authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PanelState {
    /// Waiting for an accepted presentation.
    Loading,
    /// The applied page reports an active Comfy connection.
    Connected,
    /// The page reports a lost or overdue Comfy connection.
    Disconnected,
    /// The panel declined or failed to apply its configuration.
    Failed,
}

/// At most one message and one state change from a single synchronous decision.
#[derive(Debug, Default, PartialEq)]
pub struct PanelEffects {
    /// The host description, delivered immediately before `send` on the same ordered channel.
    /// Every configure carries it, so a configure that supersedes another never loses it.
    pub description: Option<HostDescription>,
    /// A public message for the exact navigation's existing transport.
    pub send: Option<PanelMessage>,
    /// A state transition to publish once, when present.
    pub state: Option<PanelState>,
}

/// Retain one session's handshake and heartbeat decisions on the composing thread.
/// The adapter must verify the actual current navigation and frame before calling `receive`.
pub struct PanelHostSession {
    session: PanelSession,
    description: HostDescription,
    presentation: PanelPresentation,
    configured: bool,
    applied: bool,
    last_reply: Moment,
    last_ping: Option<Moment>,
    last_observed: Moment,
    state: PanelState,
}

impl PanelHostSession {
    /// Begin a bounded handshake at an externally supplied monotonic time.
    ///
    /// `description` is what this host serves and declares for the whole session.
    ///
    /// # Errors
    /// Rejects a start time that cannot represent the initial timeout.
    pub fn new(
        session: PanelSession,
        description: HostDescription,
        presentation: PanelPresentation,
        now: Moment,
    ) -> Result<Self, PanelClockError> {
        now.checked_add(STARTUP_TIMEOUT).map_err(|_| PanelClockError::DeadlineOverflow)?;
        Ok(Self {
            session,
            description,
            presentation,
            configured: false,
            applied: false,
            last_reply: now,
            last_ping: None,
            last_observed: now,
            state: PanelState::Loading,
        })
    }

    /// Borrow the current public connection observation.
    pub const fn state(&self) -> PanelState {
        self.state
    }

    /// Accept a validated message addressed to this session.
    ///
    /// Foreign sessions and messages that are not valid in the current phase have no effect.
    /// The native adapter is responsible for verifying the source frame and navigation.
    ///
    /// # Errors
    /// Rejects a monotonic clock regression without changing the handshake.
    pub fn receive(
        &mut self,
        message: PanelMessage,
        now: Moment,
    ) -> Result<PanelEffects, PanelClockError> {
        self.observe_time(now)?;
        if message.session() != &self.session || self.state == PanelState::Failed {
            return Ok(PanelEffects::default());
        }
        let mut effects = PanelEffects::default();
        match message {
            PanelMessage::Ready(_) => {
                self.configured = true;
                self.configure(&mut effects);
            }
            PanelMessage::Applied(_) if self.configured => {
                self.applied = true;
                self.last_reply = now;
                effects.send = Some(self.ping(now));
            }
            PanelMessage::Pong { connected, .. } if self.applied => {
                self.last_reply = now;
                effects.state = self.present(if connected {
                    PanelState::Connected
                } else {
                    PanelState::Disconnected
                });
            }
            PanelMessage::Rejected { .. } if self.configured => {
                effects.state = self.present(PanelState::Failed);
            }
            _ => {}
        }
        Ok(effects)
    }

    /// Update host appearance without replacing the current page or workflow.
    pub fn update_presentation(&mut self, presentation: PanelPresentation) -> PanelEffects {
        if self.state == PanelState::Failed || self.presentation == presentation {
            return PanelEffects::default();
        }
        self.presentation = presentation;
        let mut effects = PanelEffects::default();
        if self.configured {
            self.configure(&mut effects);
        }
        effects
    }

    /// Detect startup failure and schedule a bounded heartbeat at the supplied monotonic time.
    ///
    /// # Errors
    /// Rejects a monotonic clock regression without changing the handshake.
    pub fn tick(&mut self, now: Moment) -> Result<PanelEffects, PanelClockError> {
        let mut effects = self.expire(now)?;
        if self.applied
            && self.state != PanelState::Failed
            && match self.last_ping {
                Some(last) => elapsed(now, last)? >= PING_INTERVAL,
                None => true,
            }
        {
            effects.send = Some(self.ping(now));
        }
        Ok(effects)
    }

    /// Apply elapsed handshake and reply deadlines without allocating a heartbeat message.
    ///
    /// Hosts may call this when command admission is unavailable; elapsed deadlines still
    /// change the observed state, while the original heartbeat remains due for later retry.
    /// # Errors
    /// Rejects a monotonic clock regression without changing the handshake.
    pub fn expire(&mut self, now: Moment) -> Result<PanelEffects, PanelClockError> {
        self.observe_time(now)?;
        let mut effects = PanelEffects::default();
        if self.state == PanelState::Failed {
            return Ok(effects);
        }
        if !self.applied && elapsed(now, self.last_reply)? >= STARTUP_TIMEOUT {
            effects.state = self.present(PanelState::Failed);
        } else if self.applied && elapsed(now, self.last_reply)? >= REPLY_TIMEOUT {
            effects.state = self.present(PanelState::Disconnected);
        }
        Ok(effects)
    }

    /// Request a fresh ping only while an applied page has replied within its original deadline.
    ///
    /// # Errors
    /// Rejects a monotonic clock regression without changing the handshake.
    pub fn retry_connection(&mut self, now: Moment) -> Result<PanelEffects, PanelClockError> {
        self.observe_time(now)?;
        if !self.applied
            || self.state == PanelState::Failed
            || elapsed(now, self.last_reply)? >= REPLY_TIMEOUT
        {
            return Ok(PanelEffects::default());
        }
        Ok(PanelEffects { send: Some(self.ping(now)), ..PanelEffects::default() })
    }

    /// Return the earliest original handshake or heartbeat deadline for the native pump.
    ///
    /// # Errors
    /// Rejects a deadline that cannot be represented without wrapping.
    pub fn next_deadline(&self) -> Result<Option<Moment>, PanelClockError> {
        let expiration = self.next_expiration()?;
        if !self.applied || self.state == PanelState::Failed {
            return Ok(expiration);
        }
        let ping = self
            .last_ping
            .map(|last| {
                last.checked_add(PING_INTERVAL).map_err(|_| PanelClockError::DeadlineOverflow)
            })
            .transpose()?;
        Ok(match (expiration, ping) {
            (Some(reply), Some(ping)) => Some(reply.min(ping)),
            (Some(deadline), None) | (None, Some(deadline)) => Some(deadline),
            (None, None) => None,
        })
    }

    /// Return the original handshake or reply cutoff independent of heartbeat admission.
    ///
    /// # Errors
    /// Rejects a deadline that cannot be represented without wrapping.
    pub fn next_expiration(&self) -> Result<Option<Moment>, PanelClockError> {
        if self.state == PanelState::Failed
            || (self.applied && self.state == PanelState::Disconnected)
        {
            return Ok(None);
        }
        let timeout = if self.applied { REPLY_TIMEOUT } else { STARTUP_TIMEOUT };
        self.last_reply
            .checked_add(timeout)
            .map(Some)
            .map_err(|_| PanelClockError::DeadlineOverflow)
    }

    /// Refuse backward observations before applying any state change.
    fn observe_time(&mut self, now: Moment) -> Result<(), PanelClockError> {
        if now < self.last_observed {
            return Err(PanelClockError::Regressed);
        }
        self.last_observed = now;
        Ok(())
    }

    /// Send the description, then configuration from the retained current presentation.
    fn configure(&self, effects: &mut PanelEffects) {
        effects.description = Some(self.description.clone());
        effects.send = Some(PanelMessage::Configure {
            session: self.session.clone(),
            presentation: self.presentation.clone(),
        });
    }

    /// Record the ping before returning it, including for synchronous delivery.
    fn ping(&mut self, now: Moment) -> PanelMessage {
        self.last_ping = Some(now);
        PanelMessage::Ping(self.session.clone())
    }

    /// Publish a state only when it actually changes.
    fn present(&mut self, state: PanelState) -> Option<PanelState> {
        if self.state == state {
            None
        } else {
            self.state = state;
            Some(state)
        }
    }
}

/// Compare two moments in the same supplied execution-clock epoch without wrapping.
fn elapsed(now: Moment, earlier: Moment) -> Result<Duration, PanelClockError> {
    now.checked_duration_since(earlier).map_err(|_| PanelClockError::Regressed)
}

/// An invalid monotonic observation or unrepresentable deadline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PanelClockError {
    /// A caller supplied a time earlier than the previous observation.
    Regressed,
    /// A required deadline cannot be represented without wrapping.
    DeadlineOverflow,
}

impl fmt::Display for PanelClockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Regressed => formatter.write_str("Panel monotonic clock regressed"),
            Self::DeadlineOverflow => formatter.write_str("Panel deadline cannot be represented"),
        }
    }
}

impl Error for PanelClockError {}
