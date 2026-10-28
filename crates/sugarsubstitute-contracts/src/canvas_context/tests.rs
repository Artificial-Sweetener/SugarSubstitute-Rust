// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the same context request and reply vectors used by SugarCubes.

use super::*;
use std::error::Error;

#[test]
fn generated_context_matches_shared_request_and_display_only_replies() -> Result<(), Box<dyn Error>>
{
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../third_party/sugarcubes-canvas/context_vectors.json"
    ))?;
    let request = CanvasContextRequest::decode(&serde_json::to_vec(&vectors["request"])?)?;
    assert_eq!(request.request_id(), 7);
    assert_eq!(request.session().as_str(), "session-12345678");
    for (capability, key) in [
        (CanvasDisplayCapability::Available, "available"),
        (CanvasDisplayCapability::Unavailable, "unavailable"),
    ] {
        let actual: serde_json::Value = serde_json::from_slice(&request.reply(capability)?)?;
        assert_eq!(actual, vectors[key]);
    }
    for value in vectors["invalidRequests"].as_array().ok_or("Invalid vectors")? {
        assert!(CanvasContextRequest::decode(&serde_json::to_vec(value)?).is_err());
    }
    let cancelled: serde_json::Value = serde_json::from_slice(&request.cancelled_reply()?)?;
    assert_eq!(cancelled, vectors["cancelled"]);
    let generated: wire::CanvasErrorReply = serde_json::from_value(cancelled)?;
    assert!(generated.valid());
    Ok(())
}

#[test]
fn context_cancellation_retains_its_original_correlation_and_error_contract()
-> Result<(), Box<dyn Error>> {
    let request = CanvasContextRequest::decode(br#"{"protocol":"sugarcubes.canvas","version":3,"session":"session-12345678","requestId":2147483647,"type":"context.request"}"#)?;
    let encoded = request.cancelled_reply()?;
    let mut reply: wire::CanvasErrorReply = serde_json::from_slice(&encoded)?;
    assert_eq!(reply.session, request.session().as_str());
    assert_eq!(reply.request_id, request.request_id());
    assert_eq!(reply.failure.status, "cancelled");
    assert!(reply.failure.message.is_empty());
    assert!(reply.valid());
    assert!(CanvasContextRequest::decode(&encoded).is_err());
    reply.failure.status = "unknown".into();
    assert!(!reply.valid());
    reply.failure.status = "cancelled".into();
    reply.failure.message = "🌈".repeat(256);
    assert!(reply.valid());
    reply.failure.message.push('x');
    assert!(!reply.valid());
    let duplicate = String::from_utf8(encoded)?
        .replace("\"requestId\":2147483647", "\"requestId\":2147483647,\"requestId\":1");
    assert!(serde_json::from_str::<wire::CanvasErrorReply>(&duplicate).is_err());
    Ok(())
}

#[test]
fn context_rejects_duplicate_fields_and_excessive_payloads() {
    let duplicate = br#"{"protocol":"sugarcubes.canvas","version":3,"session":"session-12345678","requestId":7,"requestId":8,"type":"context.request"}"#;
    assert!(CanvasContextRequest::decode(duplicate).is_err());
    assert!(matches!(
        CanvasContextRequest::decode(&vec![b' '; MAX_CANVAS_CONTEXT_BYTES + 1]),
        Err(CanvasContextError::TooLarge)
    ));
}

#[test]
fn generated_reply_requires_an_explicit_null_document() -> Result<(), Box<dyn Error>> {
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../third_party/sugarcubes-canvas/context_vectors.json"
    ))?;
    let mut reply = vectors["unavailable"].clone();
    reply["context"].as_object_mut().ok_or("Invalid reply context")?.remove("document");
    assert!(serde_json::from_value::<wire::CanvasNativeReadReply>(reply).is_err());
    Ok(())
}
