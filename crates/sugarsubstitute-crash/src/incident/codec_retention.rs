// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve bounded exact payload-discard intent without granting native deletion authority.
use super::{
    IncidentFileFacts, IncidentStoreError,
    codec_evidence::{KindRecord, PayloadRecord},
    codec_identity::{OperationRecord, SubjectRecord},
    codec_ledger::FileRecord,
};
use serde::{Deserialize, Serialize};
use sugarsubstitute_contracts::incident::{
    AttachmentInventory, AttachmentKind, DiscardedPayload, IncidentKey, IncidentRevision,
    LogicalStoreOperationKey, MAX_ATTACHMENTS, MAX_PAYLOAD_BYTES, MAX_TEXT_ATTACHMENT_BYTES,
    PayloadDiscard, PayloadDisposition,
};

/// Recorded expected payload facts, distinct from an actual retained native observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayloadDiscardVictimFacts {
    kind: AttachmentKind,
    identity: [u8; 24],
    sha256: [u8; 32],
    bytes: u64,
}
impl PayloadDiscardVictimFacts {
    /// Validate one nonempty allowlisted payload without opening or deleting it.
    /// # Errors
    /// Rejects zero identity/digest, oversized text or more than 256 MiB of payload.
    pub fn new(
        kind: AttachmentKind,
        identity: [u8; 24],
        sha256: [u8; 32],
        bytes: u64,
    ) -> Result<Self, IncidentStoreError> {
        if identity == [0; 24]
            || sha256 == [0; 32]
            || bytes == 0
            || bytes > MAX_PAYLOAD_BYTES
            || (kind != AttachmentKind::Minidump && bytes > MAX_TEXT_ATTACHMENT_BYTES)
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        Ok(Self { kind, identity, sha256, bytes })
    }
    /// Return the exact semantic payload kind.
    pub const fn kind(self) -> AttachmentKind {
        self.kind
    }
    /// Return recorded matching identity bytes, never an open file capability.
    pub const fn identity(self) -> [u8; 24] {
        self.identity
    }
    /// Return the recorded complete-content digest.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }
    /// Return recorded retained physical bytes.
    pub const fn bytes(self) -> u64 {
        self.bytes
    }
}

/// Exact historical intent for replacing one summary before reclaiming its payload generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PayloadDiscardJournal {
    operation: LogicalStoreOperationKey,
    incident: IncidentKey,
    original_revision: IncidentRevision,
    payload_revision: IncidentRevision,
    original_file: IncidentFileFacts,
    candidate_stage: IncidentFileFacts,
    original_payload: AttachmentInventory,
    victims: Box<[PayloadDiscardVictimFacts]>,
    disposition: DiscardedPayload,
}
impl PayloadDiscardJournal {
    /// Validate one complete ordered group and its distinct summary stage expectations.
    /// # Errors
    /// Rejects foreign namespaces, revision exhaustion, invalid generations, unsupported
    /// discard reasons, changed attachment facts or duplicate/aliased native identities.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        operation: LogicalStoreOperationKey,
        incident: IncidentKey,
        original_revision: IncidentRevision,
        payload_revision: IncidentRevision,
        original_file: IncidentFileFacts,
        candidate_stage: IncidentFileFacts,
        original_payload: AttachmentInventory,
        victims: &[PayloadDiscardVictimFacts],
        disposition: DiscardedPayload,
    ) -> Result<Self, IncidentStoreError> {
        original_revision.next()?;
        if operation.owner().namespace != incident.subject().coordinator().namespace
            || payload_revision.get() > original_revision.get()
            || original_file.identity() == candidate_stage.identity()
            || original_file.sha256() == candidate_stage.sha256()
            || victims.is_empty()
            || victims.len() > MAX_ATTACHMENTS
            || original_payload.entries().count() != victims.len()
            || disposition.source_bytes() != original_payload.total_bytes()
            || !matches!(
                disposition.reason(),
                PayloadDiscard::AcknowledgedRetention | PayloadDiscard::StoragePressure
            )
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        for (index, (attachment, victim)) in original_payload.entries().zip(victims).enumerate() {
            if attachment.kind() != victim.kind
                || attachment.retained_bytes() != victim.bytes
                || *attachment.sha256() != victim.sha256
                || victim.identity == original_file.identity()
                || victim.identity == candidate_stage.identity()
                || victims[..index].iter().any(|prior| prior.identity == victim.identity)
            {
                return Err(IncidentStoreError::RecoveryConflict);
            }
        }
        Ok(Self {
            operation,
            incident,
            original_revision,
            payload_revision,
            original_file,
            candidate_stage,
            original_payload,
            victims: victims.into(),
            disposition,
        })
    }
    /// Return the historical full operation; this does not establish current authority.
    pub const fn operation(&self) -> LogicalStoreOperationKey {
        self.operation
    }
    /// Return the exact incident whose summary owns this generation.
    pub const fn incident(&self) -> IncidentKey {
        self.incident
    }
    /// Return the original current summary revision before its single checked advance.
    pub const fn original_revision(&self) -> IncidentRevision {
        self.original_revision
    }
    /// Return the immutable physical payload publication revision.
    pub const fn payload_revision(&self) -> IncidentRevision {
        self.payload_revision
    }
    /// Return recorded original summary facts.
    pub const fn original_file(&self) -> IncidentFileFacts {
        self.original_file
    }
    /// Return recorded candidate summary stage facts.
    pub const fn candidate_stage(&self) -> IncidentFileFacts {
        self.candidate_stage
    }
    /// Borrow original attachment facts, including exact source and omission counts.
    pub const fn original_payload(&self) -> &AttachmentInventory {
        &self.original_payload
    }
    /// Borrow exact expected victims in original inventory order.
    pub fn victims(&self) -> impl Iterator<Item = &PayloadDiscardVictimFacts> {
        self.victims.iter()
    }
    /// Return the summary's explicit discard observation, without asserting deletion.
    pub const fn disposition(&self) -> DiscardedPayload {
        self.disposition
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PayloadDiscardRecord {
    operation: OperationRecord,
    incident: SubjectRecord,
    original_revision: u64,
    payload_revision: u64,
    original_file: FileRecord,
    candidate_stage: FileRecord,
    original_payload: PayloadRecord,
    #[serde(deserialize_with = "bounded_victims")]
    victims: Vec<VictimRecord>,
    disposition: PayloadRecord,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VictimRecord {
    kind: KindRecord,
    identity: [u8; 24],
    sha256: [u8; 32],
    bytes: u64,
}
fn bounded_victims<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<VictimRecord>, D::Error> {
    struct Visitor;
    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = Vec<VictimRecord>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("at most eight exact payload victims")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut sequence: A,
        ) -> Result<Self::Value, A::Error> {
            let mut entries = Vec::with_capacity(MAX_ATTACHMENTS);
            while let Some(entry) = sequence.next_element()? {
                if entries.len() == MAX_ATTACHMENTS {
                    return Err(serde::de::Error::custom("payload victims exceed their bound"));
                }
                entries.push(entry);
            }
            Ok(entries)
        }
    }
    deserializer.deserialize_seq(Visitor)
}
impl PayloadDiscardRecord {
    pub(super) fn into_journal(self) -> Result<PayloadDiscardJournal, IncidentStoreError> {
        let PayloadDisposition::Retained(original_payload) = self.original_payload.into_facts()?
        else {
            return Err(IncidentStoreError::RecoveryConflict);
        };
        let PayloadDisposition::Discarded(disposition) = self.disposition.into_facts()? else {
            return Err(IncidentStoreError::RecoveryConflict);
        };
        let victims = self
            .victims
            .into_iter()
            .map(|victim| {
                PayloadDiscardVictimFacts::new(
                    victim.kind.into_facts(),
                    victim.identity,
                    victim.sha256,
                    victim.bytes,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        PayloadDiscardJournal::new(
            self.operation.into_operation()?,
            self.incident.into_key()?,
            IncidentRevision::new(self.original_revision)?,
            IncidentRevision::new(self.payload_revision)?,
            self.original_file.into_facts()?,
            self.candidate_stage.into_facts()?,
            original_payload,
            &victims,
            disposition,
        )
    }
}
impl From<&PayloadDiscardJournal> for PayloadDiscardRecord {
    fn from(value: &PayloadDiscardJournal) -> Self {
        Self {
            operation: value.operation.into(),
            incident: value.incident.into(),
            original_revision: value.original_revision.get(),
            payload_revision: value.payload_revision.get(),
            original_file: value.original_file.into(),
            candidate_stage: value.candidate_stage.into(),
            original_payload: (&PayloadDisposition::Retained(value.original_payload.clone()))
                .into(),
            victims: value
                .victims()
                .map(|victim| VictimRecord {
                    kind: victim.kind.into(),
                    identity: victim.identity,
                    sha256: victim.sha256,
                    bytes: victim.bytes,
                })
                .collect(),
            disposition: (&PayloadDisposition::Discarded(value.disposition)).into(),
        }
    }
}
