// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind strict schema2 metadata, immutable payload generation and same-file parser facts.
use super::*;
use sugarsubstitute_supervision::committed_reader::CommittedReadObject;
impl IncidentDumpReader {
    #[expect(
        clippy::result_large_err,
        reason = "Preserve the full original reader cause inline in its admitted 768 KiB decode owner"
    )]
    pub(super) fn validate(
        &mut self,
        key: IncidentKey,
        revision: IncidentRevision,
        dump: DumpId,
        utc: IncidentUtc,
    ) -> Result<(), DumpReadCause> {
        if self.fields.first.is_some() || self.fields.stored.is_some() {
            return Err(DumpReadCause::Identity);
        }
        let summary_slot = self.source(CommittedReadObject::Summary(key))?;
        self.fields.summary_slot = Some(summary_slot);
        let stored =
            super::super::decode_summary(&self.fields.source).map_err(DumpReadCause::Record)?;
        validate_summary(&stored, key, revision, dump, utc)?;
        let payload_revision = stored.payload_revision().ok_or(DumpReadCause::Identity)?;
        let PayloadDisposition::Retained(inventory) = stored.summary().facts().payload() else {
            return Err(DumpReadCause::Identity);
        };
        let expected = inventory
            .entries()
            .find(|fact| fact.kind() == AttachmentKind::Minidump)
            .copied()
            .ok_or(DumpReadCause::Identity)?;
        let mut build = [0; 64];
        let build_bytes = stored.summary().seed().build().as_str().as_bytes();
        build[..build_bytes.len()].copy_from_slice(build_bytes);
        let build_length = build_bytes.len() as u32;
        self.fields.stored = Some(stored);
        self.source(CommittedReadObject::StoreState)?;
        let super::super::IncidentStoreState::Ledger(ledger) =
            super::super::decode_store_state(&self.fields.source).map_err(DumpReadCause::Record)?
        else {
            return Err(DumpReadCause::Identity);
        };
        if ledger.namespace() != key.subject().coordinator().namespace
            || ledger.last().is_some_and(|last| last.at() > utc)
        {
            return Err(DumpReadCause::Identity);
        }
        self.fields.ledger = Some(ledger);
        let native = self.native.as_mut().ok_or(DumpReadCause::Identity)?;
        let dump_slot = native
            .open(CommittedReadObject::Minidump(key, payload_revision))
            .map_err(DumpReadCause::Native)?;
        let facts = native.facts(&dump_slot).map_err(DumpReadCause::Native)?;
        self.fields.dump_slot = Some(dump_slot);
        if facts.bytes() != expected.retained_bytes() || facts.sha256() != *expected.sha256() {
            return Err(DumpReadCause::Identity);
        }
        self.fields.source.clear();
        self.fields.parser = Some(crate::native::committed_parser::validate(
            self,
            key,
            dump,
            facts.bytes(),
            build,
            build_length,
        )?);
        self.check_current_inner()?;
        self.fields.verified = true;
        Ok(())
    }
}
#[expect(
    clippy::result_large_err,
    reason = "Preserve the shared inline reader-cause contract; this pure validator allocates no error storage"
)]
pub(in crate::incident) fn validate_summary(
    stored: &StoredIncidentSummary,
    key: IncidentKey,
    revision: IncidentRevision,
    dump: DumpId,
    utc: IncidentUtc,
) -> Result<(), DumpReadCause> {
    let summary = stored.summary();
    let retention = summary.facts().retention();
    if summary.seed().key() != key
        || summary.revision() != revision
        || summary.facts().capture() != CaptureEvidence::Finished(dump)
        || summary.seed().created() > utc
        || retention.occurred() > utc
        || retention.acknowledged().is_some_and(|at| at > utc)
    {
        return Err(DumpReadCause::Identity);
    }
    Ok(())
}
