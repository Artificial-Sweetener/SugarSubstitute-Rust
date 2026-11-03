// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise source commands, captured rasters and `source.changed` against the schema.

use super::*;

fn capture(targets: Value, maximum_size: u64) -> Value {
    request(
        "source.request",
        json!({"type": "capture", "targets": targets, "maximumSize": maximum_size,
            "document": {"id": "input:surface-1", "geometryRevision": "geometry-1",
                "width": 1024, "height": 768}}),
    )
}

#[test]
fn every_command_and_target_decodes_to_its_typed_intent() -> TestResult {
    let decode = |value: &Value| -> Result<CanvasSourceCommand, Box<dyn Error>> {
        Ok(CanvasSourceRequest::decode(&bytes(value)?)?.command().clone())
    };
    assert_eq!(
        decode(&request("source.request", json!({"type": "context"})))?,
        CanvasSourceCommand::Context
    );
    assert_eq!(
        decode(&request("source.request", json!({"type": "watch", "enabled": false})))?,
        CanvasSourceCommand::Watch { enabled: false }
    );
    let targets = json!([{"kind": "selection"}, {"kind": "current-layer"},
        {"kind": "layer", "layerId": "layer:mask-1"}, {"kind": "composition"},
        {"kind": "composition-from-layer", "layerId": "layer:2"}]);
    let layer = |id: &str| CanvasLayerId::new(id);
    assert_eq!(
        decode(&capture(targets, 0))?,
        CanvasSourceCommand::Capture(CanvasSourceCapture {
            document: document()?,
            targets: vec![
                CanvasSourceTarget::Selection,
                CanvasSourceTarget::CurrentLayer,
                CanvasSourceTarget::Layer(layer("layer:mask-1")?),
                CanvasSourceTarget::Composition,
                CanvasSourceTarget::CompositionFromLayer(layer("layer:2")?),
            ],
            size: CanvasCaptureSize::Full,
        })
    );
    let CanvasSourceCommand::Capture(preview) = decode(&capture(json!([]), 1024))? else {
        return Err("Capture did not decode".into());
    };
    assert_eq!(preview.size, CanvasCaptureSize::Preview(1024));
    Ok(())
}

#[test]
fn malformed_and_out_of_range_source_requests_are_refused() -> TestResult {
    let many: Vec<Value> = (0..65).map(|_| json!({"kind": "selection"})).collect();
    for value in [
        capture(json!([]), 1025),
        capture(json!(many), 0),
        capture(json!([{"kind": "layer"}]), 0),
        capture(json!([{"kind": "selection", "layerId": "x"}]), 0),
        capture(json!([{"kind": "mask"}]), 0),
        capture(json!([{"kind": "layer", "layerId": "x".repeat(129)}]), 0),
        request("source.request", json!({"type": "watch"})),
        request("source.request", json!({"type": "context", "extra": 1})),
        request("source.request", json!({"type": "masks"})),
    ] {
        assert!(CanvasSourceRequest::decode(&bytes(&value)?).is_err(), "{value}");
    }
    // Maximal layer ids exceed the inline lane but not the family bound.
    let wide: Vec<Value> =
        (0..64).map(|_| json!({"kind": "layer", "layerId": "图".repeat(128)})).collect();
    let encoded = bytes(&capture(json!(wide), 0))?;
    assert!(encoded.len() > 16 * 1024 && encoded.len() <= MAX_CANVAS_SOURCE_REQUEST_BYTES);
    assert!(CanvasSourceRequest::decode(&encoded).is_ok());
    Ok(())
}

#[test]
fn context_watching_and_failure_replies_are_schema_valid_and_exact() -> TestResult {
    let request = CanvasSourceRequest::decode(&bytes(&request(
        "source.request",
        json!({"type": "context"}),
    ))?)?;
    let context = CanvasSourceContext {
        supported: true,
        document: Some(document()?),
        revision: CanvasRevision::new("content-3")?,
        targets: vec![
            CanvasSourceTargetKind::Selection,
            CanvasSourceTargetKind::CurrentLayer,
            CanvasSourceTargetKind::Layer,
            CanvasSourceTargetKind::Composition,
        ],
        layers: vec![
            CanvasSourceLayer {
                id: CanvasLayerId::new("layer:mask-1")?,
                name: CanvasLayerName::new("Mask node:13:mask")?,
                parent: None,
                depth: 0,
                visible: true,
            },
            CanvasSourceLayer {
                id: CanvasLayerId::new("layer:image")?,
                name: CanvasLayerName::new("Load Image")?,
                parent: Some(CanvasLayerId::new("layer:group")?),
                depth: 1,
                visible: false,
            },
        ],
        current_layer: Some(CanvasLayerId::new("layer:mask-1")?),
        observes_changes: true,
    };
    let encoded: Value = serde_json::from_slice(request.context_reply(&context)?.as_bytes())?;
    assert!(accepted(&encoded, wire::CanvasSourceReply::valid));
    assert_eq!(encoded["type"], "source.reply");
    assert_eq!(
        encoded["result"],
        json!({"kind": "context", "supported": true,
            "document": {"id": "input:surface-1", "geometryRevision": "geometry-1",
                "width": 1024, "height": 768},
            "revision": "content-3",
            "targets": ["selection", "current-layer", "layer", "composition"],
            "layers": [
                {"id": "layer:mask-1", "name": "Mask node:13:mask", "parentId": null, "depth": 0,
                    "visible": true},
                {"id": "layer:image", "name": "Load Image", "parentId": "layer:group", "depth": 1,
                    "visible": false}],
            "currentLayerId": "layer:mask-1", "observesChanges": true})
    );
    let mut deep = context.clone();
    deep.layers[0].depth = 129;
    assert!(request.context_reply(&deep).is_err());
    let mut crowded = context;
    crowded.targets = vec![CanvasSourceTargetKind::Selection; 6];
    assert!(request.context_reply(&crowded).is_err());
    let watching: Value = serde_json::from_slice(request.watching_reply(true)?.as_bytes())?;
    assert_eq!(watching["result"], json!({"kind": "watching", "supported": true}));
    let failure = CanvasFailure::new(CanvasFailureStatus::Stale);
    let failed: Value = serde_json::from_slice(request.failure_reply(&failure)?.as_bytes())?;
    assert_eq!(failed["result"], json!({"kind": "failure", "status": "stale", "message": ""}));
    Ok(())
}

#[test]
fn captured_rasters_use_canonical_padded_base64_and_exact_extents() -> TestResult {
    assert_eq!(raster::encode_base64(b""), "");
    assert_eq!(raster::encode_base64(b"f"), "Zg==");
    assert_eq!(raster::encode_base64(b"fo"), "Zm8=");
    assert_eq!(raster::encode_base64(b"foo"), "Zm9v");
    assert_eq!(raster::encode_base64(&[0, 1, 2, 255, 254, 253, 252]), "AAEC//79/A==");
    let coverage = CanvasPixelRaster::new(2, 1, CanvasPixelFormat::Coverage8, vec![0, 255])?;
    let mask = CanvasPixelRaster::new(1, 1, CanvasPixelFormat::Rgba8, vec![64, 64, 64, 191])?;
    assert!(CanvasPixelRaster::new(2, 1, CanvasPixelFormat::Rgba8, vec![0; 4]).is_err());
    assert!(CanvasPixelRaster::new(0, 1, CanvasPixelFormat::Coverage8, Vec::new()).is_err());
    assert!(
        CanvasPixelRaster::new(65_537, 1, CanvasPixelFormat::Coverage8, vec![0; 65_537]).is_err()
    );
    let request =
        CanvasSourceRequest::decode(&bytes(&capture(json!([{"kind": "selection"}]), 0))?)?;
    let snapshot = |target, pixels| -> Result<CanvasSourceSnapshot, Box<dyn Error>> {
        Ok(CanvasSourceSnapshot {
            target,
            document: document()?,
            revision: CanvasRevision::new("content-3")?,
            pixels,
        })
    };
    let payload = request.captured_reply(&[
        snapshot(CanvasSourceTarget::Selection, coverage)?,
        snapshot(CanvasSourceTarget::Layer(CanvasLayerId::new("layer:mask-1")?), mask)?,
    ])?;
    let encoded: Value = serde_json::from_slice(payload.as_bytes())?;
    assert!(accepted(&encoded, wire::CanvasSourceReply::valid));
    let snapshots = &encoded["result"]["snapshots"];
    assert_eq!(snapshots[0]["target"], json!({"kind": "selection"}));
    assert_eq!(
        snapshots[0]["pixels"],
        json!({"width": 2, "height": 1, "format": "coverage8", "data": "AP8="})
    );
    assert_eq!(snapshots[1]["target"], json!({"kind": "layer", "layerId": "layer:mask-1"}));
    assert_eq!(
        snapshots[1]["pixels"],
        json!({"width": 1, "height": 1, "format": "rgba8", "data": "QEBAvw=="})
    );
    Ok(())
}

#[test]
fn a_maximal_raster_fits_the_source_reply_bound() -> TestResult {
    let pixels = vec![7; CanvasPixelRaster::MAX_BYTES];
    let raster = CanvasPixelRaster::new(4096, 4096, CanvasPixelFormat::Rgba8, pixels)?;
    assert!(
        CanvasPixelRaster::new(4096, 4097, CanvasPixelFormat::Rgba8, vec![0; 4096 * 4097 * 4])
            .is_err()
    );
    let request =
        CanvasSourceRequest::decode(&bytes(&capture(json!([{"kind": "composition"}]), 0))?)?;
    let snapshot = CanvasSourceSnapshot {
        target: CanvasSourceTarget::Composition,
        document: document()?,
        revision: CanvasRevision::new("r".repeat(512))?,
        pixels: raster,
    };
    let payload = request.captured_reply(std::slice::from_ref(&snapshot))?;
    assert!(payload.as_bytes().len() <= MAX_CANVAS_SOURCE_REPLY_BYTES);
    Ok(())
}

#[test]
fn source_changed_is_a_zero_id_notification() -> TestResult {
    let change = CanvasSourceChange {
        document: Some(CanvasDocumentId::new("input:surface-1")?),
        revision: CanvasRevision::new("content-4")?,
    };
    let payload = change.encode(&session()?)?;
    assert_eq!(payload.kind(), CanvasNativeNotificationKind::SourceChanged);
    let encoded: Value = serde_json::from_slice(payload.as_bytes())?;
    assert_eq!(
        encoded,
        json!({"protocol": "sugarcubes.canvas", "version": 3, "session": SESSION, "requestId": 0,
            "type": "source.changed",
            "change": {"documentId": "input:surface-1", "revision": "content-4"}})
    );
    assert!(accepted(&encoded, wire::CanvasSourceChanged::valid));
    let cleared = CanvasSourceChange { document: None, revision: CanvasRevision::new("")? };
    let encoded: Value = serde_json::from_slice(cleared.encode(&session()?)?.as_bytes())?;
    assert_eq!(encoded["change"]["documentId"], Value::Null);
    Ok(())
}
