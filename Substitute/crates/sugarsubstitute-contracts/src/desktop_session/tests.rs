// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify portable primitive admission and independent byte limits.
use super::*;
use sha2::{Digest, Sha256};

#[test]
fn namespace_is_below_installation_user_data_and_identities_remain_distinct()
-> Result<(), Box<dyn Error>> {
    let spelling = "00000000-0000-4000-8000-000000000002";
    let profile = ProfileId::new(spelling.to_owned())?;
    assert_eq!(session_directory(&profile), format!("sessions/{spelling}/"));
    assert!(ProfileId::new("../desktop-data".to_owned()).is_err());
    assert_eq!(session_directory(&ProfileId::new("real".to_owned())?), "sessions/real/");
    assert!(WorkspaceEntryId::new("00000000-0000-0000-0000-000000000000").is_err());
    assert!(OutputId::new("00000000-0000-4000-8000-00000000000A").is_err());
    Ok(())
}

#[test]
fn checkpoint_byte_admission_precedes_parsing() {
    assert_eq!(
        decode_checkpoint(&vec![b' '; MAX_CHECKPOINT_BYTES + 1]),
        Err(SessionError::TooLarge)
    );
}

#[test]
fn graph_admission_bounds_bytes_shape_and_depth_without_interpreting_comfy_semantics() {
    assert!(primitives::graph(r#"{"arbitraryComfyOwnedFacts":true}"#));
    assert!(!primitives::graph("[]"));
    assert!(!primitives::graph(r#"{"value":1e999}"#));
    assert!(!primitives::graph(&format!("{{\"value\":{}0{}}}", "[".repeat(65), "]".repeat(65))));
    assert!(!primitives::graph(&" ".repeat(MAX_DOCUMENT_BYTES + 1)));
}

#[test]
fn generated_session_decoder_fingerprints_the_retained_schema() -> Result<(), Box<dyn Error>> {
    let schema = include_str!("../../../../third_party/sugarcubes-session/session.schema.json")
        .replace("\r\n", "\n");
    let wire = include_str!("wire.rs");
    let recorded = wire
        .lines()
        .find_map(|line| line.strip_prefix("// Schema SHA-256 (LF): "))
        .ok_or("Missing schema hash")?;
    assert_eq!(recorded, format!("{:x}", Sha256::digest(schema.as_bytes())));
    Ok(())
}

/// Reuse the shared accepted occurrence when exercising complete document byte admission.
fn document_fixture() -> Result<wire::SessionDocument, Box<dyn Error>> {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../third_party/sugarcubes-session/session_vectors.json"
    ))?;
    Ok(serde_json::from_value(
        fixture["checkpoints"][0]["value"]["entries"][0]["document"].clone(),
    )?)
}

#[test]
fn document_combined_utf8_bound_precedes_current_and_baseline_graph_decoding()
-> Result<(), Box<dyn Error>> {
    let mut value = document_fixture()?;
    value.graph_json = "🍬".repeat(MAX_DOCUMENT_BYTES / 8);
    value.baseline_graph_json = Some(format!("{}🍬", value.graph_json));
    assert_eq!(validate_document(&value), Err(SessionError::TooLarge));
    Ok(())
}

#[test]
fn document_current_and_baseline_envelope_share_exact_32mib_admission() -> Result<(), Box<dyn Error>>
{
    let mut value = document_fixture()?;
    value.graph_json = r#"{"padding":""}"#.to_owned();
    value.baseline_graph_json = Some(r#"{"nodes":[]}"#.to_owned());
    let overhead = serde_json::to_vec(&value)?.len();
    value.graph_json = format!(r#"{{"padding":"{}"}}"#, "x".repeat(MAX_DOCUMENT_BYTES - overhead));
    assert_eq!(serde_json::to_vec(&value)?.len(), MAX_DOCUMENT_BYTES);
    assert_eq!(validate_document(&value), Ok(()));
    value.graph_json.insert(value.graph_json.len() - 2, 'x');
    assert_eq!(validate_document(&value), Err(SessionError::TooLarge));
    Ok(())
}

#[test]
fn escaped_unicode_in_current_or_baseline_graph_uses_encoded_document_budget()
-> Result<(), Box<dyn Error>> {
    for baseline in [false, true] {
        let mut value = document_fixture()?;
        value.graph_json = r#"{"nodes":[]}"#.to_owned();
        value.baseline_graph_json = Some(r#"{"nodes":[]}"#.to_owned());
        let empty = r#"{"padding":""}"#.to_owned();
        if baseline {
            value.baseline_graph_json = Some(empty);
        } else {
            value.graph_json = empty;
        }
        let available = MAX_DOCUMENT_BYTES - serde_json::to_vec(&value)?.len();
        let token = "\\ud83c\\udf6c";
        let encoded_token_bytes = serde_json::to_vec(&token)?.len() - 2;
        let padding = token.repeat(available / encoded_token_bytes)
            + &"x".repeat(available % encoded_token_bytes);
        let graph = format!(r#"{{"padding":"{padding}"}}"#);
        if baseline {
            value.baseline_graph_json = Some(graph.clone());
        } else {
            value.graph_json = graph.clone();
        }
        assert_eq!(serde_json::to_vec(&value)?.len(), MAX_DOCUMENT_BYTES);
        assert_eq!(validate_document(&value), Ok(()));
        let mut over = graph;
        over.insert(over.len() - 2, 'x');
        if baseline {
            value.baseline_graph_json = Some(over);
        } else {
            value.graph_json = over;
        }
        assert_eq!(validate_document(&value), Err(SessionError::TooLarge));
    }
    Ok(())
}
