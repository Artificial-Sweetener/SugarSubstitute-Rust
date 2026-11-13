// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify correlated discovery and bounded typed descriptors against the shared web vectors.

use super::*;
use crate::canvas_surface::CanvasSurfaceMessage;
use serde_json::{Value, json};
use std::error::Error;

/// Read the unchanged language-neutral request, reply and rejection specimens.
fn vectors() -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../Sugar/integrations/contracts/canvas_surface_discovery_vectors.json"
    )))?)
}

/// Admit the shared request through the public message boundary.
fn request() -> Result<CanvasSurfaceDiscoveryRequest, Box<dyn Error>> {
    let value = vectors()?.get("request").ok_or("Missing discovery request")?.clone();
    match CanvasSurfaceMessage::decode(&serde_json::to_vec(&value)?)? {
        CanvasSurfaceMessage::Discovery(request) => Ok(request),
        _ => Err("Expected surface discovery".into()),
    }
}

#[test]
fn legacy_discovery_receives_only_an_empty_parseable_unsupported_descriptor()
-> Result<(), Box<dyn Error>> {
    let mut value = vectors()?["request"].clone();
    value["schema"] = json!(1);
    let CanvasSurfaceMessage::Discovery(request) =
        CanvasSurfaceMessage::decode(&serde_json::to_vec(&value)?)?
    else {
        return Err("Expected legacy discovery".into());
    };
    let capabilities = CanvasSurfaceCapabilities {
        image_formats: vec![CanvasSurfaceImageFormat::Png],
        viewport_commands: vec![CanvasSurfaceViewportCommand::Inspect],
        viewport_state: true,
        ..Default::default()
    };
    let value: Value = serde_json::from_slice(&request.reply(
        CanvasSurfaceDiscoveryStatus::Available,
        &capabilities,
        "",
    )?)?;
    assert_eq!(value["status"], "unsupported");
    assert_eq!(value["capabilities"]["schema"], 1);
    assert_eq!(value["capabilities"]["imageFormats"], json!([]));
    assert_eq!(value["capabilities"]["viewportCommands"], json!([]));
    assert_eq!(value["capabilities"]["viewportState"], false);
    assert!(
        value["message"]
            .as_str()
            .is_some_and(|text| text.contains("Update SugarCubes and its native host together"))
    );
    Ok(())
}

#[test]
fn discovery_replies_match_the_shared_web_contract() -> Result<(), Box<dyn Error>> {
    let vectors = vectors()?;
    let request = request()?;
    assert_eq!(request.session().as_str(), "surface-discovery-session");
    assert_eq!(request.request_id(), 7);
    let capabilities = CanvasSurfaceCapabilities {
        image_formats: vec![CanvasSurfaceImageFormat::Png],
        ..Default::default()
    };
    for (status, capabilities, key) in [
        (CanvasSurfaceDiscoveryStatus::Available, capabilities, "available"),
        (
            CanvasSurfaceDiscoveryStatus::Unsupported,
            CanvasSurfaceCapabilities::default(),
            "unsupported",
        ),
    ] {
        let bytes = request.reply(status, &capabilities, "")?;
        let value: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(&value, vectors.get(key).ok_or("Missing shared discovery reply")?);
        let wire: wire::CanvasSurfaceCapabilitiesReply = serde_json::from_slice(&bytes)?;
        assert!(wire.valid());
        assert!(CanvasSurfaceMessage::decode(&bytes).is_err());
    }
    for value in vectors
        .get("invalidRequests")
        .and_then(Value::as_array)
        .ok_or("Missing invalid discovery requests")?
    {
        assert!(CanvasSurfaceMessage::decode(&serde_json::to_vec(value)?).is_err(), "{value}");
    }
    for value in vectors
        .get("invalidReplies")
        .and_then(Value::as_array)
        .ok_or("Missing invalid discovery replies")?
    {
        let decoded = serde_json::from_value::<wire::CanvasSurfaceCapabilitiesReply>(value.clone());
        assert!(!decoded.is_ok_and(|reply| reply.valid()), "{value}");
    }
    Ok(())
}

#[test]
fn discovery_preserves_each_typed_capability_without_enabling_others() -> Result<(), Box<dyn Error>>
{
    let capabilities = CanvasSurfaceCapabilities {
        image_formats: vec![
            CanvasSurfaceImageFormat::Png,
            CanvasSurfaceImageFormat::Jpeg,
            CanvasSurfaceImageFormat::WebP,
        ],
        viewport_commands: vec![
            CanvasSurfaceViewportCommand::Fit,
            CanvasSurfaceViewportCommand::Native,
            CanvasSurfaceViewportCommand::Zoom,
            CanvasSurfaceViewportCommand::Pan,
            CanvasSurfaceViewportCommand::Wipe,
            CanvasSurfaceViewportCommand::Inspect,
        ],
        viewport_state: true,
        preview_formats: vec![CanvasSurfacePreviewFormat::Png, CanvasSurfacePreviewFormat::Jpeg],
        comparison: true,
        overview: true,
        input: true,
        backdrop: false,
    };
    let bytes = request()?.reply(
        CanvasSurfaceDiscoveryStatus::Available,
        &capabilities,
        &"🌈".repeat(512),
    )?;
    let value: Value = serde_json::from_slice(&bytes)?;
    assert_eq!(
        value["capabilities"],
        json!({
            "schema": 2,
            "imageFormats": ["png", "jpeg", "webp"],
            "viewportCommands": ["fit", "native", "zoom", "pan", "wipe", "inspect"],
            "viewportState": true,
            "previewFormats": ["png", "jpeg"],
            "comparison": true,
            "overview": true,
            "input": true,
        })
    );
    assert_eq!(value["message"].as_str().ok_or("Missing reply text")?.chars().count(), 512);
    assert!(
        request()?
            .reply(CanvasSurfaceDiscoveryStatus::Available, &capabilities, &"🌈".repeat(513),)
            .is_err()
    );
    assert!(
        request()?
            .reply(
                CanvasSurfaceDiscoveryStatus::Available,
                &CanvasSurfaceCapabilities::default(),
                "",
            )
            .is_ok()
    );
    Ok(())
}

#[test]
fn discovery_rejects_nonempty_failure_descriptors_and_oversized_lists() -> Result<(), Box<dyn Error>>
{
    let request = request()?;
    let populated = [
        CanvasSurfaceCapabilities {
            image_formats: vec![CanvasSurfaceImageFormat::Png],
            ..Default::default()
        },
        CanvasSurfaceCapabilities {
            viewport_commands: vec![CanvasSurfaceViewportCommand::Fit],
            ..Default::default()
        },
        CanvasSurfaceCapabilities { viewport_state: true, ..Default::default() },
        CanvasSurfaceCapabilities {
            preview_formats: vec![CanvasSurfacePreviewFormat::Png],
            ..Default::default()
        },
        CanvasSurfaceCapabilities { comparison: true, ..Default::default() },
        CanvasSurfaceCapabilities { overview: true, ..Default::default() },
        CanvasSurfaceCapabilities { input: true, ..Default::default() },
        CanvasSurfaceCapabilities { backdrop: true, ..Default::default() },
    ];
    for status in [
        CanvasSurfaceDiscoveryStatus::Unsupported,
        CanvasSurfaceDiscoveryStatus::Unavailable,
        CanvasSurfaceDiscoveryStatus::Error,
    ] {
        let bytes = request.reply(status, &CanvasSurfaceCapabilities::default(), "")?;
        let reply: wire::CanvasSurfaceCapabilitiesReply = serde_json::from_slice(&bytes)?;
        assert!(reply.valid());
        assert_eq!(reply.status, status.as_str());
        for capabilities in &populated {
            assert!(matches!(
                request.reply(status, capabilities, ""),
                Err(CanvasSurfaceMessageError::Invalid)
            ));
        }
    }
    for capabilities in [
        CanvasSurfaceCapabilities {
            image_formats: vec![CanvasSurfaceImageFormat::Png; 4],
            ..Default::default()
        },
        CanvasSurfaceCapabilities {
            viewport_commands: vec![CanvasSurfaceViewportCommand::Fit; 7],
            ..Default::default()
        },
        CanvasSurfaceCapabilities {
            preview_formats: vec![CanvasSurfacePreviewFormat::Png; 3],
            ..Default::default()
        },
    ] {
        assert!(matches!(
            request.reply(CanvasSurfaceDiscoveryStatus::Available, &capabilities, ""),
            Err(CanvasSurfaceMessageError::Invalid)
        ));
    }
    Ok(())
}

#[test]
fn discovery_requires_exact_versions_and_unambiguous_request_fields() -> Result<(), Box<dyn Error>>
{
    let base = vectors()?.get("request").ok_or("Missing discovery request")?.clone();
    let encoded = serde_json::to_string(&base)?;
    let fields = encoded.strip_prefix('{').ok_or("Expected request object")?;
    for duplicate in [
        "\"schema\":1",
        "\"version\":3",
        "\"requestId\":7",
        "\"session\":\"surface-discovery-session\"",
        "\"type\":\"surface.capabilities.request\"",
    ] {
        let duplicate = format!("{{{duplicate},{fields}");
        assert!(matches!(
            CanvasSurfaceMessage::decode(duplicate.as_bytes()),
            Err(CanvasSurfaceMessageError::Malformed(_))
        ));
    }
    for (field, value) in [
        ("version", json!(2)),
        ("version", json!(4)),
        ("version", json!(3.5)),
        ("schema", json!(0)),
        ("schema", json!(2_147_483_648_u64)),
        ("protocol", json!("different.canvas")),
        ("requestId", json!(-1)),
        ("address", json!({})),
    ] {
        let mut value_to_decode = base.clone();
        value_to_decode
            .as_object_mut()
            .ok_or("Expected request object")?
            .insert(field.into(), value);
        assert!(CanvasSurfaceMessage::decode(&serde_json::to_vec(&value_to_decode)?).is_err());
    }
    for field in ["protocol", "version", "session", "requestId", "type", "schema"] {
        let mut value = base.clone();
        value
            .as_object_mut()
            .ok_or("Expected request object")?
            .remove(field)
            .ok_or("Missing request field")?;
        assert!(CanvasSurfaceMessage::decode(&serde_json::to_vec(&value)?).is_err());
    }
    assert!(matches!(
        CanvasSurfaceMessage::decode(&vec![b' '; MAX_SURFACE_MESSAGE_BYTES + 1]),
        Err(CanvasSurfaceMessageError::TooLarge)
    ));
    Ok(())
}

#[test]
fn discovery_message_session_is_the_request_session() -> Result<(), Box<dyn Error>> {
    let value = vectors()?.get("request").ok_or("Missing discovery request")?.clone();
    let message = CanvasSurfaceMessage::decode(&serde_json::to_vec(&value)?)?;
    assert_eq!(message.session(), request()?.session());
    Ok(())
}

#[test]
fn backdrop_support_is_negotiated_only_with_schema_three() -> Result<(), Box<dyn Error>> {
    let capabilities = CanvasSurfaceCapabilities { backdrop: true, ..Default::default() };
    for schema in [2, 3] {
        let mut value = vectors()?["request"].clone();
        value["schema"] = json!(schema);
        let CanvasSurfaceMessage::Discovery(request) =
            CanvasSurfaceMessage::decode(&serde_json::to_vec(&value)?)?
        else {
            return Err("Expected discovery request".into());
        };
        let bytes = request.reply(CanvasSurfaceDiscoveryStatus::Available, &capabilities, "")?;
        let value: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(value["capabilities"]["schema"], schema);
        if schema == 3 {
            assert_eq!(value["capabilities"]["backdrop"], json!({"schema": 1}));
        } else {
            assert!(value["capabilities"].get("backdrop").is_none());
        }
        let wire: wire::CanvasSurfaceCapabilitiesReply = serde_json::from_slice(&bytes)?;
        assert!(wire.valid());
    }
    Ok(())
}

/// A page that asks for a newer discovery schema is answered in the newest schema this host knows.
#[test]
fn newer_discovery_schemas_are_answered_in_the_newest_known_schema() -> Result<(), Box<dyn Error>> {
    let capabilities = CanvasSurfaceCapabilities { backdrop: true, ..Default::default() };
    for schema in [4_u64, 2_147_483_647] {
        let mut value = vectors()?["request"].clone();
        value["schema"] = json!(schema);
        let CanvasSurfaceMessage::Discovery(request) =
            CanvasSurfaceMessage::decode(&serde_json::to_vec(&value)?)?
        else {
            return Err("Expected discovery request".into());
        };
        assert_eq!(request.schema(), schema);
        let bytes = request.reply(CanvasSurfaceDiscoveryStatus::Available, &capabilities, "")?;
        let value: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(value["capabilities"]["schema"], 3);
        assert_eq!(value["capabilities"]["backdrop"], json!({"schema": 1}));
        let wire: wire::CanvasSurfaceCapabilitiesReply = serde_json::from_slice(&bytes)?;
        assert!(wire.valid());
    }
    Ok(())
}
