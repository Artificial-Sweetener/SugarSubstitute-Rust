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
        ExitAttribution, IncidentRevision, IncidentSummary, IncidentUtc, ReportAttempt,
        ReportDisposition, ReportFailure, RetentionFacts, SummaryFacts,
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
