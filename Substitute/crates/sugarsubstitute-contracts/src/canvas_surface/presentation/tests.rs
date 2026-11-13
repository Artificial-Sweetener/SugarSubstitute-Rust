// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify watch correlation and typed asynchronous presentation encoding.

use super::*;
use crate::canvas_surface::{CanvasSurfaceMessage, CanvasViewportMode, preview::tests::vectors};
use serde_json::{Value, json};
use std::error::Error;

#[test]
fn presentation_watch_preserves_request_and_observed_enabled_state() -> Result<(), Box<dyn Error>> {
    let vectors = vectors()?;
    for value in vectors["validRequests"]
        .as_array()
        .ok_or("Missing requests")?
        .iter()
        .filter(|value| value["type"].as_str() == Some("surface.presentation.request"))
    {
        let CanvasSurfaceMessage::Presentation(request) =
            CanvasSurfaceMessage::decode(&serde_json::to_vec(value)?)?
        else {
            return Err("Expected watch request".into());
        };
        assert_eq!(request.request_id(), 7);
        assert_eq!(request.session().as_str(), "preview-contract-session");
        assert_eq!(request.enabled(), value["enabled"].as_bool().ok_or("Missing enabled")?);
        for status in [
            CanvasSurfaceDiscoveryStatus::Available,
            CanvasSurfaceDiscoveryStatus::Unsupported,
            CanvasSurfaceDiscoveryStatus::Unavailable,
            CanvasSurfaceDiscoveryStatus::Error,
        ] {
            let bytes = request.reply(status, request.enabled(), "")?;
            let reply: wire::CanvasSurfacePresentationReply = serde_json::from_slice(&bytes)?;
            assert!(reply.valid());
            assert_eq!(reply.request_id, request.request_id());
            assert_eq!(reply.enabled, request.enabled());
            assert_eq!(reply.status, status.as_str());
        }
        assert!(
            request.reply(CanvasSurfaceDiscoveryStatus::Available, true, &"é".repeat(513)).is_err()
        );
    }
    for value in vectors["validReplies"].as_array().ok_or("Missing replies")? {
        let decoded: wire::CanvasSurfacePresentationReply = serde_json::from_value(value.clone())?;
        assert!(decoded.valid());
        assert_eq!(serde_json::to_value(decoded)?, *value);
    }
    Ok(())
}

fn viewport(value: &Value) -> Result<CanvasViewportState, Box<dyn Error>> {
    let mode = match value["mode"].as_str().ok_or("Missing mode")? {
        "fit" => CanvasViewportMode::Fit,
        "native" => CanvasViewportMode::Native,
        "custom" => CanvasViewportMode::Custom,
        _ => return Err("Unknown fixture viewport".into()),
    };
    Ok(CanvasViewportState::new(
        mode,
        value["physicalScale"].as_f64().ok_or("Missing scale")?,
        u32::try_from(value["dimensions"]["width"].as_u64().ok_or("Missing width")?)?,
        u32::try_from(value["dimensions"]["height"].as_u64().ok_or("Missing height")?)?,
    )?)
}

fn typed_change(change: &Value) -> Result<CanvasSurfacePresentationChange, Box<dyn Error>> {
    let displayed = &change["displayed"];
    let displayed = match displayed["type"].as_str().ok_or("Missing displayed type")? {
        "empty" => CanvasSurfacePresentationDisplay::Empty,
        "preview" => CanvasSurfacePresentationDisplay::Preview {
            token: displayed["token"].as_str().ok_or("Missing token")?.into(),
            lease_revision: displayed["leaseRevision"].as_u64().ok_or("Missing lease")?,
            viewport: viewport(&displayed["viewport"])?,
        },
        "final" => CanvasSurfacePresentationDisplay::Final {
            selection_revision: displayed["selectionRevision"]
                .as_u64()
                .ok_or("Missing selection revision")?,
            viewport: viewport(&displayed["viewport"])?,
        },
        _ => return Err("Unknown displayed fixture".into()),
    };
    let status = match change["status"].as_str().ok_or("Missing status")? {
        "presented" => CanvasSurfacePresentationStatus::Presented,
        "failed" => CanvasSurfacePresentationStatus::Failed,
        "cancelled" => CanvasSurfacePresentationStatus::Cancelled,
        _ => return Err("Unknown fixture status".into()),
    };
    Ok(CanvasSurfacePresentationChange::new(
        CanvasSurfaceAddress::decode(&serde_json::to_vec(&change["address"])?)?,
        change["sequence"].as_u64().ok_or("Missing sequence")?,
        change["token"].as_str().ok_or("Missing token")?.into(),
        change["leaseRevision"].as_u64().ok_or("Missing lease")?,
        status,
        displayed,
        change["message"].as_str().ok_or("Missing message")?.into(),
    )?)
}

#[test]
fn typed_presentation_events_roundtrip_every_shared_display_variant() -> Result<(), Box<dyn Error>>
{
    let vectors = vectors()?;
    for value in vectors["validChanges"].as_array().ok_or("Missing changes")? {
        let change = typed_change(&value["change"])?;
        let session =
            PanelSession::new(value["session"].as_str().ok_or("Missing session")?.into())?;
        let bytes = change.encode(&session)?;
        assert_eq!(serde_json::from_slice::<Value>(&bytes)?, *value);
        let decoded: wire::CanvasSurfacePresentationChanged = serde_json::from_slice(&bytes)?;
        assert!(decoded.valid());
        assert_eq!(decoded.request_id, 0);
        assert_eq!(decoded.schema, 1);
        assert_eq!(serde_json::to_vec(&decoded)?, bytes);
    }
    Ok(())
}

#[test]
fn presented_outcomes_require_the_exact_fit_preview_and_retained_previews_stay_fit()
-> Result<(), Box<dyn Error>> {
    let vectors = vectors()?;
    let base = &vectors["validChanges"][0]["change"];
    for displayed in [
        json!({"type": "empty"}),
        json!({"type": "final", "selectionRevision": 2, "viewport": base["displayed"]["viewport"]}),
    ] {
        let mut value = base.clone();
        value["displayed"] = displayed;
        assert!(typed_change(&value).is_err());
    }
    for (field, invalid) in [("token", json!("preview:2")), ("leaseRevision", json!(4))] {
        let mut value = base.clone();
        value["displayed"][field] = invalid;
        assert!(typed_change(&value).is_err());
    }
    for status in ["presented", "failed", "cancelled"] {
        for mode in ["native", "custom"] {
            let mut value = base.clone();
            value["status"] = json!(status);
            value["displayed"]["viewport"]["mode"] = json!(mode);
            assert!(typed_change(&value).is_err(), "{status}: {mode}");
        }
    }
    for status in ["failed", "cancelled"] {
        let mut value = base.clone();
        value["status"] = json!(status);
        value["displayed"]["token"] = json!("preview:2");
        value["displayed"]["leaseRevision"] = json!(4);
        assert!(typed_change(&value).is_ok(), "{status} retained preview");
    }
    Ok(())
}

#[test]
fn shared_presentation_admission_vectors_match_the_web_client() -> Result<(), Box<dyn Error>> {
    let vectors: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../Sugar/integrations/contracts/canvas_surface_preview_admission_vectors.json"
    )))?;
    for value in vectors["validChanges"].as_array().ok_or("Missing valid changes")? {
        assert!(typed_change(value).is_ok(), "{value}");
    }
    for value in vectors["invalidChanges"].as_array().ok_or("Missing invalid changes")? {
        assert!(typed_change(value).is_err(), "{value}");
    }
    Ok(())
}

#[test]
fn presentation_changed_retains_closed_shapes_and_numeric_bounds() -> Result<(), Box<dyn Error>> {
    let vectors = vectors()?;
    let base = &vectors["validChanges"][0];
    for pointer in
        ["/extra", "/change/extra", "/change/displayed/extra", "/change/displayed/viewport/extra"]
    {
        let mut value = base.clone();
        let (parent, key) = pointer.rsplit_once('/').ok_or("Missing pointer")?;
        value
            .pointer_mut(parent)
            .ok_or("Missing parent")?
            .as_object_mut()
            .ok_or("Expected object")?
            .insert(key.into(), json!(true));
        assert!(serde_json::from_value::<wire::CanvasSurfacePresentationChanged>(value).is_err());
    }
    for (pointer, invalid) in [
        ("/requestId", json!(1)),
        ("/schema", json!(2)),
        ("/change/sequence", json!(0)),
        ("/change/leaseRevision", json!(0)),
        ("/change/token", json!("x".repeat(25))),
        ("/change/message", json!("é".repeat(513))),
        ("/change/displayed/leaseRevision", json!(0)),
        ("/change/displayed/viewport/physicalScale", json!(0)),
    ] {
        let mut value = base.clone();
        *value.pointer_mut(pointer).ok_or("Missing field")? = invalid;
        let decoded: wire::CanvasSurfacePresentationChanged = serde_json::from_value(value)?;
        assert!(!decoded.valid(), "{pointer}");
    }
    Ok(())
}

#[test]
fn a_presentation_watch_is_accepted_only_at_its_fixed_schema_one() -> Result<(), Box<dyn Error>> {
    let request = |schema: Value| -> Result<Vec<u8>, Box<dyn Error>> {
        Ok(serde_json::to_vec(&json!({
            "protocol": "sugarcubes.canvas", "version": 3, "session": "preview-contract-session",
            "requestId": 7, "type": "surface.presentation.request", "schema": schema,
            "enabled": true,
        }))?)
    };
    let CanvasSurfaceMessage::Presentation(watch) =
        CanvasSurfaceMessage::decode(&request(json!(1))?)?
    else {
        return Err("Expected watch request".into());
    };
    let reply: Value = serde_json::from_slice(&watch.reply(
        CanvasSurfaceDiscoveryStatus::Unavailable,
        false,
        "",
    )?)?;
    assert_eq!(reply["schema"], 1);
    assert_eq!(reply["status"], "unavailable");
    // Unlike surface discovery, the watch does not negotiate: a newer schema is refused.
    for schema in [json!(0), json!(2), json!(3), json!(1003)] {
        assert!(CanvasSurfaceMessage::decode(&request(schema.clone())?).is_err(), "{schema}");
    }
    Ok(())
}
