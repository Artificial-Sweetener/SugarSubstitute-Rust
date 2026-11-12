// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve immutable summary attribution and explicit acknowledgement independently of storage.
use super::{
    IncidentStoreError,
    codec_evidence::{CaptureRecord, FailureRecord, PayloadRecord},
    codec_seed::SeedRecord,
    codec_symbols::SymbolsRecord,
};
use serde::{Deserialize, Serialize};
use sugarsubstitute_contracts::{
    incident::{
        ExitAttribution, IncidentRevision, IncidentSummary, IncidentUtc, PayloadDisposition,
        ReportAttempt, ReportDisposition, ReportFailure, RetentionFacts, SummaryFacts,
    },
    lifecycle::DesktopExitKind,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SummaryRecord {
    seed: SeedRecord,
    revision: u64,
    occurred: u64,
    #[serde(deserialize_with = "super::codec::required_option")]
    acknowledged: Option<u64>,
    exit: ExitRecord,
    failure: FailureRecord,
    capture: CaptureRecord,
    payload: PayloadRecord,
    reporting: ReportingRecord,
    symbols: SymbolsRecord,
}
mod exit;
use exit::{ExitKindRecord, ExitRecord};
mod reporting;
use reporting::{AttemptRecord, ReportFailureRecord, ReportingRecord};
impl SummaryRecord {
    pub(super) fn into_summary(self) -> Result<IncidentSummary, IncidentStoreError> {
        let retention = RetentionFacts::new(
            super::codec::utc(self.occurred)?,
            self.acknowledged.map(super::codec::utc).transpose()?,
        )?;
        let exit = match self.exit {
            ExitRecord::Supervisor { status } => ExitAttribution::Supervisor { status },
            ExitRecord::CaptureOnly => ExitAttribution::CaptureOnly,
            ExitRecord::Desktop { kind, status } => ExitAttribution::Desktop {
                kind: match kind {
                    ExitKindRecord::Normal => DesktopExitKind::Normal,
                    ExitKindRecord::StartupCancelled => DesktopExitKind::StartupCancelled,
                    ExitKindRecord::StartupFailed => DesktopExitKind::StartupFailed,
                    ExitKindRecord::Unexpected => DesktopExitKind::Unexpected,
                },
                status,
            },
        };
        let reporting = ReportDisposition::new(
            self.reporting.attempts,
            self.reporting.last.map(|last| match last {
                AttemptRecord::Presented {} => ReportAttempt::Presented,
                AttemptRecord::Failed { cause } => ReportAttempt::Failed(match cause {
                    ReportFailureRecord::Launch => ReportFailure::Launch,
                    ReportFailureRecord::Load => ReportFailure::Load,
                    ReportFailureRecord::Presentation => ReportFailure::Presentation,
                    ReportFailureRecord::UnexpectedExit => ReportFailure::UnexpectedExit,
                    ReportFailureRecord::Acknowledgement => ReportFailure::Acknowledgement,
                    ReportFailureRecord::Export => ReportFailure::Export,
                    ReportFailureRecord::Retirement => ReportFailure::Retirement,
                }),
            }),
        )?;
        IncidentSummary::new(
            self.seed.into_seed()?,
            IncidentRevision::new(self.revision)?,
            SummaryFacts::new(
                retention,
                exit,
                self.failure.into_facts(),
                self.capture.into_facts()?,
                self.payload.into_facts()?,
                reporting,
                self.symbols.into_facts()?,
            ),
        )
        .map_err(Into::into)
    }
}
impl From<&IncidentSummary> for SummaryRecord {
    fn from(summary: &IncidentSummary) -> Self {
        let facts = summary.facts();
        Self {
            seed: summary.seed().into(),
            revision: summary.revision().get(),
            occurred: facts.retention().occurred().unix_seconds(),
            acknowledged: facts.retention().acknowledged().map(IncidentUtc::unix_seconds),
            exit: match facts.exit() {
                ExitAttribution::Supervisor { status } => ExitRecord::Supervisor { status },
                ExitAttribution::CaptureOnly => ExitRecord::CaptureOnly,
                ExitAttribution::Desktop { kind, status } => ExitRecord::Desktop {
                    status,
                    kind: match kind {
                        DesktopExitKind::Normal => ExitKindRecord::Normal,
                        DesktopExitKind::StartupCancelled => ExitKindRecord::StartupCancelled,
                        DesktopExitKind::StartupFailed => ExitKindRecord::StartupFailed,
                        DesktopExitKind::Unexpected => ExitKindRecord::Unexpected,
                    },
                },
            },
            failure: facts.failure().into(),
            capture: facts.capture().into(),
            payload: facts.payload().into(),
            reporting: ReportingRecord {
                attempts: facts.reporting().attempts(),
                last: facts.reporting().last().map(|last| match last {
                    ReportAttempt::Presented => AttemptRecord::Presented {},
                    ReportAttempt::Failed(cause) => AttemptRecord::Failed {
                        cause: match cause {
                            ReportFailure::Launch => ReportFailureRecord::Launch,
                            ReportFailure::Load => ReportFailureRecord::Load,
                            ReportFailure::Presentation => ReportFailureRecord::Presentation,
                            ReportFailure::UnexpectedExit => ReportFailureRecord::UnexpectedExit,
                            ReportFailure::Acknowledgement => ReportFailureRecord::Acknowledgement,
                            ReportFailure::Export => ReportFailureRecord::Export,
                            ReportFailure::Retirement => ReportFailureRecord::Retirement,
                        },
                    },
                }),
            },
            symbols: facts.symbols().into(),
        }
    }
}

/// Persist one summary independently of its immutable attachment publication generation.
/// These matching facts grant no native custody, payload availability or activation proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredIncidentSummary {
    summary: IncidentSummary,
    payload_revision: Option<IncidentRevision>,
}
impl StoredIncidentSummary {
    /// Bind retained attachments to an independently preserved publication revision.
    /// # Errors
    /// Retained payloads require a generation between the seed and summary revisions;
    /// all other payload dispositions require no generation.
    pub fn new(
        summary: IncidentSummary,
        payload_revision: Option<IncidentRevision>,
    ) -> Result<Self, IncidentStoreError> {
        let retained = matches!(summary.facts().payload(), PayloadDisposition::Retained(_));
        if retained != payload_revision.is_some()
            || payload_revision.is_some_and(|revision| {
                revision < summary.seed().revision() || revision > summary.revision()
            })
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        Ok(Self { summary, payload_revision })
    }
    /// Borrow domain facts without exposing native storage identities.
    pub const fn summary(&self) -> &IncidentSummary {
        &self.summary
    }
    /// Observe the independently immutable payload publication generation.
    pub const fn payload_revision(&self) -> Option<IncidentRevision> {
        self.payload_revision
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredRecord {
    summary: SummaryRecord,
    #[serde(deserialize_with = "super::codec::required_option")]
    payload_revision: Option<u64>,
}
/// Encode canonical schema-three whole summary bytes within 64KiB.
/// # Errors
/// Preserves encoding causes and rejects oversized complete records.
pub fn encode_summary(summary: &StoredIncidentSummary) -> Result<Vec<u8>, IncidentStoreError> {
    super::codec_encode::encode(3, StoredRecord::from(summary), super::codec::STATE_BOUND)
}
impl From<&StoredIncidentSummary> for StoredRecord {
    fn from(summary: &StoredIncidentSummary) -> Self {
        Self {
            summary: summary.summary().into(),
            payload_revision: summary.payload_revision().map(IncidentRevision::get),
        }
    }
}
#[cfg(windows)]
pub(super) fn write_summary<W: std::io::Write>(
    summary: &StoredIncidentSummary,
    output: W,
) -> Result<(), IncidentStoreError> {
    super::codec_encode::write(3, &StoredRecord::from(summary), output)
}
/// Decode strict schema-two/three summaries; capture-only attribution requires schema three.
/// Reading legacy facts grants no native format activation; durable conversion is explicit.
/// # Errors
/// Rejects unknown fields/versions, inconsistent payload references and malformed bytes.
pub fn decode_summary(bytes: &[u8]) -> Result<StoredIncidentSummary, IncidentStoreError> {
    if bytes.len() > super::codec::STATE_BOUND {
        return Err(IncidentStoreError::RecordBound);
    }
    let record: super::codec::Record<StoredRecord> = super::codec_parse::parse(bytes)?;
    if !matches!(record.schema_version, 2 | 3) {
        return Err(IncidentStoreError::UnsupportedSchema(record.schema_version));
    }
    if record.schema_version == 2 && matches!(&record.content.summary.exit, ExitRecord::CaptureOnly)
    {
        return Err(IncidentStoreError::RecoveryConflict);
    }
    StoredIncidentSummary::new(
        record.content.summary.into_summary()?,
        record.content.payload_revision.map(IncidentRevision::new).transpose()?,
    )
}
/// Convert strict legacy-one bytes to canonical schema two without modifying the input.
/// Valid schema-two input is decoded and canonically re-encoded; this grants no durability.
/// # Errors
/// Rejects malformed/unknown schemas and fields, invalid facts and oversized records,
/// preserving the original decoder cause. Legacy retained references derive only from
/// that legacy summary's own revision.
pub fn migrate_summary_v1(bytes: &[u8]) -> Result<Vec<u8>, IncidentStoreError> {
    if bytes.len() > super::codec::STATE_BOUND {
        return Err(IncidentStoreError::RecordBound);
    }
    let envelope: super::codec::Record<serde::de::IgnoredAny> = super::codec_parse::parse(bytes)?;
    match envelope.schema_version {
        1 => {
            let record = super::codec::decode::<SummaryRecord>(bytes, super::codec::STATE_BOUND)?;
            if matches!(&record.exit, ExitRecord::CaptureOnly) {
                return Err(IncidentStoreError::RecoveryConflict);
            }
            let summary = record.into_summary()?;
            let revision = matches!(summary.facts().payload(), PayloadDisposition::Retained(_))
                .then_some(summary.revision());
            encode_legacy_two(&StoredIncidentSummary::new(summary, revision)?)
        }
        2 => encode_legacy_two(&decode_summary(bytes)?),
        version => Err(IncidentStoreError::UnsupportedSchema(version)),
    }
}

/// Convert strict schema-two bytes to canonical schema three without modifying their original bytes.
/// Current schema-three input is validated and canonically re-encoded for idempotence.
/// # Errors
/// Rejects schema one/unknown versions, capture-only in schema two, malformed or oversized bytes.
pub fn migrate_summary_v2(bytes: &[u8]) -> Result<Vec<u8>, IncidentStoreError> {
    encode_summary(&decode_summary(bytes)?)
}
/// Preserve the established public one-to-two conversion result exactly.
fn encode_legacy_two(summary: &StoredIncidentSummary) -> Result<Vec<u8>, IncidentStoreError> {
    if matches!(summary.summary().facts().exit(), ExitAttribution::CaptureOnly) {
        return Err(IncidentStoreError::RecoveryConflict);
    }
    super::codec_encode::encode(2, StoredRecord::from(summary), super::codec::STATE_BOUND)
}

/// Build at most one validated contiguous candidate; current bytes undergo strict full decoding.
/// The caller retains its real decode loan until all temporary domain/buffer allocations drop.
#[cfg(windows)]
pub(super) fn migration_candidate(
    bytes: &[u8],
) -> Result<
    Option<(
        sugarsubstitute_contracts::incident::SummaryMigrationStep,
        StoredIncidentSummary,
        Vec<u8>,
    )>,
    IncidentStoreError,
> {
    use sugarsubstitute_contracts::incident::SummaryMigrationStep as Step;
    if bytes.len() > super::codec::STATE_BOUND {
        return Err(IncidentStoreError::RecordBound);
    }
    let record: super::codec::Record<serde::de::IgnoredAny> = super::codec_parse::parse(bytes)?;
    let (step, candidate) = match record.schema_version {
        1 => (Step::OneToTwo, migrate_summary_v1(bytes)?),
        2 => (Step::TwoToThree, migrate_summary_v2(bytes)?),
        3 => {
            decode_summary(bytes)?;
            return Ok(None);
        }
        version => return Err(IncidentStoreError::UnsupportedSchema(version)),
    };
    let summary = decode_summary(&candidate)?;
    Ok(Some((step, summary, candidate)))
}

/// Ordinary repository reads/mutations require current schema after checked durable migration.
/// Public legacy reading remains separate and cannot silently activate a successor format.
#[cfg(windows)]
pub(super) fn decode_current_summary(
    bytes: &[u8],
) -> Result<StoredIncidentSummary, IncidentStoreError> {
    let stored = decode_summary(bytes)?;
    let record: super::codec::Record<serde::de::IgnoredAny> = super::codec_parse::parse(bytes)?;
    if record.schema_version != 3 {
        return Err(IncidentStoreError::UnsupportedSchema(record.schema_version));
    }
    Ok(stored)
}
