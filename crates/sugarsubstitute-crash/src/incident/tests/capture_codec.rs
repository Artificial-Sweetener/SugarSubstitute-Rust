// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise strict external capture intent bytes without constructing native authority.
use super::*;
use serde_json::{Value, json};

#[cfg(windows)]
#[test]
fn historical_replay_moves_exact_decoded_plan_and_keeps_claim_after_rejection()
-> Result<(), Box<dyn Error>> {
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    use sugarsubstitute_contracts::execution::{MemoryLease, MemoryRelease};
    struct Release(Arc<AtomicU64>);
    impl MemoryRelease for Release {
        fn release(&self, bytes: u64) {
            self.0.fetch_add(bytes, Ordering::SeqCst);
        }
    }
    let released = Arc::new(AtomicU64::new(0));
    let bytes = HistoricalCaptureRecovery::required_storage_bytes();
    let lease = MemoryLease::new(bytes, Box::new(Release(released.clone())));
    let mut recovery = HistoricalCaptureRecovery::new(lease.claim(bytes)?)?;
    let ledger = SummaryEvictions::new(owner(2)?.namespace, 0, 0, 0, None)?;
    assert!(recovery.retain_replay(IncidentStoreState::Ledger(ledger)).is_err());
    assert!(recovery.replay.is_none());
    assert!(lease.claim(1).is_err());
    let encoded =
        encode_store_state(&IncidentStoreState::CapturePromotion(Box::new(capture_plan(false)?)))?;
    let decoded = decode_store_state(&encoded)?;
    let IncidentStoreState::CapturePromotion(plan) = &decoded else {
        return Err("Decoded promotion changed record kind".into());
    };
    let original = std::ptr::from_ref(plan.as_ref());
    recovery.retain_replay(decoded)?;
    assert!(std::ptr::eq(original, recovery.replay.as_deref().ok_or("Retained plan absent")?));
    assert!(recovery.retain_replay(decode_store_state(&encoded)?).is_err());
    assert!(recovery.reset().is_err());
    assert!(std::ptr::eq(original, recovery.replay.as_deref().ok_or("Original plan replaced")?));
    assert!(lease.claim(1).is_err());
    assert_eq!(released.load(Ordering::SeqCst), 0);
    drop(recovery);
    assert_eq!(lease.claim(bytes)?.bytes(), bytes);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    drop(lease);
    assert_eq!(released.load(Ordering::SeqCst), bytes);
    Ok(())
}

pub(super) fn capture_plan(discarded: bool) -> Result<CaptureDispositionPlan, Box<dyn Error>> {
    let source = CaptureRecordedFileFacts::new([3; 24], [3; 32], 512)?;
    let summary = CaptureRecordedFileFacts::new([8; 24], [8; 32], 64)?;
    Ok(CaptureDispositionPlan::new(
        LogicalStoreOperationKey::new(owner(9)?, IncidentOperationId::new(1)?),
        seed()?.key(),
        IncidentRevision::new(3)?,
        sugarsubstitute_contracts::crash::CaptureGenerationOrigin::Coordinator(owner(2)?),
        DumpId::new([7; 16])?,
        BuildIdentity::new("build_1")?,
        [10; 24],
        [11; 24],
        source,
        CaptureRecordedFileFacts::new([4; 24], [5; 32], 256)?,
        CaptureRecordedFileFacts::new([5; 24], [5; 32], 256)?,
        CaptureRecordedFileFacts::new([6; 24], [7; 32], 200)?,
        if discarded {
            CaptureDispositionCompletion::Discarded {
                summary_file: summary,
                reason: PayloadDiscard::UserRequested,
                source_bytes: 512,
            }
        } else {
            CaptureDispositionCompletion::Promoted {
                summary_file: summary,
                payload_file: CaptureRecordedFileFacts::new(
                    [9; 24],
                    source.sha256(),
                    source.bytes(),
                )?,
            }
        },
    )?)
}
fn repeated(value: u8, length: usize) -> Value {
    json!(vec![value; length])
}
fn record(discarded: bool) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_slice(&encode_store_state(&IncidentStoreState::CapturePromotion(
        Box::new(capture_plan(discarded)?),
    ))?)?)
}
fn rejected(record: Value) -> Result<(), Box<dyn Error>> {
    assert!(decode_store_state(&serde_json::to_vec(&record)?).is_err());
    Ok(())
}

#[test]
fn capture_roundtrip_preserves_full_historical_identity_and_both_completions()
-> Result<(), Box<dyn Error>> {
    for discarded in [false, true] {
        let plan = capture_plan(discarded)?;
        assert_ne!(plan.historical_operation().owner(), plan.capture_generation());
        assert_eq!(plan.capture_generation(), plan.incident().subject().coordinator());
        assert_eq!(plan.incident_revision().get(), 3);
        assert_eq!(plan.uuid(), DumpId::new([7; 16])?);
        assert_eq!(plan.build().as_str(), "build_1");
        assert_eq!(plan.database_identity(), [10; 24]);
        assert_eq!(plan.metrics_identity(), [11; 24]);
        let state = IncidentStoreState::CapturePromotion(Box::new(plan));
        let bytes = encode_store_state(&state)?;
        assert_eq!(decode_store_state(&bytes)?, state);
        assert_eq!(encode_store_state(&decode_store_state(&bytes)?)?, bytes);
        let value: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(value["content"]["record"], "capture_promotion");
        assert!(value["content"]["journal"].get("current_operation").is_none());
        assert!(value["content"]["journal"].get("journal_sha256").is_none());
    }
    Ok(())
}
#[test]
fn capture_unknown_missing_and_duplicate_fields_reject_at_each_record_boundary()
-> Result<(), Box<dyn Error>> {
    for pointer in [
        "",
        "/content",
        "/content/journal",
        "/content/journal/source",
        "/content/journal/original_stage",
        "/content/journal/completion",
        "/content/journal/historical_operation/owner",
    ] {
        let mut unknown = record(false)?;
        unknown
            .pointer_mut(pointer)
            .and_then(Value::as_object_mut)
            .ok_or("Missing record boundary")?
            .insert("PRIVATE_CAPTURE_FIELD".into(), json!(1));
        rejected(unknown)?;
    }
    for field in [
        "historical_operation",
        "incident",
        "incident_revision",
        "capture_generation",
        "uuid",
        "build",
        "database_identity",
        "metrics_identity",
        "source",
        "metadata_preimage",
        "original_stage",
        "candidate_stage",
        "completion",
    ] {
        let mut missing = record(false)?;
        missing["content"]["journal"].as_object_mut().ok_or("Missing journal")?.remove(field);
        rejected(missing)?;
    }
    let text = String::from_utf8(encode_store_state(&IncidentStoreState::CapturePromotion(
        Box::new(capture_plan(false)?),
    ))?)?;
    assert!(
        decode_store_state(
            text.replace(
                "\"incident_revision\":3",
                "\"incident_revision\":3,\"incident_revision\":3"
            )
            .as_bytes()
        )
        .is_err()
    );
    Ok(())
}
#[test]
fn capture_namespace_generation_uuid_and_positive_ids_do_not_default() -> Result<(), Box<dyn Error>>
{
    for (pointer, invalid) in [
        ("/content/journal/capture_generation/epoch", repeated(8, 16)),
        ("/content/journal/historical_operation/owner/namespace", repeated(2, 32)),
        ("/content/journal/historical_operation/owner/epoch", repeated(0, 16)),
        ("/content/journal/historical_operation/id", json!(0)),
        ("/content/journal/incident_revision", json!(0)),
        ("/content/journal/uuid", repeated(0, 16)),
        ("/content/journal/database_identity", repeated(0, 24)),
        ("/content/journal/metrics_identity", repeated(0, 24)),
        ("/content/journal/build", json!("PRIVATE/path")),
    ] {
        let mut changed = record(false)?;
        *changed.pointer_mut(pointer).ok_or("Missing identity")? = invalid;
        rejected(changed)?;
    }
    Ok(())
}
#[test]
fn capture_stage_roles_preimage_digest_and_exact_record_subtraction_are_checked()
-> Result<(), Box<dyn Error>> {
    for (pointer, invalid) in [
        ("/content/journal/original_stage/role", json!("candidate_metadata")),
        ("/content/journal/candidate_stage/role", json!("original_metadata")),
        ("/content/journal/candidate_stage/role", json!("PRIVATE_STAGE_ROLE")),
        ("/content/journal/original_stage/sha256", repeated(6, 32)),
        ("/content/journal/original_stage/bytes", json!(255)),
        ("/content/journal/candidate_stage/bytes", json!(199)),
        ("/content/journal/candidate_stage/sha256", repeated(5, 32)),
    ] {
        let mut changed = record(false)?;
        *changed.pointer_mut(pointer).ok_or("Missing stage field")? = invalid;
        rejected(changed)?;
    }
    for (pointer, reused) in [
        ("/content/journal/original_stage/identity", 3),
        ("/content/journal/original_stage/identity", 4),
        ("/content/journal/original_stage/identity", 6),
        ("/content/journal/candidate_stage/identity", 3),
        ("/content/journal/candidate_stage/identity", 4),
    ] {
        let mut changed = record(false)?;
        *changed.pointer_mut(pointer).ok_or("Missing stage identity")? = repeated(reused, 24);
        rejected(changed)?;
    }
    Ok(())
}
#[test]
fn capture_file_facts_apply_payload_summary_and_metadata_bounds() -> Result<(), Box<dyn Error>> {
    for pointer in [
        "/source",
        "/metadata_preimage",
        "/original_stage",
        "/candidate_stage",
        "/completion/summary_file",
        "/completion/payload_file",
    ] {
        for field in ["identity", "sha256", "bytes"] {
            let mut changed = record(false)?;
            let target = format!("/content/journal{pointer}/{field}");
            *changed.pointer_mut(&target).ok_or("Missing file fact")? = match field {
                "identity" => repeated(0, 24),
                "sha256" => repeated(0, 32),
                _ => json!(0),
            };
            rejected(changed)?;
        }
    }
    for (pointer, bytes) in [
        ("/source/bytes", MAX_PAYLOAD_BYTES + 1),
        ("/completion/payload_file/bytes", MAX_PAYLOAD_BYTES + 1),
        ("/completion/summary_file/bytes", 65_537),
        ("/metadata_preimage/bytes", 1_048_577),
        ("/original_stage/bytes", 1_048_577),
        ("/candidate_stage/bytes", 1_048_577),
    ] {
        let mut changed = record(false)?;
        *changed.pointer_mut(&format!("/content/journal{pointer}")).ok_or("Missing length")? =
            json!(bytes);
        rejected(changed)?;
    }
    let mut exact = record(false)?;
    exact["content"]["journal"]["source"]["bytes"] = json!(MAX_PAYLOAD_BYTES);
    exact["content"]["journal"]["completion"]["payload_file"]["bytes"] = json!(MAX_PAYLOAD_BYTES);
    exact["content"]["journal"]["metadata_preimage"]["bytes"] = json!(1_048_576);
    exact["content"]["journal"]["original_stage"]["bytes"] = json!(1_048_576);
    exact["content"]["journal"]["candidate_stage"]["bytes"] = json!(1_048_520);
    exact["content"]["journal"]["completion"]["summary_file"]["bytes"] = json!(65_536);
    assert!(decode_store_state(&serde_json::to_vec(&exact)?).is_ok());
    Ok(())
}
#[test]
fn capture_completion_must_match_exact_source_and_closed_discard_shape()
-> Result<(), Box<dyn Error>> {
    for (pointer, invalid) in [
        ("/content/journal/completion/payload_file/sha256", repeated(4, 32)),
        ("/content/journal/completion/payload_file/bytes", json!(511)),
        ("/content/journal/completion/disposition", json!("PRIVATE_COMPLETION")),
    ] {
        let mut changed = record(false)?;
        *changed.pointer_mut(pointer).ok_or("Missing completion")? = invalid;
        rejected(changed)?;
    }
    for (pointer, invalid) in [
        ("/content/journal/completion/source_bytes", json!(511)),
        ("/content/journal/completion/reason", json!("PRIVATE_DISCARD")),
    ] {
        let mut changed = record(true)?;
        *changed.pointer_mut(pointer).ok_or("Missing discard")? = invalid;
        rejected(changed)?;
    }
    let mut mixed = record(true)?;
    mixed["content"]["journal"]["completion"]["payload_file"] = json!({});
    rejected(mixed)?;
    Ok(())
}
#[test]
fn capture_schema_and_byte_bound_preserve_rejection_without_authority() -> Result<(), Box<dyn Error>>
{
    let mut changed = record(false)?;
    changed["schema_version"] = json!(2);
    assert!(matches!(
        decode_store_state(&serde_json::to_vec(&changed)?),
        Err(IncidentStoreError::UnsupportedSchema(2))
    ));
    assert!(matches!(
        decode_store_state(&vec![b' '; 65_537]),
        Err(IncidentStoreError::RecordBound)
    ));
    let mut forged = record(false)?;
    forged["content"]["journal"]["current_operation"] = json!({});
    rejected(forged)?;
    Ok(())
}

#[test]
fn capture_journal_accepts_only_the_pinned_producer_record_removal() -> Result<(), Box<dyn Error>> {
    for discarded in [false, true] {
        let canonical = record(discarded)?;
        assert!(decode_store_state(&serde_json::to_vec(&canonical)?).is_ok());
        for removed in [0_u64, 55, 64] {
            let mut unsupported = canonical.clone();
            unsupported["content"]["journal"]["candidate_stage"]["bytes"] = json!(256 - removed);
            let bytes = serde_json::to_vec(&unsupported)?;
            assert!(decode_store_state(&bytes).is_err());
        }
    }
    Ok(())
}
