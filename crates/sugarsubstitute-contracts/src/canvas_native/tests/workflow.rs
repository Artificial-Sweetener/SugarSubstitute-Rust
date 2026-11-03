// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise workflow provisioning commands and replies against the shared vectors.

use super::*;
use crate::canvas_surface::{CanvasFileReference, CanvasFileStorage};

const WORKFLOW: &str =
    include_str!("../../../../../third_party/sugarcubes-canvas/workflow_vectors.json");

const REQUESTS: [&str; 8] = [
    "sync",
    "emptySync",
    "activateInput",
    "activateOutput",
    "appearance",
    "resizeCanvas",
    "resizeScale",
    "hover",
];
const REPLIES: [&str; 4] = ["synced", "activated", "applied", "failure"];

fn command(vectors: &Value, name: &str) -> Result<CanvasWorkflowCommand, Box<dyn Error>> {
    Ok(CanvasWorkflowRequest::decode(&bytes(&vectors["valid"][name])?)?.command().clone())
}

#[test]
fn sync_decodes_surfaces_masks_files_and_colours() -> TestResult {
    let vectors = vectors(WORKFLOW)?;
    let CanvasWorkflowCommand::Sync { workflow, surfaces } = command(&vectors, "sync")? else {
        return Err("Sync did not decode as a sync".into());
    };
    assert_eq!(workflow.as_str(), "workflow:a");
    let [image, latent] = surfaces.as_slice() else { return Err("Expected two surfaces".into()) };
    assert_eq!(image.key.as_str(), "node:12:image");
    assert_eq!(image.label.as_str(), "Load Image");
    let CanvasWorkflowSurfaceBase::File(file) = &image.base else {
        return Err("Image surface lost its file".into());
    };
    assert_eq!((file.filename(), file.subfolder()), ("portrait.png", "masks"));
    assert_eq!(file.storage(), CanvasFileStorage::Input);
    let [first, second] = image.masks.as_slice() else { return Err("Expected two masks".into()) };
    assert_eq!(first.color, CanvasColor { red: 0x3f, green: 0xa9, blue: 0xf5 });
    assert_eq!(first.opacity.get(), 0.5);
    assert_eq!(first.channel, CanvasMaskChannel::Alpha);
    assert_eq!(
        first.initial.as_ref().map(CanvasFileReference::filename),
        Some("portrait-mask.png")
    );
    assert!(second.initial.is_none());
    assert_eq!(latent.base, CanvasWorkflowSurfaceBase::Blank(CanvasDimensions::new(1024, 768)?));
    assert!(latent.masks.is_empty());
    assert!(matches!(
        command(&vectors, "emptySync")?,
        CanvasWorkflowCommand::Sync { surfaces, .. } if surfaces.is_empty()
    ));
    Ok(())
}

#[test]
fn activation_appearance_resize_and_hover_decode_exactly() -> TestResult {
    let vectors = vectors(WORKFLOW)?;
    let key = |value: &str| CanvasSurfaceKey::new(value);
    let mask = |value: &str| CanvasMaskKey::new(value);
    let workflow = CanvasWorkflowKey::new("workflow:a")?;
    assert_eq!(
        command(&vectors, "activateInput")?,
        CanvasWorkflowCommand::Activate {
            workflow: workflow.clone(),
            route: CanvasWorkflowRoute::Input,
            surface: Some(key("node:12:image")?),
            mask: Some(mask("node:13:mask")?),
        }
    );
    assert_eq!(
        command(&vectors, "activateOutput")?,
        CanvasWorkflowCommand::Activate {
            workflow: workflow.clone(),
            route: CanvasWorkflowRoute::Output,
            surface: None,
            mask: None,
        }
    );
    assert_eq!(
        command(&vectors, "appearance")?,
        CanvasWorkflowCommand::Appearance {
            workflow: workflow.clone(),
            masks: vec![CanvasWorkflowAppearance {
                surface: key("node:12:image")?,
                mask: mask("node:13:mask")?,
                color: CanvasColor { red: 0, green: 0, blue: 0 },
                opacity: CanvasOpacity::new(1.0)?,
            }],
        }
    );
    assert_eq!(
        command(&vectors, "resizeCanvas")?,
        CanvasWorkflowCommand::Resize {
            workflow: workflow.clone(),
            surface: key("authority:7")?,
            size: CanvasDimensions::new(1, 300_000)?,
            mode: CanvasWorkflowResizeMode::Canvas(CanvasResizeAnchor::BottomRight),
        }
    );
    assert_eq!(
        command(&vectors, "resizeScale")?,
        CanvasWorkflowCommand::Resize {
            workflow: workflow.clone(),
            surface: key("authority:7")?,
            size: CanvasDimensions::new(512, 512)?,
            mode: CanvasWorkflowResizeMode::Scale(CanvasResampling::Smooth),
        }
    );
    assert_eq!(
        command(&vectors, "hover")?,
        CanvasWorkflowCommand::Hover { workflow, surface: key("node:12:image")?, mask: None }
    );
    Ok(())
}

#[test]
fn every_shared_rejection_is_refused_in_its_direction() -> TestResult {
    let vectors = vectors(WORKFLOW)?;
    for name in REPLIES {
        assert!(accepted(&vectors["valid"][name], wire::CanvasWorkflowReply::valid), "{name}");
    }
    for name in REQUESTS {
        assert!(CanvasWorkflowRequest::decode(&bytes(&vectors["valid"][name])?).is_ok(), "{name}");
    }
    for (name, base, value) in rejections(&vectors)? {
        let decoded = if REPLIES.contains(&base.as_str()) {
            accepted(&value, wire::CanvasWorkflowReply::valid)
        } else {
            CanvasWorkflowRequest::decode(&bytes(&value)?).is_ok()
        };
        assert!(!decoded, "{name} was accepted");
    }
    Ok(())
}

#[test]
fn reply_encoders_reproduce_the_shared_messages() -> TestResult {
    let vectors = vectors(WORKFLOW)?;
    let request = CanvasWorkflowRequest::decode(&bytes(&vectors["valid"]["sync"])?)?;
    let encoded = |payload: CanvasNativeReplyPayload| -> Result<Value, Box<dyn Error>> {
        assert_eq!(payload.request_id(), 9);
        Ok(serde_json::from_slice(payload.as_bytes())?)
    };
    let synced = CanvasWorkflowSyncedSurface {
        key: CanvasSurfaceKey::new("node:12:image")?,
        document: document()?,
        masks: vec![
            CanvasWorkflowSyncedMask {
                key: CanvasMaskKey::new("node:13:mask")?,
                layer: CanvasLayerId::new("layer:mask-1")?,
            },
            CanvasWorkflowSyncedMask {
                key: CanvasMaskKey::new("node:14:mask")?,
                layer: CanvasLayerId::new("layer:mask-2")?,
            },
        ],
    };
    assert_eq!(
        encoded(request.synced_reply(std::slice::from_ref(&synced))?)?,
        vectors["valid"]["synced"]
    );
    assert_eq!(encoded(request.activated_reply()?)?, vectors["valid"]["activated"]);
    assert_eq!(encoded(request.applied_reply()?)?, vectors["valid"]["applied"]);
    let failure = CanvasFailure {
        status: CanvasFailureStatus::Stale,
        message: CanvasFailureMessage::new("The workflow changed.")?,
    };
    assert_eq!(encoded(request.failure_reply(&failure)?)?, vectors["valid"]["failure"]);
    assert!(request.synced_reply(&vec![synced.clone(); 65]).is_err());
    let mut crowded = synced;
    crowded.masks = vec![crowded.masks[0].clone(); 33];
    assert!(request.synced_reply(&[crowded]).is_err());
    Ok(())
}

#[test]
fn a_full_sync_beyond_the_inline_lane_still_decodes_within_its_bound() -> TestResult {
    let file = json!({"filename": "portrait.png", "subfolder": "masks", "type": "input"});
    let masks: Vec<Value> = (0..32)
        .map(|index| {
            json!({"key": format!("node:{index}:mask"), "label": "Mask", "color": "#3fa9f5",
            "opacity": 0.5, "initial": file, "channel": "alpha"})
        })
        .collect();
    let surfaces: Vec<Value> = (0..64)
        .map(|index| {
            json!({"key": format!("node:{index}:image"), "label": "Load Image",
            "base": {"kind": "file", "file": file}, "masks": masks})
        })
        .collect();
    let encoded = bytes(&request(
        "workflow.request",
        json!({"type": "sync", "workflow": "workflow-1", "surfaces": surfaces}),
    ))?;
    assert!(encoded.len() > 16 * 1024 && encoded.len() <= MAX_CANVAS_WORKFLOW_REQUEST_BYTES);
    let CanvasNativeRequest::Workflow(request) = CanvasNativeRequest::decode(&encoded)? else {
        return Err("Sync did not decode as a workflow request".into());
    };
    assert!(matches!(
        request.command(),
        CanvasWorkflowCommand::Sync { surfaces, .. } if surfaces.len() == 64 && surfaces[63].masks.len() == 32
    ));
    Ok(())
}
