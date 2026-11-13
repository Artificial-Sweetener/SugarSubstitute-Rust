// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Test canonical historical facts without manufacturing protected read or grant authority.
use super::*;
use crate::incident::{IncidentStoreState, decode_store_state, encode_store_state};
use std::error::Error;
use sugarsubstitute_contracts::{
    incident::IncidentOperationId, lifecycle::*, recovery_supervisor::RecoveryGrantId,
};
fn record() -> Result<RecoveryCaptureOriginRecord, Box<dyn Error>> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let run = RunKey { owner, instance: InstanceId::new(3)?, generation: RunGeneration::new(4)? };
    Ok(RecoveryCaptureOriginRecord::new(
        sugarsubstitute_contracts::incident::LogicalStoreOperationKey::new(
            owner,
            IncidentOperationId::new(5)?,
        ),
        RecoveryCaptureIdentity::new(run, RecoveryGrantId::new(6)?),
        [7; 24],
        &BuildIdentity::new("historical-build")?,
    )?)
}
#[test]
fn recovery_origin_journal_roundtrip_is_bounded_schema_two_and_preserves_full_identity()
-> Result<(), Box<dyn Error>> {
    let record = record()?;
    let state = IncidentStoreState::RecoveryCaptureOrigin(record.clone());
    let bytes = encode_store_state(&state)?;
    assert!(bytes.len() <= 4096);
    assert_eq!(decode_store_state(&bytes)?, state);
    assert_eq!(encode_store_state(&decode_store_state(&bytes)?)?, bytes);
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(value["schema_version"], 2);
    assert_eq!(value["content"]["record"], "recovery_capture_origin");
    assert_eq!(record.origin().run().owner, record.operation().owner());
    assert_eq!(record.root_identity(), [7; 24]);
    assert_eq!(record.build().as_str(), "historical-build");
    Ok(())
}
#[test]
fn recovery_origin_journal_rejects_mixed_keys_zero_grants_unknowns_duplicates_and_schema_one()
-> Result<(), Box<dyn Error>> {
    let bytes = encode_store_state(&IncidentStoreState::RecoveryCaptureOrigin(record()?))?;
    let original: serde_json::Value = serde_json::from_slice(&bytes)?;
    for path in [
        "/content/journal/origin/grant",
        "/content/journal/origin/instance",
        "/content/journal/origin/generation",
    ] {
        let mut changed = original.clone();
        *changed.pointer_mut(path).ok_or("origin field absent")? = serde_json::json!(0);
        assert!(decode_store_state(&serde_json::to_vec(&changed)?).is_err());
    }
    for path in ["/content/journal/capture_key/epoch", "/content/journal/origin/root/epoch"] {
        let mut changed = original.clone();
        *changed.pointer_mut(path).ok_or("origin field absent")? = serde_json::json!(vec![9; 16]);
        assert!(decode_store_state(&serde_json::to_vec(&changed)?).is_err());
    }
    let mut changed = original.clone();
    changed["schema_version"] = serde_json::json!(1);
    assert!(decode_store_state(&serde_json::to_vec(&changed)?).is_err());
    for path in ["/content/journal", "/content/journal/origin", "/content/journal/origin/root"] {
        let mut changed = original.clone();
        changed
            .pointer_mut(path)
            .and_then(serde_json::Value::as_object_mut)
            .ok_or("origin object absent")?
            .insert("unknown".into(), serde_json::json!(1));
        assert!(decode_store_state(&serde_json::to_vec(&changed)?).is_err());
    }
    let text = String::from_utf8(bytes)?;
    assert!(
        decode_store_state(text.replace("\"grant\":6", "\"grant\":6,\"grant\":6").as_bytes())
            .is_err()
    );
    Ok(())
}
#[test]
fn recovery_origin_record_cannot_mix_original_root_or_missing_actual_root_identity()
-> Result<(), Box<dyn Error>> {
    let record = record()?;
    let owner = record.operation.owner();
    let other = CoordinatorKey { epoch: CoordinatorEpoch::new([9; 16])?, ..owner };
    let operation = sugarsubstitute_contracts::incident::LogicalStoreOperationKey::new(
        other,
        record.operation.id(),
    );
    assert!(
        RecoveryCaptureOriginRecord::new(
            operation,
            record.origin,
            record.root_identity,
            &record.build
        )
        .is_err()
    );
    assert!(
        RecoveryCaptureOriginRecord::new(record.operation, record.origin, [0; 24], &record.build)
            .is_err()
    );
    Ok(())
}

#[test]
fn recovery_origin_requires_recorded_producer_build_and_never_defaults_missing_or_invalid_build()
-> Result<(), Box<dyn Error>> {
    let bytes = encode_store_state(&IncidentStoreState::RecoveryCaptureOrigin(record()?))?;
    let original: serde_json::Value = serde_json::from_slice(&bytes)?;
    for bad in [
        serde_json::json!(""),
        serde_json::json!("a".repeat(65)),
        serde_json::json!("build/secret"),
        serde_json::Value::Null,
    ] {
        let mut changed = original.clone();
        changed["content"]["journal"]["build"] = bad;
        assert!(decode_store_state(&serde_json::to_vec(&changed)?).is_err());
    }
    let mut missing = original;
    missing["content"]["journal"].as_object_mut().ok_or("Origin journal missing")?.remove("build");
    assert!(decode_store_state(&serde_json::to_vec(&missing)?).is_err());
    let text = String::from_utf8(bytes)?;
    assert!(
        decode_store_state(
            text.replace(
                "\"build\":\"historical-build\"",
                "\"build\":\"historical-build\",\"build\":\"historical-build\""
            )
            .as_bytes()
        )
        .is_err()
    );
    Ok(())
}
