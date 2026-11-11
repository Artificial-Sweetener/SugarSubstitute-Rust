// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Characterize exact closed payload-generation intent and malformed persisted input.
use super::*;

pub(super) fn journal() -> Result<PayloadDiscardJournal, Box<dyn Error>> {
    let attachments = AttachmentInventory::new(&[
        AttachmentFacts::new(AttachmentKind::Log, 7, 900, 893, [5; 32])?,
        AttachmentFacts::new(AttachmentKind::Minidump, 70_000, 70_000, 0, [6; 32])?,
    ])?;
    Ok(PayloadDiscardJournal::new(
        LogicalStoreOperationKey::new(owner(8)?, IncidentOperationId::new(11)?),
        seed()?.key(),
        IncidentRevision::new(9)?,
        IncidentRevision::new(2)?,
        IncidentFileFacts::new([1; 24], [2; 32], 100)?,
        IncidentFileFacts::new([3; 24], [4; 32], 101)?,
        attachments,
        &[
            PayloadDiscardVictimFacts::new(AttachmentKind::Log, [5; 24], [5; 32], 7)?,
            PayloadDiscardVictimFacts::new(AttachmentKind::Minidump, [6; 24], [6; 32], 70_000)?,
        ],
        DiscardedPayload::new(PayloadDiscard::StoragePressure, 70_007)?,
    )?)
}
fn text() -> Result<String, Box<dyn Error>> {
    Ok(String::from_utf8(encode_store_state(&IncidentStoreState::PayloadDiscard(Box::new(
        journal()?,
    )))?)?)
}
fn construct(
    good: &PayloadDiscardJournal,
    revision: IncidentRevision,
    payload: IncidentRevision,
    original: IncidentFileFacts,
    candidate: IncidentFileFacts,
    victims: &[PayloadDiscardVictimFacts],
    disposition: DiscardedPayload,
) -> Result<PayloadDiscardJournal, IncidentStoreError> {
    PayloadDiscardJournal::new(
        good.operation(),
        good.incident(),
        revision,
        payload,
        original,
        candidate,
        good.original_payload().clone(),
        victims,
        disposition,
    )
}
#[test]
fn payload_discard_roundtrip_preserves_historical_key_immutable_generation_and_omission()
-> Result<(), Box<dyn Error>> {
    let journal = journal()?;
    let state = IncidentStoreState::PayloadDiscard(Box::new(journal.clone()));
    let encoded = encode_store_state(&state)?;
    assert_eq!(decode_store_state(&encoded)?, state);
    assert!(std::str::from_utf8(&encoded)?.starts_with(
        "{\"schema_version\":1,\"content\":{\"record\":\"payload_discard\",\"journal\":{"
    ));
    assert_eq!(journal.operation().owner(), owner(8)?);
    assert_eq!(journal.incident().subject().coordinator(), owner(2)?);
    assert_eq!(journal.original_revision().get(), 9);
    assert_eq!(journal.payload_revision().get(), 2);
    let attachments: Vec<_> = journal.original_payload().entries().collect();
    assert_eq!(attachments[0].original_bytes(), 900);
    assert_eq!(attachments[0].omitted_bytes(), 893);
    assert_eq!(journal.disposition().source_bytes(), 70_007);
    Ok(())
}
#[test]
fn payload_victim_has_separate_large_dump_bound_without_relaxing_metadata()
-> Result<(), Box<dyn Error>> {
    assert!(IncidentFileFacts::new([1; 24], [1; 32], 65_537).is_err());
    assert!(
        PayloadDiscardVictimFacts::new(
            AttachmentKind::Minidump,
            [1; 24],
            [1; 32],
            MAX_PAYLOAD_BYTES,
        )
        .is_ok()
    );
    for (kind, identity, sha, bytes) in [
        (AttachmentKind::Minidump, [0; 24], [1; 32], 1),
        (AttachmentKind::Minidump, [1; 24], [0; 32], 1),
        (AttachmentKind::Minidump, [1; 24], [1; 32], 0),
        (AttachmentKind::Minidump, [1; 24], [1; 32], MAX_PAYLOAD_BYTES + 1),
        (AttachmentKind::Log, [1; 24], [1; 32], MAX_TEXT_ATTACHMENT_BYTES + 1),
    ] {
        assert!(PayloadDiscardVictimFacts::new(kind, identity, sha, bytes).is_err());
    }
    Ok(())
}
#[test]
fn payload_discard_requires_complete_ordered_exact_victims() -> Result<(), Box<dyn Error>> {
    let good = journal()?;
    let victims: Vec<_> = good.victims().copied().collect();
    let changed = PayloadDiscardVictimFacts::new(AttachmentKind::Log, [5; 24], [7; 32], 7)?;
    let alias = PayloadDiscardVictimFacts::new(AttachmentKind::Minidump, [5; 24], [6; 32], 70_000)?;
    let summary_alias = PayloadDiscardVictimFacts::new(AttachmentKind::Log, [1; 24], [5; 32], 7)?;
    let length = PayloadDiscardVictimFacts::new(AttachmentKind::Log, [5; 24], [5; 32], 8)?;
    for rejected in [
        vec![],
        vec![victims[0]],
        vec![victims[1], victims[0]],
        vec![changed, victims[1]],
        vec![victims[0], alias],
        vec![summary_alias, victims[1]],
        vec![length, victims[1]],
        vec![victims[0], victims[1], victims[0]],
    ] {
        assert!(
            construct(
                &good,
                good.original_revision(),
                good.payload_revision(),
                good.original_file(),
                good.candidate_stage(),
                &rejected,
                good.disposition()
            )
            .is_err()
        );
    }
    Ok(())
}
#[test]
fn payload_discard_checks_revision_disposition_and_metadata_distinction()
-> Result<(), Box<dyn Error>> {
    let good = journal()?;
    let victims: Vec<_> = good.victims().copied().collect();
    for (revision, payload, original, candidate, disposition) in [
        (
            IncidentRevision::new(u64::MAX)?,
            good.payload_revision(),
            good.original_file(),
            good.candidate_stage(),
            good.disposition(),
        ),
        (
            good.original_revision(),
            IncidentRevision::new(10)?,
            good.original_file(),
            good.candidate_stage(),
            good.disposition(),
        ),
        (
            good.original_revision(),
            good.payload_revision(),
            good.original_file(),
            IncidentFileFacts::new([1; 24], [4; 32], 101)?,
            good.disposition(),
        ),
        (
            good.original_revision(),
            good.payload_revision(),
            good.original_file(),
            IncidentFileFacts::new([3; 24], [2; 32], 101)?,
            good.disposition(),
        ),
        (
            good.original_revision(),
            good.payload_revision(),
            good.original_file(),
            good.candidate_stage(),
            DiscardedPayload::new(PayloadDiscard::UserRequested, 70_007)?,
        ),
        (
            good.original_revision(),
            good.payload_revision(),
            good.original_file(),
            good.candidate_stage(),
            DiscardedPayload::new(PayloadDiscard::Oversized, 70_007)?,
        ),
        (
            good.original_revision(),
            good.payload_revision(),
            good.original_file(),
            good.candidate_stage(),
            DiscardedPayload::new(PayloadDiscard::StoragePressure, 900)?,
        ),
    ] {
        assert!(
            construct(&good, revision, payload, original, candidate, &victims, disposition)
                .is_err()
        );
    }
    Ok(())
}
#[test]
fn payload_discard_strict_schema_roles_unknown_fields_and_duplicates() -> Result<(), Box<dyn Error>>
{
    let original = text()?;
    for changed in [
        original.replace("\"schema_version\":1", "\"schema_version\":2"),
        original.replace("\"payload_discard\"", "\"payload_discard_future\""),
        original.replace("\"payload_revision\":2", "\"payload_revision\":0"),
        original.replace("\"payload_revision\":2,", ""),
        original.replace("\"payload_revision\":2", "\"payload_revision\":2,\"payload_revision\":2"),
        original
            .replace("\"original_revision\":9", "\"original_revision\":9,\"external_secret\":1"),
        original.replace("\"storage_pressure\"", "\"oversized\""),
        original.replace("\"retained\"", "\"quarantined\""),
        original.replace("\"discarded\"", "\"unavailable\""),
        original.replace("\"kind\":\"minidump\"", "\"kind\":\"future\""),
    ] {
        assert_ne!(changed, original);
        assert!(decode_store_state(changed.as_bytes()).is_err());
    }
    Ok(())
}
#[test]
fn payload_discard_decoder_bounds_victims_before_growth_and_rejects_aliases()
-> Result<(), Box<dyn Error>> {
    let original = text()?;
    let mut value: serde_json::Value = serde_json::from_str(&original)?;
    let victim = value["content"]["journal"]["victims"][0].clone();
    value["content"]["journal"]["victims"] = serde_json::Value::Array(vec![victim; 9]);
    assert!(decode_store_state(&serde_json::to_vec(&value)?).is_err());
    value = serde_json::from_str(&original)?;
    value["content"]["journal"]["victims"][1]["identity"] =
        value["content"]["journal"]["victims"][0]["identity"].clone();
    assert!(decode_store_state(&serde_json::to_vec(&value)?).is_err());
    assert!(decode_store_state(&vec![b' '; 65_537]).is_err());
    Ok(())
}
#[test]
fn payload_discard_rejects_foreign_namespace_but_preserves_historical_epoch()
-> Result<(), Box<dyn Error>> {
    let original = text()?;
    let mut value: serde_json::Value = serde_json::from_str(&original)?;
    value["content"]["journal"]["operation"]["owner"]["namespace"] = serde_json::json!(vec![9; 32]);
    assert!(decode_store_state(&serde_json::to_vec(&value)?).is_err());
    let acknowledged = original.replace("\"storage_pressure\"", "\"acknowledged_retention\"");
    assert!(decode_store_state(acknowledged.as_bytes()).is_ok());
    Ok(())
}
