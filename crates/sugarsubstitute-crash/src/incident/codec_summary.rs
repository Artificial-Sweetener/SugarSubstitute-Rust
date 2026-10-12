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
#[derive(Serialize, Deserialize)]
#[serde(tag = "subject", rename_all = "snake_case", deny_unknown_fields)]
enum ExitRecord {
    Desktop { kind: ExitKindRecord, status: i64 },
    Supervisor { status: i64 },
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ExitKindRecord {
    Normal,
    StartupCancelled,
    StartupFailed,
    Unexpected,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportingRecord {
    attempts: u64,
    #[serde(deserialize_with = "super::codec::required_option")]
    last: Option<AttemptRecord>,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
enum AttemptRecord {
    Presented {},
    Failed { cause: ReportFailureRecord },
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ReportFailureRecord {
    Launch,
    Load,
    Presentation,
    UnexpectedExit,
    Acknowledgement,
    Export,
    Retirement,
}

impl SummaryRecord {
    pub(super) fn into_summary(self) -> Result<IncidentSummary, IncidentStoreError> {
        let retention = RetentionFacts::new(
            super::codec::utc(self.occurred)?,
            self.acknowledged.map(super::codec::utc).transpose()?,
        )?;
        let exit = match self.exit {
            ExitRecord::Supervisor { status } => ExitAttribution::Supervisor { status },
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
/// Encode canonical schema-two whole summary bytes within 64KiB.
/// # Errors
/// Preserves encoding causes and rejects oversized complete records.
pub fn encode_summary(summary: &StoredIncidentSummary) -> Result<Vec<u8>, IncidentStoreError> {
    let bytes = serde_json::to_vec(&super::codec::Record {
        schema_version: 2,
        content: StoredRecord {
            summary: summary.summary().into(),
            payload_revision: summary.payload_revision().map(IncidentRevision::get),
        },
    })?;
    if bytes.len() > super::codec::STATE_BOUND {
        return Err(IncidentStoreError::RecordBound);
    }
    Ok(bytes)
}
/// Decode only strict schema-two summary bytes; legacy conversion is explicit.
/// # Errors
/// Rejects unknown fields/versions, inconsistent payload references and malformed bytes.
pub fn decode_summary(bytes: &[u8]) -> Result<StoredIncidentSummary, IncidentStoreError> {
    if bytes.len() > super::codec::STATE_BOUND {
        return Err(IncidentStoreError::RecordBound);
    }
    let record: super::codec::Record<StoredRecord> = serde_json::from_slice(bytes)?;
    if record.schema_version != 2 {
        return Err(IncidentStoreError::UnsupportedSchema(record.schema_version));
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
    let envelope: super::codec::Record<serde::de::IgnoredAny> = serde_json::from_slice(bytes)?;
    match envelope.schema_version {
        1 => {
            let summary = super::codec::decode::<SummaryRecord>(bytes, super::codec::STATE_BOUND)?
                .into_summary()?;
            let revision = matches!(summary.facts().payload(), PayloadDisposition::Retained(_))
                .then_some(summary.revision());
            encode_summary(&StoredIncidentSummary::new(summary, revision)?)
        }
        2 => encode_summary(&decode_summary(bytes)?),
        version => Err(IncidentStoreError::UnsupportedSchema(version)),
    }
}
