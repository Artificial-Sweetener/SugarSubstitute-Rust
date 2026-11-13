// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode logical records with existing codecs; native-bound transaction data remains inert.
use super::*;
use crate::incident::{
    IncidentStoreState, SummaryMigrationJournal, decode_seed, decode_store_state,
    encode_store_state,
};
use sugarsubstitute_contracts::incident::{IncidentSeed, MAX_SUMMARIES};

pub(super) struct Records {
    pub(super) summaries: Vec<StoredIncidentSummary>,
    pub(super) archives: Vec<SummaryMigrationJournal>,
    pub(super) stages: Vec<PortableIncidentEntry>,
    pub(super) payloads: Vec<PortableIncidentEntry>,
    pub(super) evictions: sugarsubstitute_contracts::incident::SummaryEvictions,
}
pub(super) fn read(
    session: &mut dyn PortableIncidentReadSession,
    entries: &[PortableIncidentEntry],
    namespace: NamespaceId,
    now: IncidentUtc,
    context: &JobContext,
) -> Result<Records, PortableIncidentError> {
    let mut result = Records {
        summaries: Vec::new(),
        archives: Vec::new(),
        stages: Vec::new(),
        payloads: Vec::new(),
        evictions: sugarsubstitute_contracts::incident::SummaryEvictions::new(
            namespace, 0, 0, 0, None,
        )
        .map_err(|cause| PortableIncidentError::codec(cause.into()))?,
    };
    let mut seeds = Vec::new();
    let mut ledger = false;
    for entry in entries.iter().copied() {
        source::control(context)?;
        match entry.class {
            DiagnosticEntryClass::Summary => {
                let bytes = source::record(session, entry, 65536, context)?;
                let stored = read_summary(&bytes)?;
                let summary = stored.summary();
                namespace_matches(
                    summary.seed().key().subject().coordinator().namespace,
                    namespace,
                )?;
                if summary.seed().created() > now
                    || summary.facts().retention().occurred() > now
                    || summary.facts().retention().acknowledged().is_some_and(|at| at > now)
                {
                    return Err(PortableIncidentError::codec(IncidentStoreError::RecoveryConflict));
                }
                if result
                    .summaries
                    .iter()
                    .any(|prior| prior.summary().seed().key() == summary.seed().key())
                {
                    return Err(source::rejected(PortableIncidentFailureCode::Duplicate));
                }
                source::bind(
                    session,
                    entry,
                    DiagnosticObject::Summary(summary.seed().key()),
                    context,
                )?;
                source::push(&mut result.summaries, MAX_SUMMARIES, stored)?;
            }
            DiagnosticEntryClass::Seed => {
                let seed = decode_seed(&source::record(session, entry, 4096, context)?)
                    .map_err(PortableIncidentError::codec)?;
                namespace_matches(seed.key().subject().coordinator().namespace, namespace)?;
                source::bind(session, entry, DiagnosticObject::Seed(seed.key()), context)?;
                if seeds.iter().any(|prior: &IncidentSeed| prior.key() == seed.key()) {
                    return Err(source::rejected(PortableIncidentFailureCode::Duplicate));
                }
                source::push(&mut seeds, MAX_SUMMARIES, seed)?;
            }
            DiagnosticEntryClass::StoreState => {
                if ledger {
                    return Err(source::rejected(PortableIncidentFailureCode::Duplicate));
                }
                let bytes = source::record(session, entry, 65536, context)?;
                let state = decode_store_state(&bytes).map_err(PortableIncidentError::codec)?;
                let IncidentStoreState::Ledger(value) = state else {
                    return Err(source::rejected(PortableIncidentFailureCode::Uncommitted));
                };
                namespace_matches(value.namespace(), namespace)?;
                if value.last().is_some_and(|last| last.at() > now) {
                    return Err(PortableIncidentError::codec(IncidentStoreError::RecoveryConflict));
                }
                source::bind(session, entry, DiagnosticObject::StoreState, context)?;
                result.evictions = value;
                ledger = true;
            }
            DiagnosticEntryClass::Journal => {
                let bytes = source::record(session, entry, 65536, context)?;
                let state = decode_store_state(&bytes).map_err(PortableIncidentError::codec)?;
                if encode_store_state(&state).map_err(PortableIncidentError::codec)? != bytes {
                    return Err(PortableIncidentError::codec(IncidentStoreError::RecoveryConflict));
                }
                let operation = match &state {
                    IncidentStoreState::SummaryMigrationArchive(archive) => {
                        archive.migration().operation()
                    }
                    IncidentStoreState::RecoveryCaptureOrigin(_) => {
                        return Err(source::rejected(
                            PortableIncidentFailureCode::UnsupportedRecord,
                        ));
                    }
                    _ => return Err(source::rejected(PortableIncidentFailureCode::Uncommitted)),
                };
                namespace_matches(operation.owner().namespace, namespace)?;
                source::bind(session, entry, DiagnosticObject::Journal(operation), context)?;
                if let IncidentStoreState::SummaryMigrationArchive(archive) = state {
                    namespace_matches(
                        archive.migration().incident().subject().coordinator().namespace,
                        namespace,
                    )?;
                    source::push(
                        &mut result.archives,
                        DIAGNOSTIC_SCAN_BOUND,
                        archive.migration().clone(),
                    )?;
                }
            }
            DiagnosticEntryClass::Stage => {
                source::push(&mut result.stages, DIAGNOSTIC_SCAN_BOUND, entry)?
            }
            DiagnosticEntryClass::Payload => {
                source::push(&mut result.payloads, DIAGNOSTIC_SCAN_BOUND, entry)?
            }
            DiagnosticEntryClass::Quarantine | DiagnosticEntryClass::Unknown => {
                return Err(source::rejected(PortableIncidentFailureCode::UnsupportedRecord));
            }
        }
    }
    if !ledger {
        return Err(source::rejected(PortableIncidentFailureCode::MissingContent));
    }
    for seed in seeds {
        if !result.summaries.iter().any(|stored| stored.summary().seed() == &seed) {
            return Err(source::rejected(PortableIncidentFailureCode::Uncommitted));
        }
    }
    Ok(result)
}
fn namespace_matches(
    actual: NamespaceId,
    expected: NamespaceId,
) -> Result<(), PortableIncidentError> {
    if actual != expected {
        return Err(source::rejected(PortableIncidentFailureCode::ForeignNamespace));
    }
    Ok(())
}
fn read_summary(bytes: &[u8]) -> Result<StoredIncidentSummary, PortableIncidentError> {
    let record: crate::incident::codec::Record<serde::de::IgnoredAny> =
        crate::incident::codec_parse::parse(bytes)
            .map_err(|cause| PortableIncidentError::codec(cause.into()))?;
    if record.schema_version == 1 {
        let readable =
            crate::incident::migrate_summary_v1(bytes).map_err(PortableIncidentError::codec)?;
        crate::incident::decode_summary(&readable).map_err(PortableIncidentError::codec)
    } else {
        crate::incident::decode_summary(bytes).map_err(PortableIncidentError::codec)
    }
}
