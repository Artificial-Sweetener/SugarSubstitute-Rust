// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the public desktop-session boundary against SugarCubes' exact shared vectors.
use serde::Deserialize;
use std::error::Error;
use sugarsubstitute_contracts::desktop_session::{
    SessionError, admit_operation, decode_checkpoint, decode_message, encode_checkpoint,
    wire::{SessionCapabilities, SessionFence, SessionScope},
};

const VECTORS: &str = include_str!("../../../third_party/sugarcubes-session/session_vectors.json");

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
