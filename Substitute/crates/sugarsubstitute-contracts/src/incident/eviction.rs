// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate cumulative store ledger observations independently of immutable summaries.
use super::{IncidentError, IncidentKey, IncidentUtc};
use crate::lifecycle::NamespaceId;

/// Exclusive summary category supplied by the repository at eviction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SummaryEvictionKind {
    /// Unacknowledged summary was evicted.
    Pending,
    /// Acknowledged summary was evicted.
    Acknowledged,
    /// Quarantined summary was evicted, without double counting another category.
    Quarantined,
}
/// Bounded last-eviction observation; construction does not prove durable deletion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SummaryEviction {
    incident: IncidentKey,
    at: IncidentUtc,
    kind: SummaryEvictionKind,
}
impl SummaryEviction {
    /// Retain the full incident identity, actual observed UTC and exclusive category.
    pub const fn new(incident: IncidentKey, at: IncidentUtc, kind: SummaryEvictionKind) -> Self {
        Self { incident, at, kind }
    }
    /// Return full desktop/supervisor identity, including its coordinator namespace.
    pub const fn incident(self) -> IncidentKey {
        self.incident
    }
    /// Return supplied whole UTC seconds, without sampling or inventing a clock value.
    pub const fn at(self) -> IncidentUtc {
        self.at
    }
    /// Return the exclusive evicted summary category.
    pub const fn kind(self) -> SummaryEvictionKind {
        self.kind
    }
}
/// Cumulative namespace-local store facts; the repository owns durable truth and deduplication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SummaryEvictions {
    namespace: NamespaceId,
    pending: u64,
    acknowledged: u64,
    quarantined: u64,
    last: Option<SummaryEviction>,
}
impl SummaryEvictions {
    /// Validate externally supplied ledger observations without changing existing summaries.
    /// # Errors
    /// Rejects zero counts with a last observation, nonzero counts without one, a zero
    /// last category count, or a last incident belonging to another namespace.
    pub fn new(
        namespace: NamespaceId,
        pending: u64,
        acknowledged: u64,
        quarantined: u64,
        last: Option<SummaryEviction>,
    ) -> Result<Self, IncidentError> {
        let counts_are_zero = pending == 0 && acknowledged == 0 && quarantined == 0;
        if counts_are_zero != last.is_none() {
            return Err(IncidentError::MismatchedFacts);
        }
        if let Some(eviction) = last {
            let category_count = match eviction.kind {
                SummaryEvictionKind::Pending => pending,
                SummaryEvictionKind::Acknowledged => acknowledged,
                SummaryEvictionKind::Quarantined => quarantined,
            };
            if category_count == 0
                || eviction.incident.subject().coordinator().namespace != namespace
            {
                return Err(IncidentError::MismatchedFacts);
            }
        }
        Ok(Self { namespace, pending, acknowledged, quarantined, last })
    }
    /// Produce updated arithmetic facts for one externally observed eviction.
    ///
    /// Equal UTC seconds are valid. This neither deletes data nor establishes durable
    /// publication; repository consumers must commit and deduplicate their own observations.
    /// # Errors
    /// Rejects another namespace, regressing UTC or category overflow. The original
    /// value remains unchanged for every result.
    pub fn record(&self, eviction: SummaryEviction) -> Result<Self, IncidentError> {
        if eviction.incident.subject().coordinator().namespace != self.namespace {
            return Err(IncidentError::MismatchedFacts);
        }
        if self.last.is_some_and(|last| eviction.at < last.at) {
            return Err(IncidentError::InvalidTime);
        }
        let mut updated = *self;
        let count = match eviction.kind {
            SummaryEvictionKind::Pending => &mut updated.pending,
            SummaryEvictionKind::Acknowledged => &mut updated.acknowledged,
            SummaryEvictionKind::Quarantined => &mut updated.quarantined,
        };
        *count = count.checked_add(1).ok_or(IncidentError::EvictionCountExhausted)?;
        updated.last = Some(eviction);
        Ok(updated)
    }
    /// Return the authoritative store namespace for these cumulative observations.
    pub const fn namespace(self) -> NamespaceId {
        self.namespace
    }
    /// Return cumulative pending summary evictions, independent of reporter attempts.
    pub const fn pending(self) -> u64 {
        self.pending
    }
    /// Return cumulative acknowledged summary evictions.
    pub const fn acknowledged(self) -> u64 {
        self.acknowledged
    }
    /// Return cumulative quarantined summary evictions.
    pub const fn quarantined(self) -> u64 {
        self.quarantined
    }
    /// Return the full last observation, without asserting durable eviction.
    pub const fn last(self) -> Option<SummaryEviction> {
        self.last
    }
}
