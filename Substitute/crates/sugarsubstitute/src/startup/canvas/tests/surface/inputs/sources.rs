// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Capture shown and parked input documents in every target format and size, and watch them.

use super::*;

/// Decode one snapshot's canonical base64 pixels.
fn pixels(snapshot: &Value) -> Result<Vec<u8>, Box<dyn Error>> {
    let text = snapshot["pixels"]["data"].as_str().ok_or("Snapshot has no pixel data")?;
    let value = |byte: u8| -> Result<u32, Box<dyn Error>> {
        Ok(match byte {
            b'A'..=b'Z' => u32::from(byte - b'A'),
            b'a'..=b'z' => u32::from(byte - b'a') + 26,
            b'0'..=b'9' => u32::from(byte - b'0') + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return Err("Snapshot data is not base64".into()),
        })
    };
    let mut bytes = Vec::new();
    for chunk in text.as_bytes().chunks(4) {
        let padding = chunk.iter().filter(|byte| **byte == b'=').count();
        let mut group = 0;
        for byte in chunk {
            group = group << 6 | if *byte == b'=' { 0 } else { value(*byte)? };
        }
        bytes.extend(&group.to_be_bytes()[1..4 - padding]);
    }
    Ok(bytes)
}

fn capture(document: &Value, targets: Value, maximum: u32) -> Value {
    json!({"type":"capture", "document":document, "targets":targets, "maximumSize":maximum})
}

#[test]
fn captures_return_every_target_format_at_exact_sizes_from_one_revision()
-> Result<(), Box<dyn Error>> {
    let directory = anchor()?;
    let mut inputs = Inputs::start(directory.path())?;
    let result = inputs.workflow(sync(Some("mask.png")))?;
    let image = synced(&result, "image")?.clone();
    let latent = synced(&result, "latent")?.clone();
    inputs.workflow(activate(Some("image"), Some("painted"), "input"))?;
    inputs.context_until(|document| document["id"] == image["document"]["id"])?;
    let context = inputs.ask("source.request", Some(json!({"type":"context"})))?["result"].take();
    assert_eq!(context["document"], image["document"]);
    assert_eq!(context["currentLayerId"], "layer-2");
    assert_eq!(context["observesChanges"], true);
    assert_eq!(
        context["targets"],
        json!(["selection", "current-layer", "layer", "composition", "composition-from-layer"])
    );
    assert_eq!(
        context["layers"],
        json!([
            {"id":"layer-3", "name":"Empty", "parentId":null, "depth":0, "visible":true},
            {"id":"layer-2", "name":"Painted", "parentId":null, "depth":0, "visible":true},
            {"id":"layer-1", "name":"Load Image", "parentId":null, "depth":0, "visible":true}
        ])
    );
    let targets = json!([
        {"kind":"composition"}, {"kind":"layer", "layerId":"layer-1"},
        {"kind":"layer", "layerId":"layer-2"}, {"kind":"current-layer"},
        {"kind":"layer", "layerId":"layer-3"}, {"kind":"selection"},
        {"kind":"composition-from-layer", "layerId":"layer-2"}
    ]);
    let full = inputs.ask("source.request", Some(capture(&image["document"], targets, 0)))?;
    let snapshots = full["result"]["snapshots"].as_array().ok_or("Capture has no snapshots")?;
    assert_eq!(snapshots.len(), 7);
    assert!(snapshots.iter().all(|snapshot| snapshot["revision"] == snapshots[0]["revision"]));
    assert!(snapshots.iter().all(|snapshot| snapshot["document"] == image["document"]));
    let composition = pixels(&snapshots[0])?;
    assert_eq!(
        (snapshots[0]["pixels"]["width"].clone(), snapshots[0]["pixels"]["format"].clone()),
        (json!(8), json!("rgba8"))
    );
    for (index, pixel) in composition.chunks_exact(4).enumerate() {
        let (x, y) = (index as i32 % 8, index as i32 / 8);
        let expected = [x * 30, y * 40, 100, 255];
        for (actual, expected) in pixel.iter().zip(expected) {
            assert!((i32::from(*actual) - expected).abs() <= 1, "pixel {x},{y}: {pixel:?}");
        }
    }
    assert_eq!(pixels(&snapshots[1])?, composition);
    assert_eq!(pixels(&snapshots[6])?, composition);
    let painted = pixels(&snapshots[2])?;
    for (index, pixel) in painted.chunks_exact(4).enumerate() {
        let expected = if index % 8 < 4 { [255, 255, 255, 0] } else { [0, 0, 0, 255] };
        assert_eq!(pixel, expected, "mask pixel {index}");
    }
    assert_eq!(pixels(&snapshots[3])?, painted);
    assert!(pixels(&snapshots[4])?.chunks_exact(4).all(|pixel| pixel == [0, 0, 0, 255]));
    assert_eq!(snapshots[5]["pixels"]["format"], "coverage8");
    assert_eq!(pixels(&snapshots[5])?, vec![0; 48]);
    // A preview keeps the aspect, never enlarges and averages exact areas.
    let mask = json!([{"kind":"layer", "layerId":"layer-2"}]);
    let preview =
        inputs.ask("source.request", Some(capture(&image["document"], mask.clone(), 3)))?;
    let snapshot = &preview["result"]["snapshots"][0];
    assert_eq!(
        (snapshot["pixels"]["width"].clone(), snapshot["pixels"]["height"].clone()),
        (json!(3), json!(2))
    );
    // The middle column covers half of the covered source columns; exact halves may round
    // either way.
    let reduced = pixels(snapshot)?;
    for row in reduced.chunks_exact(12) {
        assert_eq!(row[..4], [255, 255, 255, 0]);
        assert_eq!(row[8..], [0, 0, 0, 255]);
        assert!((127..=128).contains(&row[4]) && row[4..7].iter().all(|value| *value == row[4]));
        assert_eq!(u16::from(row[4]) + u16::from(row[7]), 255);
    }
    let large = inputs.ask("source.request", Some(capture(&image["document"], mask, 1024)))?;
    assert_eq!(large["result"]["snapshots"][0]["pixels"]["width"], 8);
    // Parked documents capture too; without an active mask they have no current layer.
    let parked = json!([{"kind":"composition"}]);
    let blank = inputs.ask("source.request", Some(capture(&latent["document"], parked, 0)))?;
    let backing = pixels(&blank["result"]["snapshots"][0])?;
    assert_eq!(backing.len(), 5 * 4 * 4);
    assert!(backing.chunks_exact(4).all(|pixel| pixel == [24, 24, 24, 255]), "{backing:?}");
    let current = json!([{"kind":"current-layer"}]);
    let missing = inputs.ask("source.request", Some(capture(&latent["document"], current, 0)))?;
    assert_eq!(missing["result"]["status"], "unavailable");
    // Changed geometry is stale; unknown documents and layers are unavailable.
    let mut changed = image["document"].clone();
    changed["geometryRevision"] = json!("input-1-1x1");
    let stale = inputs
        .ask("source.request", Some(capture(&changed, json!([{"kind":"composition"}]), 0)))?;
    assert_eq!(stale["result"]["status"], "stale");
    let mut unknown = image["document"].clone();
    unknown["id"] = json!("output-7");
    let foreign = inputs
        .ask("source.request", Some(capture(&unknown, json!([{"kind":"composition"}]), 0)))?;
    assert_eq!(foreign["result"]["status"], "unavailable");
    let absent = json!([{"kind":"layer", "layerId":"layer-9"}]);
    let gone = inputs.ask("source.request", Some(capture(&image["document"], absent, 0)))?;
    assert_eq!(gone["result"]["status"], "unavailable");
    inputs.close()
}

#[test]
fn watching_reports_each_changed_document_revision() -> Result<(), Box<dyn Error>> {
    let directory = anchor()?;
    let mut inputs = Inputs::start(directory.path())?;
    let result = inputs.workflow(sync(None))?;
    let image = synced(&result, "image")?.clone();
    let watching = inputs.ask("source.request", Some(json!({"type":"watch", "enabled":true})))?;
    assert_eq!(watching["result"], json!({"kind":"watching", "supported":true}));
    let before = inputs.changes.len();
    let appearance = json!({"type":"appearance", "workflow":WORKFLOW, "masks":[
        {"surface":"image", "mask":"painted", "color":"#123456", "opacity":0.25}]});
    inputs.workflow(appearance)?;
    inputs.until("No source change was reported", |inputs| inputs.changes.len() > before)?;
    let change = &inputs.changes[before];
    assert_eq!(change["type"], "source.changed");
    assert_eq!(change["requestId"], 0);
    assert_eq!(change["change"]["documentId"], image["document"]["id"]);
    let off = inputs.ask("source.request", Some(json!({"type":"watch", "enabled":false})))?;
    assert_eq!(off["result"]["supported"], true);
    inputs.close()
}

#[test]
fn previews_follow_a_stroke_while_it_is_drawn_and_watching_reports_each_step()
-> Result<(), Box<dyn Error>> {
    let directory = anchor()?;
    let mut inputs = Inputs::start(directory.path())?;
    let result = inputs.workflow(sync(None))?;
    let image = synced(&result, "image")?.clone();
    let empty = layer(&image, "empty")?;
    inputs.workflow(activate(Some("image"), Some("empty"), "input"))?;
    inputs.context_until(|document| document["id"] == image["document"]["id"])?;
    let watching = inputs.ask("source.request", Some(json!({"type":"watch", "enabled":true})))?;
    assert_eq!(watching["result"]["kind"], "watching");
    let mask = json!([{"kind":"layer", "layerId":empty}]);
    let preview = |inputs: &mut Inputs| -> Result<Value, Box<dyn Error>> {
        let reply =
            inputs.ask("source.request", Some(capture(&image["document"], mask.clone(), 4)))?;
        Ok(reply["result"]["snapshots"][0].clone())
    };
    let blank = preview(&mut inputs)?;
    let brush = inputs.ask("edit.request", Some(json!({"type":"tool", "tool":"brush"})))?;
    assert_eq!(brush["result"]["state"]["tool"], "brush", "{brush}");
    let before = inputs.changes.len();
    inputs.drag(&[[2.0, 3.0], [6.0, 3.0]])?;
    // While the button is still pressed, the canvas shows the staged stroke.
    let staged_step = |inputs: &Inputs| {
        inputs.changes[before..]
            .iter()
            .rev()
            .find(|change| change["change"]["documentId"] == image["document"]["id"])
            .filter(|change| {
                change["change"]["revision"].as_str().is_some_and(|text| text.contains('+'))
            })
            .cloned()
    };
    inputs.until("No staged stroke step was reported", |inputs| staged_step(inputs).is_some())?;
    let staged = staged_step(&inputs).ok_or("The staged stroke step vanished")?;
    let drawing = preview(&mut inputs)?;
    assert_ne!(drawing["pixels"], blank["pixels"], "the preview shows the stroke being drawn");
    assert_eq!(drawing["revision"], blank["revision"], "the stroke is not yet published");
    let state = inputs.ask("edit.request", Some(json!({"type":"state"})))?;
    assert_eq!(state["result"]["state"]["history"]["undo"], false, "{state}");
    // Releasing publishes it; watching reports the new published revision without a step.
    let reported = inputs.changes.len();
    inputs.release()?;
    inputs.until(&format!("The published stroke was not reported: {staged}"), |inputs| {
        inputs.changes[reported..].iter().any(|change| {
            change["change"]["revision"].as_str().is_some_and(|text| !text.contains('+'))
        })
    })?;
    let published = preview(&mut inputs)?;
    assert_ne!(published["revision"], blank["revision"]);
    assert_eq!(published["pixels"], drawing["pixels"]);
    inputs.close()
}

#[test]
fn watching_reports_a_document_the_input_route_shows_again() -> Result<(), Box<dyn Error>> {
    let directory = anchor()?;
    let mut inputs = Inputs::start(directory.path())?;
    let result = inputs.workflow(sync(None))?;
    let image = synced(&result, "image")?.clone();
    inputs.workflow(activate(Some("image"), Some("painted"), "input"))?;
    inputs.context_until(|document| document["id"] == image["document"]["id"])?;
    inputs.ask("source.request", Some(json!({"type":"watch", "enabled":true})))?;
    let until = |inputs: &mut Inputs,
                 since: usize,
                 document: &Value|
     -> Result<(), Box<dyn Error>> {
        inputs.until(&format!("No change named {document}"), |inputs| {
            inputs.changes[since..].iter().any(|change| change["change"]["documentId"] == *document)
        })
    };
    let hidden = inputs.changes.len();
    inputs.workflow(activate(None, None, "output"))?;
    until(&mut inputs, hidden, &Value::Null)?;
    // Showing the unchanged document again lets sources that failed without it resolve.
    let shown = inputs.changes.len();
    inputs.workflow(activate(Some("image"), Some("painted"), "input"))?;
    until(&mut inputs, shown, &image["document"]["id"])?;
    inputs.close()
}
