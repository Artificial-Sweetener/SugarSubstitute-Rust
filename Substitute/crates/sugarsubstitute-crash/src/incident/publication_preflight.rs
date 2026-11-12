// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain finite publication demand before protected staging and immutable payload activation.
use super::{ExpectedSummary, IncidentRepository, IncidentStoreError, repository::predecessor};
use sugarsubstitute_contracts::{execution::JobContext, incident::*};
use sugarsubstitute_supervision::diagnostic_storage::*;

/// Bounded incoming data; actual protected admission remains independent.
pub struct IncidentPublicationDemand {
    payload_bytes: u64,
    payload_groups: usize,
    summary_slots: usize,
    staging_headroom: u64,
}
impl IncidentPublicationDemand {
    /// Check bytes and peak staging against the same 256 MiB physical ceiling.
    /// # Errors
    /// Inconsistent groups, excess slots, insufficient 128 KiB headroom or overflow reject.
    pub fn new(
        payload_bytes: u64,
        payload_groups: usize,
        summary_slots: usize,
        staging_headroom: u64,
    ) -> Result<Self, IncidentStoreError> {
        if payload_groups > 1
            || summary_slots > 1
            || (payload_bytes == 0) != (payload_groups == 0)
            || !(131_072..=MAX_PAYLOAD_BYTES).contains(&staging_headroom)
            || payload_bytes.checked_add(staging_headroom).is_none_or(|sum| sum > MAX_PAYLOAD_BYTES)
        {
            return Err(IncidentStoreError::Capacity);
        }
        Ok(Self { payload_bytes, payload_groups, summary_slots, staging_headroom })
    }
    /// Observe maximum cumulative incoming immutable payload bytes.
    pub const fn payload_bytes(&self) -> u64 {
        self.payload_bytes
    }
    /// Observe new physical payload generations, including replacements.
    pub const fn payload_groups(&self) -> usize {
        self.payload_groups
    }
    /// Observe additional summary slots.
    pub const fn summary_slots(&self) -> usize {
        self.summary_slots
    }
    /// Observe reserved projection for simultaneous candidate and intent stages, in bytes.
    pub const fn staging_headroom(&self) -> u64 {
        self.staging_headroom
    }
}
/// One-use correlation; passive drop leaves the repository's retained admission intact.
pub struct IncidentPublicationPreflight {
    pub(super) id: u64,
    pub(super) operation: LogicalStoreOperationKey,
    pub(super) identity: DiagnosticPublicationIdentity,
}
pub(super) struct RetainedPreflight {
    pub(super) id: u64,
    pub(super) seed: IncidentSeed,
    pub(super) revision: IncidentRevision,
    pub(super) expected: ExpectedSummary,
    pub(super) native: DiagnosticPredecessor,
    pub(super) demand: IncidentPublicationDemand,
    pub(super) context: JobContext,
    pub(super) _custody: DiagnosticRetainedPreflightLoan,
}
impl RetainedPreflight {
    pub(super) fn matches_context(&self, supplied: &JobContext) -> bool {
        super::publication::same_context(&self.context, supplied)
    }
}
impl IncidentRepository {
    pub(super) fn publication_inventory(
        &mut self,
        now: IncidentUtc,
        context: &JobContext,
    ) -> Result<(DiagnosticInventory, super::allocation::SummaryCollection), IncidentStoreError>
    {
        let inventory = self.inventory(context)?;
        if inventory.unknown_entries() != 0
            || inventory
                .entries()
                .any(|entry| matches!(entry.class(), DiagnosticEntryClass::Quarantine))
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        self.recognize_summary_archives(&inventory, context)?;
        let custody = self.storage.recovery_collection(&self.operation, context)?;
        let mut collection = super::allocation::SummaryCollection::new(custody)?;
        let (mut referenced_bytes, mut referenced_files, mut referenced_groups) =
            (0_u64, 0_usize, 0_usize);
        for entry in inventory.entries().filter(|entry| {
            matches!(entry.class(), DiagnosticEntryClass::Seed | DiagnosticEntryClass::Summary)
        }) {
            if (entry.class() == DiagnosticEntryClass::Seed
                && collection.keys.len() == MAX_SUMMARIES)
                || (entry.class() == DiagnosticEntryClass::Summary
                    && collection.records.len() == MAX_SUMMARIES)
            {
                return Err(IncidentStoreError::Capacity);
            }
            let (bytes, facts, index) = self.read_candidate(entry, context)?;
            let object = if entry.class() == DiagnosticEntryClass::Seed {
                let mut decode = self.storage.recovery_decode(&self.operation, context)?;
                let seed = super::allocation::decode(&mut decode, || {
                    super::decode_seed(bytes.as_bytes())
                })?;
                self.namespace(seed.key())?;
                if seed.created() > now || collection.keys.contains(&seed.key()) {
                    return Err(IncidentStoreError::RecoveryConflict);
                }
                collection.keys.push(seed.key());
                DiagnosticObject::Seed(seed.key())
            } else {
                let mut decode = self.storage.recovery_decode(&self.operation, context)?;
                let stored = super::allocation::decode(&mut decode, || {
                    super::codec_summary::decode_current_summary(bytes.as_bytes())
                })?;
                let summary = stored.summary();
                self.namespace(summary.seed().key())?;
                if summary.seed().created() > now
                    || summary.facts().retention().occurred() > now
                    || summary.facts().retention().acknowledged().is_some_and(|at| at > now)
                    || collection.records.iter().any(
                        |(prior, _): &(super::StoredIncidentSummary, DiagnosticReadFacts)| {
                            prior.summary().seed().key() == summary.seed().key()
                        },
                    )
                {
                    return Err(IncidentStoreError::RecoveryConflict);
                }
                if let PayloadDisposition::Retained(attachments) = summary.facts().payload() {
                    referenced_bytes = referenced_bytes
                        .checked_add(attachments.total_bytes())
                        .ok_or(IncidentStoreError::Capacity)?;
                    referenced_files = referenced_files
                        .checked_add(attachments.entries().count())
                        .ok_or(IncidentStoreError::Capacity)?;
                    referenced_groups += 1;
                }
                let object = DiagnosticObject::Summary(summary.seed().key());
                collection.records.push((stored, facts));
                object
            };
            self.reads.get_mut(index)?.bind_object(object, context)?;
            self.close_read(index, context)?;
        }
        let (actual_bytes, actual_files) = inventory
            .entries()
            .filter(|entry| entry.class() == DiagnosticEntryClass::Payload)
            .try_fold((0_u64, 0_usize), |(sum, count), entry| {
                Ok::<_, IncidentStoreError>((
                    sum.checked_add(entry.bytes()).ok_or(IncidentStoreError::Capacity)?,
                    count.checked_add(1).ok_or(IncidentStoreError::Capacity)?,
                ))
            })?;
        if referenced_files != actual_files
            || referenced_bytes != actual_bytes
            || referenced_groups != inventory.payload_groups()
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        for (stored, _) in &collection.records {
            self.verify_payloads(stored, context)?;
        }
        Ok((inventory, collection))
    }
    /// Check actual inventory and finite demand, then retain one-use admission.
    ///
    /// Injected UTC is checked against every decoded occurrence/acknowledgement.
    /// Real protected retention settles each group before another is selected.
    /// The context and its original deadline remain retained through publication.
    /// # Errors
    /// Live pins, unknown content, changed predecessors, regressing UTC or capacity
    /// reject while preserving actual custody. Existing retained evidence requires its
    /// protected disposition to settle before replacement. Rejected ticket numbers are never reused.
    pub fn preflight_publication(
        &mut self,
        seed: IncidentSeed,
        expected: ExpectedSummary,
        demand: IncidentPublicationDemand,
        now: IncidentUtc,
        context: &JobContext,
    ) -> Result<IncidentPublicationPreflight, IncidentStoreError> {
        let id = self.next_ticket;
        self.next_ticket = id.checked_add(1).ok_or(IncidentStoreError::Capacity)?;
        if self.pending.is_some()
            || self.preflight.is_some()
            || !self.reads.is_empty()
            || !self.stages.is_empty()
            || self.retention_utc.is_some_and(|previous| now < previous)
            || now < seed.created()
            || demand.summary_slots != usize::from(matches!(expected, ExpectedSummary::Absent))
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        self.namespace(seed.key())?;
        let (revision, native) = self.publication_predecessor(&seed, expected, context)?;
        self.project_publication(&demand, now, seed.key(), context)?;
        // Projection can perform retention, so observe the exact selected predecessor again.
        if self.publication_predecessor(&seed, expected, context)? != (revision, native) {
            return Err(IncidentStoreError::StaleRevision);
        }
        let custody = self.storage.recovery_preflight(&self.operation, context)?;
        let identity = self.publication_identity.retain_for_token(&self.operation, context)?;
        self.retention_utc = Some(now);
        self.preflight = Some(RetainedPreflight {
            id,
            seed,
            revision,
            expected,
            native,
            demand,
            context: context.clone(),
            _custody: custody,
        });
        Ok(IncidentPublicationPreflight { id, operation: self.operation(), identity })
    }
    pub(super) fn publication_predecessor(
        &mut self,
        seed: &IncidentSeed,
        expected: ExpectedSummary,
        context: &JobContext,
    ) -> Result<(IncidentRevision, DiagnosticPredecessor), IncidentStoreError> {
        match (expected, self.find_summary(seed.key(), context)?) {
            (ExpectedSummary::Absent, None) => {
                Ok((seed.revision().next()?, DiagnosticPredecessor::Absent))
            }
            (ExpectedSummary::Exact { revision, content_sha256 }, Some((summary, facts)))
                if summary.summary().revision() == revision
                    && facts.sha256() == content_sha256
                    && summary.summary().seed() == seed =>
            {
                if matches!(summary.summary().facts().payload(), PayloadDisposition::Retained(_)) {
                    return Err(IncidentStoreError::RetainedPayloadRequiresDisposition);
                }
                Ok((revision.next()?, predecessor(facts)))
            }
            _ => Err(IncidentStoreError::StaleRevision),
        }
    }
}
