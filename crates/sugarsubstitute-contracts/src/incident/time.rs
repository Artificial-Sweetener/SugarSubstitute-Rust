// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound UTC retention facts without reading wallclock or persisting monotonic epochs.
use super::IncidentError;

/// UTC whole seconds since Unix epoch, through 9999-12-31T23:59:59Z inclusive.
/// The injecting adapter supplies UTC provenance; this value contains no clock API.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct IncidentUtc(u64);
impl IncidentUtc {
    /// Validate representable UTC seconds without converting a local timestamp.
    /// # Errors
    /// Returns `InvalidTime` for negative seconds or dates after year 9999.
    pub fn from_unix_seconds(seconds: i64) -> Result<Self, IncidentError> {
        let seconds = u64::try_from(seconds).map_err(|_| IncidentError::InvalidTime)?;
        if seconds > 253_402_300_799 {
            return Err(IncidentError::InvalidTime);
        }
        Ok(Self(seconds))
    }
    /// Return whole UTC seconds since 1970-01-01T00:00:00Z.
    pub const fn unix_seconds(self) -> u64 {
        self.0
    }
}
/// Occurrence and optional durable acknowledgement times, distinct from reporter intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetentionFacts {
    occurred: IncidentUtc,
    acknowledged: Option<IncidentUtc>,
}
impl RetentionFacts {
    /// Retain UTC facts only when acknowledgement does not precede occurrence.
    /// # Errors
    /// Returns `InvalidTime` for regressing related timestamps.
    pub fn new(
        occurred: IncidentUtc,
        acknowledged: Option<IncidentUtc>,
    ) -> Result<Self, IncidentError> {
        if acknowledged.is_some_and(|at| at < occurred) {
            return Err(IncidentError::InvalidTime);
        }
        Ok(Self { occurred, acknowledged })
    }
    /// Return the original incident occurrence time.
    pub const fn occurred(self) -> IncidentUtc {
        self.occurred
    }
    /// Return actual durable acknowledgement time, if supplied by the storage owner.
    pub const fn acknowledged(self) -> Option<IncidentUtc> {
        self.acknowledged
    }
}
