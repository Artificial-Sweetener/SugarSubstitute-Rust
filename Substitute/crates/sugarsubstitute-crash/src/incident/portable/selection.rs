// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify complete passive logical content and refuse ambiguous or unfinished inventories.
use super::*;
use sugarsubstitute_contracts::incident::{MAX_ATTACHMENTS, MAX_SUMMARIES, PayloadDisposition};

pub(super) fn qualify(
    session: &mut dyn PortableIncidentReadSession,
    provenance: PortableIncidentProvenance,
    now: IncidentUtc,
    context: &JobContext,
) -> Result<HistoryData, PortableIncidentError> {
    let observed = session.inventory(context)?;
    if observed.len() > DIAGNOSTIC_SCAN_BOUND {
        return Err(source::rejected(PortableIncidentFailureCode::Capacity));
    }
    let mut entries = Vec::new();
    entries.try_reserve_exact(observed.len()).map_err(PortableIncidentError::allocation)?;
    entries.extend_from_slice(observed);
    for (index, entry) in entries.iter().enumerate() {
        if entries[..index].iter().any(|prior| prior.id == entry.id) {
            return Err(source::rejected(PortableIncidentFailureCode::Duplicate));
        }
    }
    let records = records::read(session, &entries, provenance.namespace(), now, context)?;
    archives::validate(session, &records.stages, &records.archives, context)?;
    let mut payloads = Vec::new();
    for stored in &records.summaries {
        let PayloadDisposition::Retained(inventory) = stored.summary().facts().payload() else {
            continue;
        };
        let revision = stored
            .payload_revision()
            .ok_or_else(|| source::rejected(PortableIncidentFailureCode::MissingContent))?;
        for fact in inventory.entries() {
            let object =
                DiagnosticObject::Payload(stored.summary().seed().key(), revision, fact.kind());
            let mut matching = None;
            for entry in records.payloads.iter().copied() {
                source::control(context)?;
                if session.bind_typed_object(entry.id, object, context)? {
                    if matching.is_some() {
                        return Err(source::rejected(PortableIncidentFailureCode::Duplicate));
                    }
                    matching = Some(entry);
                }
            }
            let entry = matching
                .ok_or_else(|| source::rejected(PortableIncidentFailureCode::MissingContent))?;
            if payloads.iter().any(|prior: &Payload| prior.entry.id == entry.id) {
                return Err(source::rejected(PortableIncidentFailureCode::Duplicate));
            }
            if entry.bytes != fact.retained_bytes() {
                return Err(source::rejected(PortableIncidentFailureCode::WrongDigest));
            }
            let actual = source::stream(session, entry, context, |_| Ok(()))?;
            if actual != *fact.sha256() {
                return Err(source::rejected(PortableIncidentFailureCode::WrongDigest));
            }
            source::push(
                &mut payloads,
                MAX_SUMMARIES * MAX_ATTACHMENTS,
                Payload { entry, object, sha256: actual },
            )?;
        }
    }
    if payloads.len() != records.payloads.len() {
        return Err(source::rejected(PortableIncidentFailureCode::Uncommitted));
    }
    source::control(context)?;
    session.revalidate(context)?;
    if session.provenance() != provenance {
        return Err(source::rejected(PortableIncidentFailureCode::ChangedSource));
    }
    Ok(HistoryData { summaries: records.summaries, payloads, evictions: records.evictions })
}
