// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the same schema vectors as SugarCubes and reject ambiguous JSON boundaries.

use super::*;
use serde_json::Value;
use sha2::{Digest, Sha256};

#[test]
fn generated_decoder_identifies_the_retained_schema_snapshot() -> Result<(), Box<dyn Error>> {
    let schema = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../third_party/sugarcubes-canvas/canvas.schema.json"
    ))
    .replace("\r\n", "\n");
    let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/canvas_surface/wire.rs"));
    let recorded = source
        .lines()
        .find_map(|line| line.strip_prefix("// Schema SHA-256 (LF): "))
        .ok_or("Generated schema fingerprint absent")?;
    assert_eq!(recorded, format!("{:x}", Sha256::digest(schema.as_bytes())));
    Ok(())
}

#[test]
fn shared_surface_addresses_match_web_validation() -> Result<(), Box<dyn Error>> {
    let vectors: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../third_party/sugarcubes-canvas/address_vectors.json"
    )))?;
    let base = vectors.get("base").ok_or("Missing address vector base")?;
    let cases = vectors.get("cases").and_then(Value::as_array).ok_or("Missing address cases")?;
    for case in cases {
        let name = case.get("name").and_then(Value::as_str).ok_or("Missing vector name")?;
        let mut value = base.clone();
        if let Some(key) = case.get("path").and_then(Value::as_str) {
            let object = value.as_object_mut().ok_or("Address is not an object")?;
            if case.get("remove").and_then(Value::as_bool) == Some(true) {
                object.remove(key).ok_or("Missing vector field")?;
            } else {
                object.insert(
                    key.to_owned(),
                    case.get("value").ok_or("Missing vector value")?.clone(),
                );
            }
        }
        let expected = case.get("valid").and_then(Value::as_bool).ok_or("Missing vector result")?;
        assert_eq!(
            CanvasSurfaceAddress::decode(&serde_json::to_vec(&value)?).is_ok(),
            expected,
            "{name}"
        );
    }
    Ok(())
}

#[test]
fn surface_address_rejects_ambiguous_json_and_preserves_each_coordinate()
-> Result<(), Box<dyn Error>> {
    let source = br#"{"sessionId":"canvas:workflow:one","viewId":"view:one","windowId":"window:main","attachmentId":"attachment:first","generation":1}"#;
    let address = CanvasSurfaceAddress::decode(source)?;
    assert_eq!(address.session_id(), "canvas:workflow:one");
    assert_eq!(address.view_id(), "view:one");
    assert_eq!(address.window_id(), "window:main");
    assert_eq!(address.attachment_id(), "attachment:first");
    assert_eq!(address.generation(), 1);
    let duplicate = String::from_utf8(source.to_vec())?
        .replace("\"generation\":1", "\"generation\":1,\"generation\":2");
    assert!(matches!(
        CanvasSurfaceAddress::decode(duplicate.as_bytes()),
        Err(CanvasSurfaceAddressError::Malformed(_))
    ));
    assert!(matches!(
        CanvasSurfaceAddress::decode(&vec![b' '; MAX_SURFACE_ADDRESS_BYTES + 1]),
        Err(CanvasSurfaceAddressError::TooLarge)
    ));
    assert!(CanvasSurfaceAddress::decode(br#"{"sessionId":"canvas:x","viewId":"view:\ud800","windowId":"window:x","attachmentId":"attachment:x","generation":1}"#).is_err());
    Ok(())
}

#[test]
fn shared_geometry_vectors_match_web_validation() -> Result<(), Box<dyn Error>> {
    let vectors: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../third_party/sugarcubes-canvas/geometry_vectors.json"
    )))?;
    let base = vectors.get("base").ok_or("Missing geometry vector base")?;
    let cases = vectors.get("cases").and_then(Value::as_array).ok_or("Missing geometry cases")?;
    for case in cases {
        let name = case.get("name").and_then(Value::as_str).ok_or("Missing vector name")?;
        let mut value = base.clone();
        if let Some(path) = case.get("path").and_then(Value::as_str) {
            let parts: Vec<_> = path.split('.').collect();
            let (key, ancestors) = parts.split_last().ok_or("Empty vector path")?;
            let mut current = &mut value;
            for part in ancestors {
                current = current.get_mut(*part).ok_or("Missing vector parent")?;
            }
            let object = current.as_object_mut().ok_or("Vector parent is not an object")?;
            if case.get("remove").and_then(Value::as_bool) == Some(true) {
                object.remove(*key).ok_or("Missing vector field")?;
            } else {
                object.insert(
                    (*key).to_owned(),
                    case.get("value").ok_or("Missing vector value")?.clone(),
                );
            }
        }
        let expected = case.get("valid").and_then(Value::as_bool).ok_or("Missing vector result")?;
        assert_eq!(
            CanvasSurfaceGeometry::decode(&serde_json::to_vec(&value)?).is_ok(),
            expected,
            "{name}"
        );
    }
    Ok(())
}

#[test]
fn geometry_is_bounded_closed_and_preserves_validated_units() -> Result<(), Box<dyn Error>> {
    let source = br#"{"coordinateSpace":"client","units":"css-pixels","origin":{"x":-2,"y":3},"viewport":{"width":40,"height":20},"clip":{"x":0,"y":3,"width":20,"height":10},"physicalPixelsPerCssPixel":1.5,"visible":true,"focused":false}"#;
    let geometry = CanvasSurfaceGeometry::decode(source)?;
    assert_eq!(geometry.origin(), (-2.0, 3.0));
    assert_eq!(geometry.viewport(), (40.0, 20.0));
    assert_eq!(geometry.clip(), (0.0, 3.0, 20.0, 10.0));
    assert_eq!(geometry.density(), 1.5);
    assert!(geometry.visible());
    assert!(!geometry.focused());
    assert!(matches!(
        CanvasSurfaceGeometry::decode(&vec![b' '; MAX_GEOMETRY_BYTES + 1]),
        Err(GeometryError::TooLarge)
    ));
    let duplicate = String::from_utf8(source.to_vec())?
        .replace("\"visible\":true", "\"visible\":true,\"visible\":false");
    assert!(matches!(
        CanvasSurfaceGeometry::decode(duplicate.as_bytes()),
        Err(GeometryError::Malformed(_))
    ));
    Ok(())
}

/// Decode one geometry snapshot whose base fields are valid, with extra fields merged in.
fn geometry_with(extra: Value) -> Result<CanvasSurfaceGeometry, GeometryError> {
    let mut value = serde_json::json!({
        "coordinateSpace":"client", "units":"css-pixels", "origin":{"x":10,"y":20},
        "viewport":{"width":400,"height":300}, "clip":{"x":10,"y":20,"width":400,"height":300},
        "physicalPixelsPerCssPixel":1.5, "visible":true, "focused":true
    });
    if let (Some(object), Some(extra)) = (value.as_object_mut(), extra.as_object()) {
        object.extend(extra.clone());
    }
    let bytes = serde_json::to_vec(&value).map_err(GeometryError::Malformed)?;
    CanvasSurfaceGeometry::decode(&bytes)
}

#[test]
fn occlusion_accessors_preserve_order_units_and_default_to_unoccluded() -> Result<(), Box<dyn Error>>
{
    let plain = geometry_with(serde_json::json!({}))?;
    assert_eq!(plain.occlusions().len(), 0);
    assert!(!plain.modal());
    let covered = geometry_with(serde_json::json!({
        "occlusions":[{"x":-5,"y":4.5,"width":20,"height":0.25},{"x":100,"y":0,"width":3,"height":7}],
        "modal":true
    }))?;
    assert_eq!(
        covered.occlusions().collect::<Vec<_>>(),
        [(-5.0, 4.5, 20.0, 0.25), (100.0, 0.0, 3.0, 7.0)]
    );
    assert!(covered.modal());
    Ok(())
}
