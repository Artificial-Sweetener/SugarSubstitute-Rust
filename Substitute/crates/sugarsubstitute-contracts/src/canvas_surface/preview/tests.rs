// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify shared preview schemas and retain attachment, operation and lease identities.

use super::*;
use crate::canvas_surface::CanvasSurfaceMessage;
use serde_json::{Value, json};
use std::error::Error;

pub(crate) fn vectors() -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../Sugar/integrations/contracts/canvas_surface_preview_vectors.json"
    )))?)
}

fn request(value: &Value) -> Result<Box<CanvasSurfaceRequest>, Box<dyn Error>> {
    match CanvasSurfaceMessage::decode(&serde_json::to_vec(value)?)? {
        CanvasSurfaceMessage::Request(request) => Ok(request),
        _ => Err("Expected preview attachment request".into()),
    }
}

#[test]
fn shared_preview_requests_preserve_the_original_lease_across_later_revisions()
-> Result<(), Box<dyn Error>> {
    let vectors = vectors()?;
    for value in vectors["validRequests"].as_array().ok_or("Missing requests")? {
        let decoded = CanvasSurfaceMessage::decode(&serde_json::to_vec(value)?)?;
        assert_eq!(decoded.session().as_str(), value["session"].as_str().ok_or("Missing session")?);
        let CanvasSurfaceMessage::Request(decoded) = decoded else {
            continue;
        };
        assert_eq!(decoded.request_id(), 7);
        assert_eq!(decoded.address().attachment_id(), "attachment:one");
        match decoded.operation() {
            CanvasSurfaceOperation::PreviewBegin(begin) => {
                let identity = begin.identity();
                assert_eq!(identity.generation(), 0);
                assert_eq!(identity.prompt_id(), "prompt:one");
                assert_eq!(identity.source_id(), "source:10:20");
                assert_eq!(identity.run_order(), 1);
                assert_eq!(identity.sequence(), 1);
                assert_eq!(identity.token(), "preview:1");
                assert_eq!(begin.byte_length(), 3);
                let format = match value["format"].as_str().ok_or("Missing preview format")? {
                    "png" => CanvasSurfacePreviewFormat::Png,
                    "jpeg" => CanvasSurfacePreviewFormat::Jpeg,
                    _ => return Err("Unexpected fixture preview format".into()),
                };
                assert_eq!(begin.format(), format);
                assert_eq!(decoded.revision(), 3);
            }
            CanvasSurfaceOperation::PreviewChunk(chunk) => {
                assert_eq!(chunk.token(), "preview:1");
                assert_eq!(chunk.lease_revision(), 3);
                assert_eq!(chunk.offset(), 0);
                assert_eq!(chunk.data(), &[1, 2, 3]);
                assert_eq!(decoded.revision(), 4);
            }
            CanvasSurfaceOperation::PreviewEnd(lease)
            | CanvasSurfaceOperation::PreviewWithdraw(lease) => {
                assert_eq!(lease.token(), "preview:1");
                assert_eq!(lease.lease_revision(), 3);
                assert!(decoded.revision() > lease.lease_revision());
            }
            _ => return Err("Unexpected fixture operation".into()),
        }
    }
    Ok(())
}

#[test]
fn shared_preview_invalid_messages_and_extended_shapes_are_rejected() -> Result<(), Box<dyn Error>>
{
    let vectors = vectors()?;
    for value in vectors["invalidMessages"].as_array().ok_or("Missing invalid messages")? {
        assert!(CanvasSurfaceMessage::decode(&serde_json::to_vec(value)?).is_err(), "{value}");
    }
    for value in vectors["validRequests"].as_array().ok_or("Missing requests")? {
        let mut extended = value.clone();
        extended["extra"] = json!(true);
        assert!(CanvasSurfaceMessage::decode(&serde_json::to_vec(&extended)?).is_err());
    }
    let mut value = vectors["validRequests"][0].clone();
    value["preview"]["extra"] = json!(true);
    assert!(CanvasSurfaceMessage::decode(&serde_json::to_vec(&value)?).is_err());
    assert!(
        matches!(CanvasSurfaceMessage::decode(b"{"), Err(CanvasSurfaceMessageError::Malformed(cause)) if cause.is_syntax() || cause.is_eof())
    );
    Ok(())
}

#[test]
fn preview_metadata_and_byte_length_bounds_match_shared_schema() -> Result<(), Box<dyn Error>> {
    let vectors = vectors()?;
    let base = vectors["validRequests"][0].clone();
    for (field, prefix, maximum) in [("promptId", "prompt:", 256), ("sourceId", "source:", 4096)] {
        let mut value = base.clone();
        value["preview"][field] = json!(format!("{prefix}{}", "é".repeat(maximum - prefix.len())));
        assert!(request(&value).is_ok(), "{field}");
        value["preview"][field] =
            json!(format!("{prefix}{}", "é".repeat(maximum + 1 - prefix.len())));
        assert!(request(&value).is_err(), "{field}");
    }
    for field in ["runOrder", "sequence"] {
        let mut value = base.clone();
        value["preview"][field] = json!(0);
        assert!(request(&value).is_err());
        value["preview"][field] = json!(9_007_199_254_740_991_u64);
        if field == "sequence" {
            value["preview"]["token"] = json!("preview:9007199254740991");
        }
        assert!(request(&value).is_ok());
    }
    let mut value = base;
    value["byteLength"] = json!(MAX_SURFACE_PREVIEW_BYTES);
    assert!(request(&value).is_ok());
    value["byteLength"] = json!(MAX_SURFACE_PREVIEW_BYTES + 1);
    assert!(request(&value).is_err());
    value["byteLength"] = json!(1);
    value["format"] = json!("webp");
    assert!(request(&value).is_err());
    Ok(())
}

#[test]
fn preview_begin_preserves_each_shared_encoding_for_runtime_capability_admission()
-> Result<(), Box<dyn Error>> {
    let vectors = vectors()?;
    let base = vectors["validRequests"][0].clone();
    for (format, expected) in
        [("png", CanvasSurfacePreviewFormat::Png), ("jpeg", CanvasSurfacePreviewFormat::Jpeg)]
    {
        let mut value = base.clone();
        value["format"] = json!(format);
        let decoded = request(&value)?;
        let CanvasSurfaceOperation::PreviewBegin(begin) = decoded.operation() else {
            return Err("Expected begin declaration".into());
        };
        assert_eq!(begin.format(), expected);
        assert_eq!(begin.byte_length(), 3);
        assert_eq!(begin.identity().token(), "preview:1");
    }
    for format in ["webp", "JPEG", ""] {
        let mut value = base.clone();
        value["format"] = json!(format);
        assert!(request(&value).is_err(), "{format}");
    }
    Ok(())
}

#[test]
fn preview_chunk_syntax_is_bounded_before_native_byte_admission() -> Result<(), Box<dyn Error>> {
    let vectors = vectors()?;
    let base = vectors["validRequests"][1].clone();
    for data in ["AQI=", "AQ==", "AQID"] {
        let mut value = base.clone();
        value["data"] = json!(data);
        assert!(request(&value).is_ok(), "{data}");
    }
    for data in ["AQI", "A=I=", "AQI\n", "====", "AQI-", "", "AB==", "AAB="] {
        let mut value = base.clone();
        value["data"] = json!(data);
        assert!(request(&value).is_err(), "{data}");
    }
    let mut value = base;
    value["data"] = json!(format!("{}=", "A".repeat(10_923)));
    assert!(request(&value).is_ok());
    let decoded = request(&value)?;
    let CanvasSurfaceOperation::PreviewChunk(chunk) = decoded.operation() else {
        return Err("Expected chunk".into());
    };
    assert_eq!(chunk.data().len(), MAX_SURFACE_PREVIEW_CHUNK_BYTES);
    value["data"] = json!("A".repeat(10_924));
    assert!(request(&value).is_err());
    value["data"] = json!("A".repeat(10_928));
    assert!(request(&value).is_err());
    Ok(())
}

#[test]
fn shared_preview_semantic_admission_matches_the_web_client() -> Result<(), Box<dyn Error>> {
    let vectors: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../Sugar/integrations/contracts/canvas_surface_preview_admission_vectors.json"
    )))?;
    for value in vectors["validCommands"].as_array().ok_or("Missing valid commands")? {
        assert!(request(value).is_ok(), "{value}");
    }
    for value in vectors["invalidCommands"].as_array().ok_or("Missing invalid commands")? {
        assert!(request(value).is_err(), "{value}");
    }
    Ok(())
}
