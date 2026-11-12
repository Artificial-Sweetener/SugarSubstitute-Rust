// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Schema-three selection records preserve the last accepted generation during trials.
//!
//! Decoding ignores fields it does not know, so a later release may add fields without a new
//! schema number; the number changes only when an older reader would misread the record.
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Immutable directory identity and exact verified package used to populate it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Generation {
    /// Opaque directory key; native storage separately verifies resolved ownership.
    pub id: GenerationId,
    /// Package identity used for integrity binding and failed-candidate suppression.
    pub package: PackageDigest,
    /// Signed offer label this generation was prepared from; presentation only, never ordering
    /// or trust authority.
    pub release: SignedRelease,
}

/// Version label and release sequence copied from the authenticated offer at preparation.
/// Decoding validates the same bounds the signed descriptor admits.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SignedReleaseWire")]
pub struct SignedRelease {
    version: String,
    sequence: u64,
}
#[derive(Deserialize)]
struct SignedReleaseWire {
    version: String,
    sequence: u64,
}
impl SignedRelease {
    /// Longest version label a signed descriptor admits, in bytes.
    pub const MAX_VERSION_BYTES: usize = 64;

    /// Copy a signed offer's `version()` and `sequence()`.
    /// # Errors
    /// Returns `Rejection::Identity` for an empty, oversized or noncanonical label or a zero
    /// sequence, which no authenticated offer carries.
    pub fn new(version: String, sequence: u64) -> Result<Self, Rejection> {
        if sequence == 0
            || version.is_empty()
            || version.len() > Self::MAX_VERSION_BYTES
            || !version.bytes().all(|b| b.is_ascii_alphanumeric() || b".-+".contains(&b))
        {
            return Err(Rejection::Identity);
        }
        Ok(Self { version, sequence })
    }
    /// User-facing version label of the signed offer.
    pub fn version(&self) -> &str {
        &self.version
    }
    /// Signed release sequence; the only ordering between releases.
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
}
impl TryFrom<SignedReleaseWire> for SignedRelease {
    type Error = Rejection;
    fn try_from(wire: SignedReleaseWire) -> Result<Self, Self::Error> {
        Self::new(wire.version, wire.sequence)
    }
}

/// Correlate readiness or rejection to an exact persisted launch attempt.
/// This record is correlation data; native authenticated transport supplies authority, and IPC
/// compares the known fields with the protected record, never with fields it cannot read. Like
/// the rest of the record it ignores fields a later release adds, so an older fallback engine
/// still reads a trial in progress when the candidate's own engine cannot start.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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
#[serde(tag = "phase", rename_all = "snake_case")]
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

/// Bad trial verdicts recorded against one package. Two unexpired strikes withhold the package
/// from automatic preparation; a strike older than [`STRIKE_LIFETIME_SECONDS`] no longer counts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Strike {
    /// Package whose trial was judged bad.
    pub package: PackageDigest,
    /// Signed release sequence the package was prepared from, so an offer of the same release
    /// can be withheld before anything is downloaded.
    pub sequence: u64,
    /// Bad verdicts within one lifetime of each other, at least one.
    pub count: u8,
    /// UTC whole seconds since the Unix epoch of the latest bad verdict, as the adapter's
    /// clock reported it.
    pub at: u64,
}
impl Strike {
    /// Whether this strike still withholds its package at `now` (UTC Unix seconds).
    /// A clock that moved before the strike treats the strike as expired: a wrong clock can at
    /// worst give the release one more trial, never withhold it indefinitely.
    pub const fn withholds(&self, now: u64) -> bool {
        self.count >= QUARANTINE_STRIKES && self.live(now)
    }
    pub(super) const fn live(&self, now: u64) -> bool {
        now >= self.at && now - self.at < STRIKE_LIFETIME_SECONDS
    }
}

/// Consecutive trials of one package whose ending nobody observed: neither the coordinator nor
/// the custody witness settled them, and the next launch found the trial record abandoned.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Unwitnessed {
    /// Package of the abandoned trials.
    pub package: PackageDigest,
    /// Abandoned trials in a row, below [`UNWITNESSED_LIMIT`].
    pub count: u8,
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
    pub(super) strikes: Vec<Strike>,
    pub(super) unwitnessed: Option<Unwitnessed>,
    pub(super) rollback: Option<Generation>,
    pub(super) repair: Option<Generation>,
}
#[derive(Deserialize)]
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
    strikes: Vec<Strike>,
    #[serde(deserialize_with = "required_option")]
    unwitnessed: Option<Unwitnessed>,
    #[serde(deserialize_with = "required_option")]
    rollback: Option<Generation>,
    #[serde(deserialize_with = "required_option")]
    repair: Option<Generation>,
}
/// Read only the schema number first, so a record from another release is reported as an
/// unsupported schema rather than as damaged bytes, whatever its other fields look like.
#[derive(Deserialize)]
struct SchemaProbe {
    schema_version: u32,
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
            strikes: Vec::new(),
            unwitnessed: None,
            rollback: None,
            repair: None,
        }
    }
    /// Decode bounded JSON and validate all references; never modifies the source bytes.
    /// # Errors
    /// Returns `Schema` for any other schema number, checked before the other fields are read.
    /// Rejects unknown fields, invalid identities and contradictory selection state.
    pub fn decode(bytes: &[u8]) -> Result<Self, Rejection> {
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(Rejection::Capacity);
        }
        let version: SchemaProbe =
            serde_json::from_slice(bytes).map_err(|_| Rejection::Encoding)?;
        if version.schema_version != SCHEMA_VERSION {
            return Err(Rejection::Schema);
        }
        let wire: Wire = serde_json::from_slice(bytes).map_err(|_| Rejection::Encoding)?;
        let record = Self {
            schema_version: wire.schema_version,
            installation: wire.installation,
            revision: wire.revision,
            accepted: wire.accepted,
            retained: wire.retained,
            candidate: wire.candidate,
            strikes: wire.strikes,
            unwitnessed: wire.unwitnessed,
            rollback: wire.rollback,
            repair: wire.repair,
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
    /// Bad trial verdicts per package; explicit repair may request another trial regardless.
    pub fn strikes(&self) -> &[Strike] {
        &self.strikes
    }
    /// Consecutive abandoned trials of the current package, if any.
    pub fn unwitnessed(&self) -> Option<&Unwitnessed> {
        self.unwitnessed.as_ref()
    }
    /// Whether automatic preparation withholds `package` at `now` (UTC Unix seconds). Check this
    /// before downloading an offer; preparation enforces it again when reserving a generation.
    pub fn withheld(&self, package: &PackageDigest, now: u64) -> bool {
        self.strikes.iter().any(|strike| &strike.package == package && strike.withholds(now))
    }
    /// Whether automatic preparation withholds the signed release `sequence` at `now` (UTC Unix
    /// seconds). An offer names its release before its package is downloaded, so discovery checks
    /// this to avoid downloading a release that reservation would refuse.
    pub fn withheld_release(&self, sequence: u64, now: u64) -> bool {
        self.strikes.iter().any(|strike| strike.sequence == sequence && strike.withholds(now))
    }
    /// Newer generation whose trial was judged bad, leaving `accepted` running in its place.
    ///
    /// Only a bad trial verdict sets this; a trial that ends without a verdict returns to
    /// `Candidate::Prepared` instead. Accepting any later trial clears it.
    pub fn rollback(&self) -> Option<&Generation> {
        self.rollback.as_ref()
    }
    /// Newer accepted generation whose files were found damaged, so the retained generation runs
    /// in its place until preparation downloads and verifies that release again. This is not a
    /// bad verdict: the release did nothing wrong and is never withheld for it.
    pub fn repair(&self) -> Option<&Generation> {
        self.repair.as_ref()
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
        if self.strikes.len() > MAX_STRUCK_PACKAGES {
            return Err(Rejection::Capacity);
        }
        if self.strikes.iter().map(|strike| &strike.package).collect::<BTreeSet<_>>().len()
            != self.strikes.len()
            || self.strikes.iter().any(|strike| strike.count == 0)
            || self.unwitnessed.as_ref().is_some_and(|abandoned| {
                abandoned.count == 0 || abandoned.count >= UNWITNESSED_LIMIT
            })
            || (self.retained.is_some() && self.accepted.is_none())
            || (self.revision == 0
                && (self.accepted.is_some()
                    || self.candidate.is_some()
                    || !self.strikes.is_empty()
                    || self.unwitnessed.is_some()
                    || self.rollback.is_some()
                    || self.repair.is_some()))
        {
            return Err(Rejection::Invariant);
        }
        let generations = self
            .accepted
            .iter()
            .chain(self.retained.iter())
            .chain(self.repair.iter())
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
