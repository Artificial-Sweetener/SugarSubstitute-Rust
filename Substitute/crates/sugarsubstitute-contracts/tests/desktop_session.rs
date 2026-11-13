// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the public desktop-session boundary against SugarCubes' exact shared vectors.
use serde::Deserialize;
use std::error::Error;
use sugarsubstitute_contracts::desktop_session::{
    SessionError, admit_operation, decode_checkpoint, decode_message, encode_checkpoint,
    wire::{SessionCapabilities, SessionFence, SessionScope},
};

const VECTORS: &str = include_str!("../../../../Sugar/integrations/contracts/session_vectors.json");

#[derive(Deserialize)]
struct Vector {
    label: String,
    valid: bool,
    value: serde_json::Value,
    error: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Admission {
    label: String,
    message: usize,
    capabilities: SessionCapabilities,
    committed: bool,
    persistence: bool,
    current_fence: SessionFence,
    error: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Raw {
    label: String,
    kind: String,
    text: String,
    base_text: String,
    mutation: String,
    replace_from: Option<String>,
    replace_to: Option<String>,
    valid: bool,
}
#[derive(Deserialize)]
struct Vectors {
    scope: SessionScope,
    origin: String,
    checkpoints: Vec<Vector>,
    messages: Vec<Vector>,
    admissions: Vec<Admission>,
    raw: Vec<Raw>,
}

/// Compare the actionable admission reason with the portable machine-readable code.
fn code(error: SessionError) -> &'static str {
    match error {
        SessionError::Invalid => "invalid",
        SessionError::TooLarge => "too-large",
        SessionError::UntrustedOrigin => "untrusted-origin",
        SessionError::Stale => "stale",
        SessionError::Unsupported => "unsupported",
        SessionError::NotCommitted => "not-committed",
        SessionError::OptedOut => "opted-out",
    }
}

#[test]
fn shared_checkpoint_vectors_roundtrip_all_sections_and_reject_invalid_membership()
-> Result<(), Box<dyn Error>> {
    let vectors: Vectors = serde_json::from_str(VECTORS)?;
    assert_eq!(vectors.checkpoints.len(), 74);
    for vector in vectors.checkpoints {
        let bytes = serde_json::to_vec(&vector.value)?;
        let decoded = decode_checkpoint(&bytes);
        assert_eq!(decoded.is_ok(), vector.valid, "{}: {decoded:?}", vector.label);
        if let Ok(value) = decoded {
            let bytes = encode_checkpoint(&value)?;
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&bytes)?,
                serde_json::to_value(&value)?
            );
            assert_eq!(decode_checkpoint(&bytes)?, value);
        }
    }
    Ok(())
}

#[test]
fn shared_messages_cover_every_operation_error_acknowledgment_and_trust_refusal()
-> Result<(), Box<dyn Error>> {
    let vectors: Vectors = serde_json::from_str(VECTORS)?;
    assert_eq!(vectors.messages.len(), 49);
    for vector in vectors.messages {
        let decoded =
            decode_message(&serde_json::to_vec(&vector.value)?, &vectors.scope, &vectors.origin);
        assert_eq!(decoded.is_ok(), vector.valid, "{}: {decoded:?}", vector.label);
        if let Some(error) = vector.error {
            assert_eq!(
                decoded.as_ref().err().copied().map(code),
                Some(error.as_str()),
                "{}",
                vector.label
            );
        }
        if let Ok(value) = decoded {
            assert_eq!(serde_json::to_value(value)?, vector.value);
        }
    }
    Ok(())
}

#[test]
fn shared_capable_and_limited_hosts_enforce_commit_opt_out_and_revision_fences()
-> Result<(), Box<dyn Error>> {
    let vectors: Vectors = serde_json::from_str(VECTORS)?;
    assert_eq!(vectors.admissions.len(), 26);
    for vector in vectors.admissions {
        let value = vectors.messages.get(vector.message).ok_or("Missing admission message")?;
        let message =
            decode_message(&serde_json::to_vec(&value.value)?, &vectors.scope, &vectors.origin)?;
        let actual = admit_operation(
            &message,
            &vector.capabilities,
            vector.committed,
            vector.persistence,
            &vector.current_fence,
        );
        assert_eq!(actual.err().map(code), vector.error.as_deref(), "{}", vector.label);
    }
    Ok(())
}

#[test]
fn shared_raw_json_rejects_duplicates_escaped_duplicates_truncation_and_bad_unicode()
-> Result<(), Box<dyn Error>> {
    let vectors: Vectors = serde_json::from_str(VECTORS)?;
    assert_eq!(vectors.raw.len(), 8);
    for vector in vectors.raw {
        let baseline_valid = if vector.kind == "checkpoint" {
            decode_checkpoint(vector.base_text.as_bytes()).is_ok()
        } else {
            decode_message(vector.base_text.as_bytes(), &vectors.scope, &vectors.origin).is_ok()
        };
        assert!(baseline_valid, "{} must start from an admitted source", vector.label);
        let prefix = vector.base_text.strip_suffix('}').ok_or("Raw source must be an object")?;
        let expected = match vector.mutation.as_str() {
            "duplicate" | "escaped-duplicate" => {
                assert_eq!(
                    serde_json::from_str::<serde_json::Value>(&vector.text)?,
                    serde_json::from_str::<serde_json::Value>(&vector.base_text)?
                );
                let suffix = if vector.mutation == "duplicate" {
                    r#","version":1}"#
                } else {
                    r#","vers\u0069on":1}"#
                };
                format!("{prefix}{suffix}")
            }
            "truncated" => prefix.to_owned(),
            "unpaired-unicode" => {
                let from = vector.replace_from.as_deref().ok_or("Missing exact Unicode source")?;
                let to = vector.replace_to.as_deref().ok_or("Missing exact Unicode replacement")?;
                assert_eq!(vector.base_text.matches(from).count(), 1);
                vector.base_text.replacen(from, to, 1)
            }
            _ => return Err("Unknown raw rejection mutation".into()),
        };
        assert_eq!(vector.text, expected, "{} must apply only its intended mutation", vector.label);
        let valid = if vector.kind == "checkpoint" {
            decode_checkpoint(vector.text.as_bytes()).is_ok()
        } else {
            decode_message(vector.text.as_bytes(), &vectors.scope, &vectors.origin).is_ok()
        };
        assert_eq!(valid, vector.valid, "{}", vector.label);
    }
    Ok(())
}

/// A checkpoint the page captured must survive storage exactly: the store compares a saved
/// checkpoint with its re-decoded bytes, so a coordinate that changes by one unit in the last
/// place makes every later save fail.
#[test]
fn every_finite_graph_coordinate_survives_the_checkpoint_codec_exactly()
-> Result<(), Box<dyn Error>> {
    let vectors: Vectors = serde_json::from_str(VECTORS)?;
    let mut checkpoint = vectors
        .checkpoints
        .into_iter()
        .filter(|vector| vector.valid)
        .find_map(|vector| decode_checkpoint(&serde_json::to_vec(&vector.value).ok()?).ok())
        .ok_or("Missing valid checkpoint vector")?;
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    let mut changed = Vec::new();
    for _ in 0..2000 {
        state =
            state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        // Pan offsets and zoom scales of a real canvas: any magnitude up to a few thousand pixels.
        let coordinate = ((state >> 11) as f64 / (1_u64 << 53) as f64 - 0.5) * 8192.0;
        let entry = checkpoint.entries.first_mut().ok_or("Missing entry")?;
        entry.document.view.x = coordinate;
        let decoded = decode_checkpoint(&encode_checkpoint(&checkpoint)?)?;
        if decoded != checkpoint {
            changed.push(coordinate);
        }
    }
    assert!(
        changed.is_empty(),
        "{} coordinates changed, first {:?}",
        changed.len(),
        changed.first()
    );
    Ok(())
}
