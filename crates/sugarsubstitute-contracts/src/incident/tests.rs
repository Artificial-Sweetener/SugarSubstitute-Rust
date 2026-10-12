// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise public bounded facts without storage, native processes or scheduler effects.
use super::*;
use crate::lifecycle::{
    CoordinatorEpoch, CoordinatorKey, DesktopExitKind, InstanceId, NamespaceId, RunGeneration,
    RunKey,
};

mod eviction;
mod evidence;
mod helper;
mod identity;
mod report;
mod summary;
mod symbols;

fn owner() -> Result<CoordinatorKey, Box<dyn std::error::Error>> {
    Ok(CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    })
}
fn desktop() -> Result<IncidentKey, Box<dyn std::error::Error>> {
    Ok(IncidentKey::new(IncidentSubject::Desktop(RunKey {
        owner: owner()?,
        instance: InstanceId::new(3)?,
        generation: RunGeneration::new(4)?,
    })))
}
fn seed(key: IncidentKey) -> Result<IncidentSeed, IncidentError> {
    IncidentSeed::new(
        key,
        IncidentRevision::new(1)?,
        BuildIdentity::new("release-1")?,
        IncidentUtc::from_unix_seconds(100)?,
        IncidentStage::Startup,
    )
}
fn facts(
    exit: ExitAttribution,
    failure: FailureEvidence,
    capture: CaptureEvidence,
    symbols: SymbolOutcome,
) -> Result<SummaryFacts, IncidentError> {
    Ok(SummaryFacts::new(
        RetentionFacts::new(IncidentUtc::from_unix_seconds(200)?, None)?,
        exit,
        failure,
        capture,
        PayloadDisposition::Unavailable,
        ReportDisposition::new(0, None)?,
        symbols,
    ))
}
fn frame(build: &str, dump: DumpId) -> Result<FailingFrame, IncidentError> {
    Ok(FailingFrame::new(
        dump,
        ReleaseSymbols::new(
            BuildIdentity::new(build)?,
            FileLabel::new("sugarsubstitute.exe")?,
            SymbolArchitecture::X86_64,
            PdbIdentity::new([3; 16], 1)?,
            [4; 32],
            [5; 32],
        ),
        0x1010,
        0x10,
        SymbolName::new("sugarsubstitute::fault_fixture")?,
        Some(SourceLine::new(FileLabel::new("fixture.rs")?, 17)?),
    ))
}
