// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve package publication floors separately from healthy application selection.
use crate::{Error, VerifiedOffer};
use serde::{Deserialize, Serialize};
use sugarsubstitute_contracts::update::{
    Admission, Channel, Component, Platform, Rejection, Request,
};

/// Bounded authoritative history for both components, two channels and six platforms.
pub const MAX_RELEASE_HISTORY_BYTES: usize = 16 * 1024;
const MAX_ROUTES: usize = 24;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Floor {
    channel: Channel,
    component: Component,
    platform: Platform,
    sequence: u64,
    /// Absent only for a fresh namespace's start whose package this installation never recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    digest: Option<[u8; 32]>,
}
impl Floor {
    fn matches(&self, request: &Request) -> bool {
        self.channel == request.channel
            && self.component == request.component
            && self.platform == request.platform
    }
    fn same_route(&self, other: &Self) -> bool {
        self.channel == other.channel
            && self.component == other.component
            && self.platform == other.platform
    }
}

/// Highest authorised sequence and its package digest for one route.
#[derive(Clone, Copy)]
pub(crate) struct RouteFloor {
    pub(crate) sequence: u64,
    pub(crate) digest: Option<[u8; 32]>,
}
impl RouteFloor {
    /// Whether a package at `sequence` with `digest` falls below this floor or reuses its
    /// sequence for a different package.
    pub(crate) fn refuses(&self, sequence: u64, digest: [u8; 32]) -> bool {
        sequence < self.sequence
            || (sequence == self.sequence && self.digest.is_some_and(|floor| floor != digest))
    }
}

/// Where one route's history starts in a fresh trust namespace: nothing below `sequence` is
/// admitted, and when `digest` is known the start sequence admits only that package.
#[derive(Clone, Copy, Debug)]
pub struct RouteStart {
    /// Release channel of the route.
    pub channel: Channel,
    /// Component the route delivers.
    pub component: Component,
    /// Package platform of the route.
    pub platform: Platform,
    /// Lowest sequence the route admits again; zero leaves the route without a floor.
    pub sequence: u64,
    /// Package the installation recorded at `sequence`, when it recorded one.
    pub digest: Option<[u8; 32]>,
}

/// Highest authorized package sequence for each route, including failed activation attempts.
/// Native storage must bind this record to its installation/repository, serialize writers,
/// and durably save a successor before publishing its package. This does not mark a version
/// healthy or change launch selection. Local rollback to an already accepted generation is
/// independent of admitting a downloaded older package.
#[derive(Clone, Serialize)]
pub struct ReleaseHistory {
    schema_version: u32,
    floors: Vec<Floor>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    schema_version: u32,
    floors: Vec<Floor>,
}
impl ReleaseHistory {
    /// Create history only while explicitly initializing a fresh native trust namespace.
    /// Missing or damaged history must never be replaced by this empty state.
    pub fn empty() -> Self {
        Self { schema_version: 1, floors: Vec::new() }
    }

    /// Restore bounded protected bytes; missing fields and unknown schemas are rejected.
    /// # Errors
    /// Preserves the source on invalid encoding, duplicate routes, zero sequences or excess size.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_RELEASE_HISTORY_BYTES {
            return Err(Error::Bound);
        }
        let wire: Wire = serde_json::from_slice(bytes)?;
        let value = Self { schema_version: wire.schema_version, floors: wire.floors };
        if value.schema_version != 1 || value.floors.len() > MAX_ROUTES {
            return Err(Error::State);
        }
        for (index, floor) in value.floors.iter().enumerate() {
            if floor.sequence == 0
                || value.floors[..index].iter().any(|other| floor.same_route(other))
            {
                return Err(Error::State);
            }
        }
        Ok(value)
    }

    /// Encode complete history for atomic native publication.
    /// # Errors
    /// Returns serialization or capacity failure rather than dropping an older route's floor.
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        let bytes = serde_json::to_vec(self)?;
        if bytes.len() > MAX_RELEASE_HISTORY_BYTES {
            return Err(Error::Bound);
        }
        Ok(bytes)
    }

    /// Check an authenticated offer against durable history as well as the caller's facts.
    /// # Errors
    /// Rejects rollback and sequence reuse even when the caller supplies an empty history.
    pub fn check(&self, request: &Request, offer: &VerifiedOffer) -> Result<(), Error> {
        offer.offer().admission(request).map_err(Error::Offer)?;
        if let Some(floor) = self.floors.iter().find(|floor| floor.matches(request)) {
            if offer.offer().sequence() < floor.sequence {
                return Err(Error::Offer(Rejection::Rollback));
            }
            if offer.offer().sequence() == floor.sequence
                && floor.digest.is_some_and(|digest| digest != offer.offer().package_digest())
            {
                return Err(Error::Offer(Rejection::Identity));
            }
        }
        Ok(())
    }

    /// History for a fresh trust namespace of an installation that already holds releases: each
    /// route starts at its [`RouteStart`], so nothing older than what the installation already
    /// runs or authorised is accepted again. Starts at sequence zero add no floor.
    /// # Errors
    /// Rejects more routes than history holds and the same route twice.
    pub fn starting(starts: &[RouteStart]) -> Result<Self, Error> {
        let mut history = Self::empty();
        for start in starts.iter().filter(|start| start.sequence != 0) {
            let floor = Floor {
                channel: start.channel,
                component: start.component,
                platform: start.platform,
                sequence: start.sequence,
                digest: start.digest,
            };
            if history.floors.len() == MAX_ROUTES
                || history.floors.iter().any(|other| other.same_route(&floor))
            {
                return Err(Error::Bound);
            }
            history.floors.push(floor);
        }
        Ok(history)
    }

    /// Durable floor for `request`'s route, if this installation ever authorised one.
    pub(crate) fn floor(&self, request: &Request) -> Option<RouteFloor> {
        self.floors
            .iter()
            .find(|floor| floor.matches(request))
            .map(|floor| RouteFloor { sequence: floor.sequence, digest: floor.digest })
    }

    /// Propose history after current signature verification and exact archive matching.
    /// The native caller retains trust exclusion through durable storage and publication.
    /// Repeating the same admitted identity is idempotent and does not assert healthy startup.
    /// # Errors
    /// Rejects inadmissible offers, rollback, identity reuse and exhausted route capacity.
    pub fn authorize(&self, request: &Request, offer: &VerifiedOffer) -> Result<Self, Error> {
        self.check(request, offer)?;
        if offer.admission() != Admission::Prepare
            || offer.offer().admission(request).map_err(Error::Offer)? != Admission::Prepare
        {
            return Err(Error::Admission);
        }
        let floor = Floor {
            channel: request.channel,
            component: request.component,
            platform: request.platform,
            sequence: offer.offer().sequence(),
            digest: Some(offer.offer().package_digest()),
        };
        let mut next = self.clone();
        if let Some(previous) = next.floors.iter_mut().find(|floor| floor.matches(request)) {
            *previous = floor;
        } else if next.floors.len() < MAX_ROUTES {
            next.floors.push(floor);
        } else {
            return Err(Error::Bound);
        }
        Ok(next)
    }
}

#[cfg(test)]
mod tests;
