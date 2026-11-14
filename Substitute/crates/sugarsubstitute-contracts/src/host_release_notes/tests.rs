// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

use super::*;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const SESSION: &str = "session-12345678";
const VECTORS: &str =
    include_str!("../../../../../Sugar/integrations/contracts/release_notes_wire_vectors.json");
const SCHEMA: &str =
    include_str!("../../../../../Sugar/integrations/contracts/release-notes.schema.json");

fn session() -> Result<PanelSession, PanelError> {
    PanelSession::new(SESSION.into())
}

fn encoded(message: &ReleaseNotesHostMessage) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_slice(&message.encode(&session()?)?)?)
}

fn release(version: &str, notes: &str) -> Result<ReleaseNotesRelease, ReleaseNotesError> {
    ReleaseNotesRelease::new(ReleaseNotesVersion::new(version)?, notes.into())
}

#[test]
fn every_host_message_encodes_every_schema_field() -> TestResult {
    let envelope =
        json!({"protocol": "sugarcubes.release-notes", "version": 1, "session": SESSION});
    let with = |extra: Value| -> Result<Value, Box<dyn Error>> {
        let mut value = envelope.clone();
        let fields = value.as_object_mut().ok_or("Envelope is not an object")?;
        fields.extend(extra.as_object().ok_or("Extra is not an object")?.clone());
        Ok(value)
    };
    let request = ReleaseNotesRequestName::new("load-1")?;
    assert_eq!(
        encoded(&ReleaseNotesHostMessage::State {
            revision: 2,
            product: "SugarSubstitute".into(),
            current_version: ReleaseNotesVersion::new("0.4.2")?,
        })?,
        with(json!({"type": "state", "revision": 2, "product": "SugarSubstitute",
            "currentVersion": "0.4.2"}))?
    );
    assert_eq!(encoded(&ReleaseNotesHostMessage::Open)?, with(json!({"type": "open"}))?);
    assert_eq!(
        encoded(&ReleaseNotesHostMessage::Notes {
            request: request.clone(),
            releases: vec![release("0.4.2", "## 0.4.2\n")?, release("0.4.1", "## 0.4.1\n")?],
            omitted: 3,
        })?,
        with(json!({"type": "notes", "request": "load-1", "omitted": 3, "releases": [
            {"version": "0.4.2", "notes": "## 0.4.2\n"},
            {"version": "0.4.1", "notes": "## 0.4.1\n"},
        ]}))?
    );
    for (reason, token) in [
        (ReleaseNotesFailure::Unavailable, "unavailable"),
        (ReleaseNotesFailure::NotPublished, "not-published"),
    ] {
        assert_eq!(
            encoded(&ReleaseNotesHostMessage::Failed { request: request.clone(), reason })?,
            with(json!({"type": "failed", "request": "load-1", "reason": token}))?
        );
    }
    Ok(())
}

#[test]
fn messages_outside_the_schema_are_refused_before_they_are_sent() -> TestResult {
    let request = ReleaseNotesRequestName::new("load-1")?;
    let empty =
        ReleaseNotesHostMessage::Notes { request: request.clone(), releases: vec![], omitted: 0 };
    assert!(matches!(empty.encode(&session()?), Err(ReleaseNotesError::Invalid)));
    let many = ReleaseNotesHostMessage::Notes {
        request: request.clone(),
        releases: vec![release("1.0.0", "x")?; MAX_RELEASES + 1],
        omitted: 0,
    };
    assert!(matches!(many.encode(&session()?), Err(ReleaseNotesError::Invalid)));
    for revision in [0, 9_007_199_254_740_992] {
        let state = ReleaseNotesHostMessage::State {
            revision,
            product: "SugarSubstitute".into(),
            current_version: ReleaseNotesVersion::new("1.0.0")?,
        };
        assert!(matches!(state.encode(&session()?), Err(ReleaseNotesError::Invalid)));
    }
    let long_name = ReleaseNotesHostMessage::State {
        revision: 1,
        product: "x".repeat(MAX_PRODUCT_UNITS + 1),
        current_version: ReleaseNotesVersion::new("1.0.0")?,
    };
    assert!(matches!(long_name.encode(&session()?), Err(ReleaseNotesError::Invalid)));
    // Each release fits on its own, but together they exceed the message bound.
    let large = "x".repeat(MAX_NOTES_UNITS);
    let oversized = ReleaseNotesHostMessage::Notes {
        request,
        releases: vec![release("1.0.0", &large)?; 5],
        omitted: 0,
    };
    assert!(matches!(oversized.encode(&session()?), Err(ReleaseNotesError::TooLarge)));
    Ok(())
}

#[test]
fn notes_and_tokens_hold_the_schema_bounds() -> TestResult {
    assert!(release("1.0.0", &"🙂".repeat(MAX_NOTES_UNITS / 2)).is_ok());
    assert!(release("1.0.0", &"x".repeat(MAX_NOTES_UNITS + 1)).is_err());
    assert!(ReleaseNotesVersion::new(&"1".repeat(64)).is_ok());
    for version in ["", "1.0.0+build", "V1", &"1".repeat(65)] {
        assert!(ReleaseNotesVersion::new(version).is_err(), "{version}");
    }
    assert!(ReleaseNotesRequestName::new(&"a".repeat(49)).is_err());
    Ok(())
}

#[test]
fn page_requests_decode_for_their_session() -> TestResult {
    let watch = br#"{"protocol":"sugarcubes.release-notes","version":1,"session":"session-12345678","type":"watch"}"#;
    assert_eq!(
        ReleaseNotesClientMessage::decode(watch)?,
        ReleaseNotesClientMessage::Watch(session()?)
    );
    let load = br#"{"protocol":"sugarcubes.release-notes","version":1,"session":"session-12345678","type":"load","request":"load-2","since":"0.3.0"}"#;
    let message = ReleaseNotesClientMessage::decode(load)?;
    assert_eq!(
        message,
        ReleaseNotesClientMessage::Load {
            session: session()?,
            request: ReleaseNotesRequestName::new("load-2")?,
            since: ReleaseNotesVersion::new("0.3.0")?,
        }
    );
    assert_eq!(message.session(), &session()?);
    let oversized = vec![b' '; MAX_RELEASE_NOTES_MESSAGE_BYTES + 1];
    assert!(matches!(
        ReleaseNotesClientMessage::decode(&oversized),
        Err(ReleaseNotesError::TooLarge)
    ));
    Ok(())
}

#[test]
fn the_shared_vectors_hold_in_both_directions() -> TestResult {
    let vectors: Vec<Value> = serde_json::from_str(VECTORS)?;
    assert!(!vectors.is_empty());
    for vector in &vectors {
        let label = vector["label"].as_str().ok_or("Vector has no label")?;
        let valid = vector["valid"].as_bool().ok_or("Vector has no verdict")?;
        let bytes = serde_json::to_vec(&vector["value"])?;
        let host =
            matches!(vector["value"]["type"].as_str(), Some("state" | "open" | "notes" | "failed"));
        let as_host = serde_json::from_slice::<wire::ReleaseNotesHostMessage>(&bytes)
            .is_ok_and(|message| message.valid());
        let as_client = ReleaseNotesClientMessage::decode(&bytes).is_ok();
        assert_eq!((label, as_host), (label, valid && host));
        assert_eq!((label, as_client), (label, valid && !host));
    }
    Ok(())
}

#[test]
fn the_generated_decoder_identifies_the_shared_schema() -> TestResult {
    use sha2::{Digest, Sha256};
    let schema = SCHEMA.replace("\r\n", "\n");
    let source = include_str!(
        "../../../../../Sugar/integrations/contracts/ReleaseNotesProtocol.generated.rs"
    );
    let recorded = source
        .lines()
        .find_map(|line| line.strip_prefix("// Schema SHA-256 (LF): "))
        .ok_or("Generated release notes schema fingerprint absent")?;
    assert_eq!(recorded, format!("{:x}", Sha256::digest(schema.as_bytes())));
    Ok(())
}

#[test]
fn the_named_bounds_match_the_shared_schema() -> TestResult {
    let schema: Value = serde_json::from_str(SCHEMA)?;
    let definitions = &schema["$defs"];
    let notes = &definitions["ReleaseNotesNotesMessage"]["properties"]["releases"];
    assert_eq!(notes["maxItems"], MAX_RELEASES);
    assert_eq!(
        definitions["ReleaseNotesRelease"]["properties"]["notes"]["maxLength"],
        MAX_NOTES_UNITS
    );
    assert_eq!(
        definitions["ReleaseNotesStateMessage"]["properties"]["product"]["maxLength"],
        MAX_PRODUCT_UNITS
    );
    for message in [
        "ReleaseNotesStateMessage",
        "ReleaseNotesOpenMessage",
        "ReleaseNotesNotesMessage",
        "ReleaseNotesFailedMessage",
        "ReleaseNotesWatchMessage",
        "ReleaseNotesLoadMessage",
    ] {
        let envelope = &definitions[message]["properties"];
        assert_eq!(envelope["protocol"]["const"], PROTOCOL, "{message}");
        assert_eq!(envelope["version"]["const"], VERSION, "{message}");
    }
    Ok(())
}
