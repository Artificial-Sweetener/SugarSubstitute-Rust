// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise strict persisted records through actual codecs and domain constructors.
mod capture_codec;
mod codec;
mod codec_allocation;
mod codec_parse;
mod codec_writer;
#[cfg(windows)]
mod host_failure;
#[cfg(windows)]
mod initialization;
mod migration;
#[cfg(windows)]
mod migration_recovery;
#[cfg(windows)]
mod migration_runtime;
#[cfg(windows)]
mod migration_runtime_fixture;
#[cfg(windows)]
mod publication;
#[cfg(windows)]
pub(in crate::incident) mod publication_fixture;
#[cfg(windows)]
mod publication_preflight;
#[cfg(windows)]
mod publication_preflight_retention;
#[cfg(windows)]
mod publication_successor;
#[cfg(windows)]
mod recovery;
mod redaction;
#[cfg(windows)]
mod reporting;
#[cfg(windows)]
pub(in crate::incident) mod reporting_fixture;
mod retention;
#[cfg(windows)]
mod retention_fixture;
#[cfg(windows)]
mod retention_policy;
#[cfg(windows)]
mod retention_recovery;
#[cfg(windows)]
mod retention_runtime;
mod summary_schema;
use super::*;
use std::error::Error;
use sugarsubstitute_contracts::{incident::*, lifecycle::*};

fn owner(epoch: u8) -> Result<CoordinatorKey, Box<dyn Error>> {
    Ok(CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([epoch; 16])?,
    })
}
fn seed() -> Result<IncidentSeed, Box<dyn Error>> {
    Ok(IncidentSeed::new(
        IncidentKey::new(IncidentSubject::Desktop(RunKey {
            owner: owner(2)?,
            instance: InstanceId::new(3)?,
            generation: RunGeneration::new(4)?,
        })),
        IncidentRevision::new(1)?,
        BuildIdentity::new("build_1")?,
        IncidentUtc::from_unix_seconds(100)?,
        IncidentStage::Startup,
    )?)
}
fn summary() -> Result<IncidentSummary, Box<dyn Error>> {
    Ok(IncidentSummary::new(
        seed()?,
        IncidentRevision::new(2)?,
        SummaryFacts::new(
            RetentionFacts::new(IncidentUtc::from_unix_seconds(101)?, None)?,
            ExitAttribution::Desktop { kind: DesktopExitKind::Unexpected, status: 0 },
            FailureEvidence::Unexplained,
            CaptureEvidence::Unavailable(CaptureAbsence::Missing),
            PayloadDisposition::Unavailable,
            ReportDisposition::new(0, None)?,
            SymbolOutcome::NotRequested,
        ),
    )?)
}

#[cfg(windows)]
mod allocation_lifecycle;

#[cfg(windows)]
mod allocation_layout;

mod allocation_validation;

#[cfg(windows)]
mod pending;

#[cfg(windows)]
mod portable;
#[cfg(windows)]
mod portable_fixture;
