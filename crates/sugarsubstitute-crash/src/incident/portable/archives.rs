// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate preserved archive content without reopening historical native transactions.
use super::*;
use crate::incident::{SummaryMigrationJournal, codec_ledger::digest};
use sugarsubstitute_contracts::incident::SummaryMigrationStep;

pub(super) fn validate(
    session: &mut dyn PortableIncidentReadSession,
    stages: &[PortableIncidentEntry],
    archives: &[SummaryMigrationJournal],
    context: &JobContext,
) -> Result<(), PortableIncidentError> {
    let mut seen = Vec::new();
    seen.try_reserve_exact(archives.len()).map_err(PortableIncidentError::allocation)?;
    seen.resize(archives.len(), false);
    for entry in stages.iter().copied() {
        let bytes = source::record(session, entry, 65536, context)?;
        let hash = digest(&bytes);
        let mut matched = None;
        for (index, archive) in archives.iter().enumerate() {
            if archive.original_stage().bytes() == entry.bytes
                && archive.original_stage().sha256() == hash
            {
                if matched.is_some() {
                    return Err(source::rejected(PortableIncidentFailureCode::Duplicate));
                }
                matched = Some(index);
            }
        }
        let index =
            matched.ok_or_else(|| source::rejected(PortableIncidentFailureCode::Uncommitted))?;
        if seen[index] {
            return Err(source::rejected(PortableIncidentFailureCode::Duplicate));
        }
        validate_original(&bytes, &archives[index])?;
        seen[index] = true;
    }
    if seen.iter().any(|matched| !matched) {
        return Err(source::rejected(PortableIncidentFailureCode::MissingContent));
    }
    Ok(())
}
fn validate_original(
    bytes: &[u8],
    journal: &SummaryMigrationJournal,
) -> Result<(), PortableIncidentError> {
    let record: crate::incident::codec::Record<serde::de::IgnoredAny> =
        crate::incident::codec_parse::parse(bytes)
            .map_err(|cause| PortableIncidentError::codec(cause.into()))?;
    if record.schema_version != journal.step().source_schema() {
        return Err(PortableIncidentError::codec(IncidentStoreError::UnsupportedSchema(
            record.schema_version,
        )));
    }
    let converted = match journal.step() {
        SummaryMigrationStep::OneToTwo => crate::incident::migrate_summary_v1(bytes),
        SummaryMigrationStep::TwoToThree => crate::incident::migrate_summary_v2(bytes),
    }
    .map_err(PortableIncidentError::codec)?;
    if converted.len() as u64 != journal.candidate_stage().bytes()
        || digest(&converted) != journal.candidate_stage().sha256()
    {
        return Err(source::rejected(PortableIncidentFailureCode::WrongDigest));
    }
    let stored =
        crate::incident::decode_summary(&converted).map_err(PortableIncidentError::codec)?;
    if stored.summary().seed().key() != journal.incident()
        || stored.summary().revision() != journal.revision()
        || stored.payload_revision() != journal.payload_revision()
    {
        return Err(source::rejected(PortableIncidentFailureCode::ChangedSource));
    }
    // Old recorded physical IDs remain inert data. Native archive recognition/replay
    // still requires its original actual files and is deliberately not invoked here.
    Ok(())
}
