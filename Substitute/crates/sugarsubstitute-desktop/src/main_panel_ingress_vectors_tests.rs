// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route every shared SugarCubes contract vector through the host's framing and decoding.

use crate::main_panel_ingress::{BINDING_NAME, ClientMessage, decode_payload, frame};
use serde_json::{Value, json};
use std::error::Error;
use sugarsubstitute_contracts::panel::PanelSession;

/// The host's bound on one complete binding event.
const MAX_EVENT_BYTES: usize = 32 * 1024;

const CONTEXT: i64 = 17;

/// Embed one shared schema vector file retained beside the generated contract.
macro_rules! vectors {
    ($name:literal) => {
        serde_json::from_str::<Value>(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../Sugar/integrations/contracts/",
            $name
        )))
    };
}

/// Accept both bare messages and `{name, value}` cases, keeping their encoded bytes exact.
fn messages(file: &Value, key: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let entries = file.get(key).ok_or_else(|| format!("Missing vector set {key}"))?;
    let entries = match entries {
        Value::Array(entries) => entries.clone(),
        single => vec![single.clone()],
    };
    entries
        .into_iter()
        .map(|entry| match entry {
            Value::String(encoded) => Ok(encoded),
            Value::Object(ref case) if case.contains_key("value") => {
                Ok(serde_json::to_string(&case["value"])?)
            }
            message => Ok(serde_json::to_string(&message)?),
        })
        .collect()
}

/// Frame `payload` exactly as the injected bridge sends it on one lane and ticket.
fn event(lane: &str, payload: &str, context: i64) -> String {
    json!({"name": BINDING_NAME, "payload": format!("1:{lane}:1:{payload}"),
        "executionContextId": context})
    .to_string()
}

/// Decode through the same framing, context and session checks as the main actor.
fn routed(encoded: &str, context: Option<i64>, session: &PanelSession) -> Option<ClientMessage> {
    let frame = frame(encoded, context)?;
    let message = decode_payload(frame.payload(), session)?;
    (message.lane() == frame.lane).then_some(message)
}

/// Read the vector's own panel session so a valid message is otherwise exactly admissible.
fn session_of(message: &str) -> Result<PanelSession, Box<dyn Error>> {
    let value: Value = serde_json::from_str(message)?;
    let session = value["session"].as_str().ok_or("Vector has no session")?;
    Ok(PanelSession::new(session.to_owned())?)
}

/// A vector file's name, contents, admissible-exemplar key and invalid-message keys.
type VectorSet = (&'static str, Value, &'static str, &'static [&'static str]);

/// Every shared valid/invalid set with the key holding admissible exemplars.
fn sets() -> Result<[VectorSet; 6], Box<dyn Error>> {
    Ok([
        (
            "viewport",
            vectors!("canvas_surface_viewport_vectors.json")?,
            "validRequests",
            &["invalidRequests", "invalidRequestJson"],
        ),
        (
            "occlusion",
            vectors!("canvas_surface_occlusion_vectors.json")?,
            "validRequests",
            &["invalidRequests"],
        ),
        (
            "overview",
            vectors!("canvas_surface_overview_vectors.json")?,
            "validRequests",
            &["invalidRequests", "oversizedRequests"],
        ),
        (
            "preview",
            vectors!("canvas_surface_preview_vectors.json")?,
            "validRequests",
            &["invalidMessages"],
        ),
        (
            "discovery",
            vectors!("canvas_surface_discovery_vectors.json")?,
            "request",
            &["invalidRequests"],
        ),
        ("context", vectors!("canvas_context_vectors.json")?, "request", &["invalidRequests"]),
    ])
}

#[test]
fn shared_invalid_vectors_never_reach_application_routing() -> Result<(), Box<dyn Error>> {
    let mut checked = 0;
    for (name, file, valid, invalid) in sets()? {
        let admitted = messages(&file, valid)?;
        let session = session_of(admitted.first().ok_or("Missing valid exemplar")?)?;
        let foreign = PanelSession::new("foreign-12345678".into())?;
        for message in &admitted {
            let lane = if message.contains("\"type\":\"cancel\"") { "x" } else { "o" };
            let encoded = event(lane, message, CONTEXT);
            assert!(routed(&encoded, Some(CONTEXT), &session).is_some(), "{name}: {message}");
            assert!(routed(&encoded, Some(CONTEXT), &foreign).is_none(), "{name}: foreign session");
            assert!(routed(&encoded, Some(CONTEXT + 1), &session).is_none(), "{name}: context");
            assert!(routed(&encoded, None, &session).is_none(), "{name}: unattributed origin");
            assert!(routed(&event("c", message, CONTEXT), Some(CONTEXT), &session).is_none());
        }
        for key in invalid {
            for message in messages(&file, key)? {
                let encoded = event("o", &message, CONTEXT);
                assert!(routed(&encoded, Some(CONTEXT), &session).is_none(), "{name}.{key}");
                checked += 1;
            }
        }
    }
    let file = vectors!("canvas_surface_message_vectors.json")?;
    let base = file.get("base").ok_or("Missing message base")?;
    let session = session_of(&serde_json::to_string(base)?)?;
    for case in file["cases"].as_array().ok_or("Missing message cases")? {
        let mut value = base.clone();
        if let Some(path) = case["path"].as_str() {
            let parts: Vec<_> = path.split('.').collect();
            let (key, ancestors) = parts.split_last().ok_or("Empty vector path")?;
            let mut parent = &mut value;
            for part in ancestors {
                parent = parent.get_mut(*part).ok_or("Missing vector parent")?;
            }
            let object = parent.as_object_mut().ok_or("Vector parent is not an object")?;
            if case["remove"].as_bool() == Some(true) {
                object.remove(*key);
            } else {
                object.insert((*key).to_owned(), case["value"].clone());
            }
        }
        let encoded = event("o", &serde_json::to_string(&value)?, CONTEXT);
        let expected = case["valid"].as_bool().ok_or("Missing vector result")?;
        assert_eq!(routed(&encoded, Some(CONTEXT), &session).is_some(), expected, "{case}");
        checked += usize::from(!expected);
    }
    assert!(checked > 60, "Only {checked} invalid shared vectors were routed");
    Ok(())
}

#[test]
fn oversized_and_unknown_messages_are_inert_at_the_host_boundary() -> Result<(), Box<dyn Error>> {
    let session = PanelSession::new("session-12345678".into())?;
    let request = |kind: &str, extra: &str| {
        format!(
            r#"{{"protocol":"sugarcubes.canvas","version":3,"session":"session-12345678","requestId":1,"type":"{kind}","revision":1,"address":{{"sessionId":"canvas:w","viewId":"view:a","windowId":"window:a","attachmentId":"attachment:a","generation":1}}{extra}}}"#
        )
    };
    let prepared = event("o", &request("surface.prepare", ""), CONTEXT);
    assert!(routed(&prepared, Some(CONTEXT), &session).is_some());
    for kind in ["surface.unknown", "surface.prepare.extra", "context.reply", "surface.reply"] {
        let encoded = event("o", &request(kind, ""), CONTEXT);
        assert!(routed(&encoded, Some(CONTEXT), &session).is_none(), "{kind}");
    }
    let unknown_field =
        event("o", &request("surface.prepare", r#","grant":"filesystem""#), CONTEXT);
    assert!(routed(&unknown_field, Some(CONTEXT), &session).is_none());
    let padding = format!(r#","itemId":"{}""#, "x".repeat(16 * 1024));
    let oversized = event("o", &request("surface.prepare", &padding), CONTEXT);
    assert!(routed(&oversized, Some(CONTEXT), &session).is_none());
    let flooded = " ".repeat(MAX_EVENT_BYTES + 1);
    assert!(routed(&flooded, Some(CONTEXT), &session).is_none());
    Ok(())
}
