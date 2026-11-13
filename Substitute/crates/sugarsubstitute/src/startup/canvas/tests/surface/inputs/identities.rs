// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Every message family names an input document and its layers by the one protocol identity
//! the input-document owner assigns.

use super::*;
use crate::native_canvas::session::inputs::{layer_from_protocol_id, layer_protocol_id};
use sugarsubstitute_canvas::LayerId;

#[test]
fn layer_protocol_identities_round_trip_and_reject_non_canonical_text() -> Result<(), Box<dyn Error>>
{
    let layer = LayerId::new(42)?;
    assert_eq!(layer_protocol_id(layer), "layer-42");
    assert_eq!(layer_from_protocol_id("layer-42"), Some(layer));
    for text in ["42", "layer-", "layer-042", "layer-+42", "layer-0", "input-42", "layer-4x"] {
        assert_eq!(layer_from_protocol_id(text), None, "{text}");
    }
    Ok(())
}

#[test]
fn workflow_identities_name_the_same_document_and_mask_in_every_family()
-> Result<(), Box<dyn Error>> {
    let directory = anchor()?;
    let mut inputs = Inputs::start(directory.path())?;
    let result = inputs.workflow(sync(Some("mask.png")))?;
    let image = synced(&result, "image")?.clone();
    let document = image["document"].clone();
    let painted = layer(&image, "painted")?;
    inputs.workflow(activate(Some("image"), Some("painted"), "input"))?;

    // context.reply and geometry carry the synced document.
    let context = inputs.context_until(|shown| shown["id"] == document["id"])?;
    assert_eq!(context["context"]["document"], document);
    let geometry = inputs.ask("geometry.request", Some(json!({"type":"context"})))?;
    assert_eq!(geometry["result"]["document"], document);

    // source context names the synced mask as the current layer and lists it.
    let sources = inputs.ask("source.request", Some(json!({"type":"context"})))?;
    assert_eq!(sources["result"]["document"], document);
    assert_eq!(sources["result"]["currentLayerId"], painted.as_str());
    let listed = sources["result"]["layers"].as_array().ok_or("No layers")?;
    assert!(listed.iter().any(|entry| entry["id"] == painted.as_str()), "{listed:?}");

    // source.capture accepts the synced document and mask layer as workflow.reply named them.
    let targets = json!([{"kind":"layer", "layerId":painted}]);
    let capture = json!({"type":"capture", "document":document, "targets":targets,
        "maximumSize":0});
    let captured = inputs.ask("source.request", Some(capture))?;
    let snapshot = &captured["result"]["snapshots"][0];
    assert_eq!(snapshot["document"], document, "{captured}");
    assert_eq!(snapshot["pixels"]["format"], "rgba8");

    // The editing state names the same document and active mask.
    let state = inputs.ask("edit.request", Some(json!({"type":"state"})))?;
    assert_eq!(state["result"]["kind"], "state", "{state}");
    assert_eq!(state["result"]["state"]["document"], document["id"]);
    assert_eq!(state["result"]["state"]["activeMask"], painted.as_str());
    inputs.close()
}
