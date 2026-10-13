// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish exact seeds and summaries with native predecessor and payload validation.
use super::{
    IncidentRepository, IncidentStoreError, RedactedTextChunk, StoredIncidentSummary,
    repository::predecessor,
};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_contracts::incident::*;
use sugarsubstitute_supervision::diagnostic_storage::*;

/// Expected logical summary state; actual native identity is observed under writer custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpectedSummary {
    /// Initial publication cannot replace an existing summary.
    Absent,
    /// Replace only the exact current revision and complete canonical bytes.
    Exact {
        /// Expected immutable revision.
        revision: IncidentRevision,
        /// Expected whole-record digest.
        content_sha256: [u8; 32],
    },
}
/// Private proof retaining actual checked activation, distinct from seed matching data.
pub struct SeedCommit {
    seed: IncidentSeed,
    operation: LogicalStoreOperationKey,
    actual: DiagnosticActivation,
    _custody: DiagnosticRecoverySeedLoan,
}
impl SeedCommit {
    /// Borrow the exact committed seed.
    pub const fn seed(&self) -> &IncidentSeed {
        &self.seed
    }
    /// Observe the complete incident identity.
    pub const fn key(&self) -> IncidentKey {
        self.seed.key()
    }
    /// Observe the current operation which performed checked activation.
    pub const fn operation(&self) -> LogicalStoreOperationKey {
        self.operation
    }
    /// Observe the actual complete committed digest.
    pub fn content_sha256(&self) -> [u8; 32] {
        self.actual.facts().sha256()
    }
}
/// Private checked summary activation; matching public facts cannot construct it.
pub struct IncidentCommit {
    pub(super) operation: LogicalStoreOperationKey,
    pub(super) actual: DiagnosticActivation,
    pub(super) stored: super::allocation::RetainedSummary,
}
impl IncidentCommit {
    /// Borrow the exact committed summary.
    pub const fn summary(&self) -> &IncidentSummary {
        self.stored.stored.summary()
    }
    /// Observe the complete incident identity.
    pub const fn key(&self) -> IncidentKey {
        self.stored.stored.summary().seed().key()
    }
    /// Observe the resulting actual committed revision.
    pub const fn revision(&self) -> IncidentRevision {
        self.stored.stored.summary().revision()
    }
    /// Observe the immutable payload generation independently of the summary revision.
    pub const fn payload_revision(&self) -> Option<IncidentRevision> {
        self.stored.stored.payload_revision()
    }
    /// Observe the full current publication operation.
    pub const fn operation(&self) -> LogicalStoreOperationKey {
        self.operation
    }
    /// Observe the complete actual committed digest.
    pub fn content_sha256(&self) -> [u8; 32] {
        self.actual.facts().sha256()
    }
}
/// Immutable loaded summary plus separate current cumulative ledger observation.
pub struct CommittedIncident {
    ledger: SummaryEvictions,
    stored: super::allocation::RetainedSummary,
}
impl CommittedIncident {
    /// Borrow the validated current summary.
    pub const fn summary(&self) -> &IncidentSummary {
        self.stored.stored.summary()
    }
    /// Observe the immutable payload generation without granting native read authority.
    pub const fn payload_revision(&self) -> Option<IncidentRevision> {
        self.stored.stored.payload_revision()
    }
    /// Observe current ledger facts independently of historical summary content.
    pub const fn evictions(&self) -> SummaryEvictions {
        self.ledger
    }
}
/// One non-cloneable publication correlation; dropping it leaves repository custody intact.
pub struct PublishTicket {
    id: u64,
    operation: LogicalStoreOperationKey,
    identity: DiagnosticPublicationIdentity,
}
pub(super) struct PendingPublication {
    id: u64,
    seed: IncidentSeed,
    revision: IncidentRevision,
    expected: DiagnosticPredecessor,
    payloads: Vec<(AttachmentKind, u64, u64, u64, DiagnosticActivation)>,
    demand: super::IncidentPublicationDemand,
    context: JobContext,
    _custody: DiagnosticPendingPublicationLoan,
}
impl IncidentRepository {
    /// Durably publish a seed or reconcile an exactly identical existing seed.
    /// # Errors
    /// Changed duplicates, foreign identities, corrupt records and uncertain close block proof.
    pub fn publish_seed(
        &mut self,
        seed: IncidentSeed,
        context: &JobContext,
    ) -> Result<SeedCommit, IncidentStoreError> {
        if self.preflight.is_some() || self.pending.is_some() {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        self.publish_seed_current(seed, context)
    }
    fn publish_seed_current(
        &mut self,
        seed: IncidentSeed,
        context: &JobContext,
    ) -> Result<SeedCommit, IncidentStoreError> {
        let custody = self.storage.recovery_seed(&self.operation, context)?;
        self.publish_seed_admitted(seed, custody, context)
    }
    fn publish_seed_admitted(
        &mut self,
        seed: IncidentSeed,
        custody: DiagnosticRecoverySeedLoan,
        context: &JobContext,
    ) -> Result<SeedCommit, IncidentStoreError> {
        self.namespace(seed.key())?;
        let encoded = self.encode_seed(&seed, context)?;
        let inventory = self.inventory(context)?;
        let mut existing = None;
        for candidate in
            inventory.entries().filter(|entry| entry.class() == DiagnosticEntryClass::Seed)
        {
            let (bytes, facts, index) = self.read_candidate(candidate, context)?;
            let mut decode = self.storage.recovery_decode(&self.operation, context)?;
            let decoded =
                super::allocation::decode(&mut decode, || super::decode_seed(bytes.as_bytes()))?;
            self.reads
                .get_mut(index)?
                .bind_object(DiagnosticObject::Seed(decoded.key()), context)?;
            if decoded.key() == seed.key() {
                if bytes.as_bytes() != encoded.as_bytes() || existing.is_some() {
                    return Err(IncidentStoreError::ChangedDuplicate);
                }
                existing = Some(facts);
            }
            self.close_read(index, context)?;
        }
        drop(inventory);
        let object = DiagnosticObject::Seed(seed.key());
        let actual = if let Some(facts) = existing {
            self.storage.reconcile_activation(
                object,
                predecessor(facts),
                &self.operation,
                context,
            )?
        } else {
            self.activate_bytes(object, encoded.as_bytes(), DiagnosticPredecessor::Absent, context)?
        };
        Ok(SeedCommit { seed, operation: self.operation.key(), actual, _custody: custody })
    }
    /// Admit one publication at the current sealed operation and observed expected revision.
    /// # Errors
    /// Rejects a live predecessor ticket, foreign operation or changed summary before staging.
    pub fn begin_publish(
        &mut self,
        preflight: super::IncidentPublicationPreflight,
        operation: IncidentOperationId,
        context: &JobContext,
    ) -> Result<PublishTicket, IncidentStoreError> {
        if self.pending.is_some()
            || operation != self.operation.key().id()
            || preflight.operation != self.operation.key()
            || !preflight.identity.matches(&self.publication_identity)
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        let custody = self.storage.recovery_pending(&self.operation, context)?;
        let seed_custody = self.storage.recovery_seed(&self.operation, context)?;
        let identity = self.publication_identity.retain_for_token(&self.operation, context)?;
        let mut payloads = Vec::new();
        if let Err(source) = payloads.try_reserve_exact(MAX_ATTACHMENTS) {
            return Err(IncidentStoreError::Allocation(
                super::allocation::LoanedAllocationFailure::pending(source, custody),
            ));
        }
        if payloads.capacity() != MAX_ATTACHMENTS {
            return Err(IncidentStoreError::Capacity);
        }
        let retained = self
            .preflight
            .as_ref()
            .filter(|retained| retained.id == preflight.id && retained.matches_context(context))
            .ok_or(IncidentStoreError::RecoveryConflict)?;
        let (seed, expected, revision, native, original) = (
            retained.seed.clone(),
            retained.expected,
            retained.revision,
            retained.native,
            retained.context.clone(),
        );
        if self.publication_predecessor(&seed, expected, &original)? != (revision, native) {
            return Err(IncidentStoreError::StaleRevision);
        }
        self.publish_seed_admitted(seed.clone(), seed_custody, &original)?;
        let retained = self.preflight.take().ok_or(IncidentStoreError::RecoveryConflict)?;
        self.pending = Some(PendingPublication {
            id: retained.id,
            seed,
            revision,
            expected: native,
            payloads,
            demand: retained.demand,
            context: original,
            _custody: custody,
        });
        Ok(PublishTicket { id: retained.id, operation: self.operation.key(), identity })
    }
    fn pending(&self, ticket: &PublishTicket) -> Result<&PendingPublication, IncidentStoreError> {
        self.pending
            .as_ref()
            .filter(|pending| {
                pending.id == ticket.id
                    && ticket.operation == self.operation.key()
                    && ticket.identity.matches(&self.publication_identity)
            })
            .ok_or(IncidentStoreError::RecoveryConflict)
    }
    /// Publish one bounded allowlist-generated text attachment without raw sensitive input.
    /// # Errors
    /// Repeated kinds, dump kinds, overload and uncertain activation preserve custody.
    pub fn append_text(
        &mut self,
        ticket: &mut PublishTicket,
        kind: AttachmentKind,
        chunk: RedactedTextChunk,
        context: &JobContext,
    ) -> Result<(), IncidentStoreError> {
        let pending = self.pending(ticket)?;
        let bytes = pending
            .payloads
            .iter()
            .try_fold(chunk.as_bytes().len() as u64, |sum, (_, bytes, ..)| {
                sum.checked_add(*bytes).ok_or(IncidentStoreError::Capacity)
            })?;
        if kind == AttachmentKind::Minidump
            || pending.payloads.len() >= MAX_ATTACHMENTS
            || pending.payloads.iter().any(|(existing, ..)| *existing == kind)
            || bytes > pending.demand.payload_bytes()
            || pending.demand.payload_groups() != 1
            || !same_context(&pending.context, context)
        {
            return Err(IncidentStoreError::Capacity);
        }
        let object = DiagnosticObject::Payload(pending.seed.key(), pending.revision, kind);
        let original = pending.context.clone();
        let actual = self.activate_bytes(
            object,
            chunk.as_bytes(),
            DiagnosticPredecessor::Absent,
            &original,
        )?;
        let pending = self.pending.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        pending.payloads.push((
            kind,
            chunk.as_bytes().len() as u64,
            chunk.original_bytes(),
            chunk.omitted_bytes(),
            actual,
        ));
        Ok(())
    }
    /// Activate the exact summary only after every referenced payload is actually verified.
    /// # Errors
    /// Changed identity/revision/payload facts or uncertain activation retain the ticket state.
    pub fn commit(
        &mut self,
        ticket: &mut PublishTicket,
        summary: IncidentSummary,
        context: &JobContext,
    ) -> Result<IncidentCommit, IncidentStoreError> {
        let pending = self.pending(ticket)?;
        if !same_context(&pending.context, context) {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        let original = pending.context.clone();
        if summary.seed() != &pending.seed || summary.revision() != pending.revision {
            return Err(IncidentStoreError::StaleRevision);
        }
        match summary.facts().payload() {
            PayloadDisposition::Retained(inventory) => {
                if inventory.entries().count() != pending.payloads.len() {
                    return Err(IncidentStoreError::MissingPayload);
                }
                for fact in inventory.entries() {
                    let Some((_, bytes, original, omitted, actual)) =
                        pending.payloads.iter().find(|(kind, ..)| *kind == fact.kind())
                    else {
                        return Err(IncidentStoreError::MissingPayload);
                    };
                    if *bytes != fact.retained_bytes()
                        || *original != fact.original_bytes()
                        || *omitted != fact.omitted_bytes()
                        || actual.facts().sha256() != *fact.sha256()
                    {
                        return Err(IncidentStoreError::WrongDigest);
                    }
                }
            }
            _ if !pending.payloads.is_empty() => return Err(IncidentStoreError::MissingPayload),
            _ => (),
        }
        let expected = pending.expected;
        let payload_revision = matches!(summary.facts().payload(), PayloadDisposition::Retained(_))
            .then_some(summary.revision());
        let custody = self.storage.recovery_record(&self.operation, &original)?;
        let stored = super::allocation::RetainedSummary::new(
            StoredIncidentSummary::new(summary, payload_revision)?,
            custody,
        );
        self.verify_payloads(&stored, &original)?;
        let bytes = self.encode_summary(&stored, &original)?;
        let actual = self.activate_bytes(
            DiagnosticObject::Summary(stored.summary().seed().key()),
            bytes.as_bytes(),
            expected,
            &original,
        )?;
        self.pending = None;
        Ok(IncidentCommit { stored, operation: self.operation.key(), actual })
    }
    /// Load validated current summary and actual namespace ledger under writer custody.
    /// # Errors
    /// Missing/corrupt/wrong-key summaries or ledgers preserve actual failed-read custody.
    pub fn load(
        &mut self,
        key: IncidentKey,
        context: &JobContext,
    ) -> Result<CommittedIncident, IncidentStoreError> {
        let (stored, _) =
            self.find_summary(key, context)?.ok_or(IncidentStoreError::MissingPayload)?;
        self.verify_payloads(&stored, context)?;
        let ledger = self.ledger(context)?;
        Ok(CommittedIncident { stored, ledger })
    }
}
pub(super) fn same_context(original: &JobContext, supplied: &JobContext) -> bool {
    original.owner_id() == supplied.owner_id()
        && original.scope_id() == supplied.scope_id()
        && original.spec() == supplied.spec()
        && original.deadline() == supplied.deadline()
        && !supplied.cancellation_requested()
}
