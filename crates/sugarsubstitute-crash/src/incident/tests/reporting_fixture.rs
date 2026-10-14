// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Build inert report scenarios and seed only the fake external filesystem.
use super::*;
use sugarsubstitute_supervision::diagnostic_storage::{fixture::*, *};
type TestError = Box<dyn Error + Send + Sync>;
pub(in crate::incident) type TestResult = Result<(), TestError>;

pub(super) fn original() -> Result<IncidentSummary, TestError> {
    super::summary().map_err(|cause| std::io::Error::other(cause.to_string()).into())
}
pub(in crate::incident) fn binding(
    summary: &IncidentSummary,
) -> Result<ReportBinding, IncidentError> {
    Ok(ReportBinding::new(summary.seed().key(), summary.revision(), ReportSessionId::new(3)?))
}
pub(in crate::incident) fn installed(
    fixture: &DiagnosticStorageFixture,
    summary: IncidentSummary,
    payload_revision: Option<IncidentRevision>,
) -> TestResult {
    fixture.put(
        DiagnosticObject::Summary(summary.seed().key()),
        &encode_summary(&StoredIncidentSummary::new(summary, payload_revision)?)?,
    )?;
    Ok(())
}
pub(in crate::incident) fn retained(bytes: &[u8]) -> Result<IncidentSummary, TestError> {
    let current = original()?;
    let facts = current.facts();
    Ok(IncidentSummary::new(
        current.seed().clone(),
        current.revision(),
        SummaryFacts::new(
            facts.retention(),
            facts.exit(),
            facts.failure(),
            facts.capture(),
            PayloadDisposition::Retained(AttachmentInventory::new(&[AttachmentFacts::new(
                AttachmentKind::Fault,
                bytes.len() as u64,
                bytes.len() as u64,
                0,
                super::super::codec_ledger::digest(bytes),
            )?])?),
            facts.reporting(),
            facts.symbols().clone(),
        ),
    )?)
}
