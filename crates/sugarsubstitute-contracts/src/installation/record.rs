// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Strict version-one selection records preserve the last accepted generation during trials.
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Immutable directory identity and exact verified package used to populate it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Generation {
    /// Opaque directory key; native storage separately verifies resolved ownership.
    pub id: GenerationId,
    /// Package identity used for integrity binding and failed-candidate suppression.
    pub package: PackageDigest,
}

/// Correlate readiness or rejection to an exact persisted launch attempt.
/// This record is correlation data; native authenticated transport supplies authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trial {
    /// Installation whose accepted selection can change.
    pub installation: InstallationId,
    /// Exact staged generation being exercised.
    pub generation: Generation,
    /// Selection revision at which the trial was admitted; prevents late acknowledgement.
    pub revision: u64,
}

/// Preparation never changes the accepted launch selection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case", deny_unknown_fields)]
pub enum Candidate {
    /// Copy/extraction may be incomplete. Native staging can resume or be abandoned.
    Staging {
        /// Reserved generation whose verified package is being prepared.
        generation: Generation,
    },
    /// Complete candidate verified on disk and eligible for a coordinated trial.
    Prepared {
        /// Complete generation admitted by the native verification boundary.
        generation: Generation,
    },
    /// An admitted attempt must be reconciled with live native ownership on restart.
    Trial {
        /// Correlation required by both success and failure observations.
        attempt: Trial,
    },
}
impl Candidate {
    /// Return the immutable generation shared by every preparation/activation phase.
    pub fn generation(&self) -> &Generation {
        match self {
            Self::Staging { generation } | Self::Prepared { generation } => generation,
            Self::Trial { attempt } => &attempt.generation,
        }
    }
}

/// Authoritative accepted/candidate selection. Private fields enforce validation at decode.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InstallationRecord {
    pub(super) schema_version: u32,
    pub(super) installation: InstallationId,
    pub(super) revision: u64,
    pub(super) accepted: Option<Generation>,
    pub(super) retained: Option<Generation>,
    pub(super) candidate: Option<Candidate>,
    pub(super) rejected: Vec<PackageDigest>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    schema_version: u32,
    installation: InstallationId,
    revision: u64,
    #[serde(deserialize_with = "required_option")]
    accepted: Option<Generation>,
    #[serde(deserialize_with = "required_option")]
    retained: Option<Generation>,
    #[serde(deserialize_with = "required_option")]
    candidate: Option<Candidate>,
    rejected: Vec<PackageDigest>,
}
fn required_option<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    input: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(input)
}
impl InstallationRecord {
    /// Start a new owned installation without claiming a healthy application exists.
    pub fn empty(installation: InstallationId) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            installation,
            revision: 0,
            accepted: None,
            retained: None,
            candidate: None,
            rejected: Vec::new(),
        }
    }
    /// Decode bounded JSON and validate all references; never modifies the source bytes.
    /// # Errors
    /// Rejects unknown schemas/fields, invalid identities and contradictory selection state.
    pub fn decode(bytes: &[u8]) -> Result<Self, Rejection> {
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(Rejection::Capacity);
        }
        let wire: Wire = serde_json::from_slice(bytes).map_err(|_| Rejection::Encoding)?;
        let record = Self {
            schema_version: wire.schema_version,
            installation: wire.installation,
            revision: wire.revision,
            accepted: wire.accepted,
            retained: wire.retained,
            candidate: wire.candidate,
            rejected: wire.rejected,
        };
        record.validate()?;
        Ok(record)
    }
    /// Encode a validated proposal for atomic storage publication.
    /// # Errors
    /// Rejects inconsistent state, exhausted encoding capacity or serialization failure.
    pub fn encode(&self) -> Result<Vec<u8>, Rejection> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| Rejection::Encoding)?;
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(Rejection::Capacity);
        }
        Ok(bytes)
    }
    /// Stable identity binding all operations to one installation.
    pub fn installation(&self) -> &InstallationId {
        &self.installation
    }
    /// Compare this revision under the native publication lock before writing a proposal.
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    /// Last health-acknowledged generation, unchanged while staging or exercising a candidate.
    pub fn accepted(&self) -> Option<&Generation> {
        self.accepted.as_ref()
    }
    /// Previous accepted generation retained for explicit compatibility-checked recovery.
    pub fn retained(&self) -> Option<&Generation> {
        self.retained.as_ref()
    }
    /// Current preparation or trial, if any.
    pub fn candidate(&self) -> Option<&Candidate> {
        self.candidate.as_ref()
    }
    /// Recently failed package identities; explicit repair may request another trial.
    pub fn rejected(&self) -> &[PackageDigest] {
        &self.rejected
    }
    /// Determine startup work without guessing whether a recorded process is alive.
    pub fn startup_selection(&self) -> StartupSelection<'_> {
        if let Some(Candidate::Trial { attempt }) = &self.candidate {
            return StartupSelection::ReconcileTrial(attempt);
        }
        match &self.accepted {
            Some(generation) => StartupSelection::Accepted(generation),
            None => StartupSelection::NeedsInstallation,
        }
    }
    pub(super) fn validate(&self) -> Result<(), Rejection> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(Rejection::Schema);
        }
        if self.rejected.len() > MAX_REJECTED_PACKAGES {
            return Err(Rejection::Capacity);
        }
        if self.rejected.iter().collect::<BTreeSet<_>>().len() != self.rejected.len()
            || (self.retained.is_some() && self.accepted.is_none())
            || (self.revision == 0
                && (self.accepted.is_some()
                    || self.candidate.is_some()
                    || !self.rejected.is_empty()))
        {
            return Err(Rejection::Invariant);
        }
        let generations = self
            .accepted
            .iter()
            .chain(self.retained.iter())
            .chain(self.candidate.iter().map(Candidate::generation));
        let mut ids = BTreeSet::new();
        for generation in generations {
            if !ids.insert(&generation.id) {
                return Err(Rejection::Invariant);
            }
        }
        if let Some(Candidate::Trial { attempt }) = &self.candidate
            && (attempt.installation != self.installation || attempt.revision != self.revision)
        {
            return Err(Rejection::Binding);
        }
        Ok(())
    }
}
