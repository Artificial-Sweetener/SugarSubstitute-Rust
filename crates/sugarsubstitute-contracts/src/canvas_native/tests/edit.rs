// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise edit commands, state replies and `edit.changed` against the shared vectors.

use super::*;

const EDITING: &str =
    include_str!("../../../../../third_party/sugarcubes-canvas/editing_vectors.json");

const REQUESTS: [&str; 13] = [
    "state",
    "tool",
    "options",
    "history",
    "commit",
    "cancel",
    "nudge",
    "selection",
    "modify",
    "mode",
    "transform",
    "session",
    "navigation",
];

fn options(size: u16, hardness: u8, opacity: u8) -> Result<CanvasBrushOptions, Box<dyn Error>> {
    Ok(CanvasBrushOptions::new(size, hardness, opacity)?)
}

/// The populated state the shared `stateReply` and `changed` vectors carry.
fn editing_state() -> Result<CanvasEditState, Box<dyn Error>> {
    Ok(CanvasEditState {
        revision: 42,
        document: Some(CanvasDocumentId::new("input:surface-1")?),
        active_mask: Some(CanvasLayerId::new("layer:mask-1")?),
        tool: CanvasToolId::Brush,
        tools: CanvasToolId::ALL
            .iter()
            .map(|&id| CanvasEditTool { id, available: id != CanvasToolId::SharedEdges })
            .collect(),
        brush: options(40, 80, 100)?,
        eraser: options(1, 0, 0)?,
        selection: Some(CanvasEditSelectionBounds::new(-12.5, 30.0, 200.25, 0.5)?),
        undo: true,
        redo: false,
        session: Some(CanvasEditSession {
            kind: CanvasEditSessionKind::SharedEdges,
            undo: false,
            redo: true,
        }),
        mode: CanvasEditMode::Coverage,
    })
}

#[test]
fn every_shared_command_decodes_to_its_typed_intent() -> TestResult {
    let vectors = vectors(EDITING)?;
    let commands = REQUESTS
        .iter()
        .map(|name| Ok(CanvasEditRequest::decode(&bytes(&vectors["valid"][*name])?)?.command()))
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    assert_eq!(
        commands,
        [
            CanvasEditCommand::State,
            CanvasEditCommand::Tool(CanvasToolId::SelectLasso),
            CanvasEditCommand::Options {
                tool: CanvasBrushTool::Eraser,
                options: options(1000, 0, 100)?
            },
            CanvasEditCommand::History(CanvasHistoryAction::Undo),
            CanvasEditCommand::Commit,
            CanvasEditCommand::Cancel,
            CanvasEditCommand::Nudge { dx: -100, dy: 100 },
            CanvasEditCommand::Selection(CanvasSelectionAction::Clear),
            CanvasEditCommand::Modify(CanvasCoverageModify {
                target: CanvasModifyTarget::Mask,
                operation: CanvasModifyOperation::Feather,
                pixels: 999,
                phase: CanvasModifyPhase::Preview,
            }),
            CanvasEditCommand::Mode(CanvasEditMode::Coverage),
            CanvasEditCommand::Transform(CanvasTransformAction::FlipVertical),
            CanvasEditCommand::Session(CanvasSessionAction::Apply),
            CanvasEditCommand::Navigation { held: true },
        ]
    );
    Ok(())
}

#[test]
fn every_shared_rejection_is_refused_in_its_direction() -> TestResult {
    let vectors = vectors(EDITING)?;
    let mut refused = 0;
    for (name, base, value) in rejections(&vectors)? {
        let decoded = match base.as_str() {
            "changed" => accepted(&value, wire::CanvasEditChanged::valid),
            "stateReply" | "failureReply" | "idleReply" => {
                accepted(&value, wire::CanvasEditReply::valid)
            }
            _ => CanvasEditRequest::decode(&bytes(&value)?).is_ok(),
        };
        assert!(!decoded, "{name} was accepted");
        refused += 1;
    }
    assert!(refused >= 40);
    Ok(())
}

#[test]
fn state_failure_and_change_encoders_reproduce_the_shared_messages() -> TestResult {
    let vectors = vectors(EDITING)?;
    let request = CanvasEditRequest::decode(&bytes(&vectors["valid"]["state"])?)?;
    let same = |encoded: &[u8], expected: &Value| -> Result<bool, Box<dyn Error>> {
        // Compare through the generated shape: `30` and `30.0` are the same JSON number.
        let actual: wire::CanvasEditReply = serde_json::from_slice(encoded)?;
        Ok(actual.valid() && actual == serde_json::from_value(expected.clone())?)
    };
    let state = editing_state()?;
    assert!(same(request.state_reply(&state)?.as_bytes(), &vectors["valid"]["stateReply"])?);
    let idle = CanvasEditState {
        revision: 0,
        document: None,
        active_mask: None,
        tool: CanvasToolId::PanZoom,
        tools: Vec::new(),
        brush: options(40, 80, 100)?,
        eraser: options(40, 80, 100)?,
        selection: None,
        undo: false,
        redo: false,
        session: None,
        mode: CanvasEditMode::None,
    };
    assert!(same(request.state_reply(&idle)?.as_bytes(), &vectors["valid"]["idleReply"])?);
    let failure = CanvasFailure {
        status: CanvasFailureStatus::Unsupported,
        message: CanvasFailureMessage::new("Editing is not ready.")?,
    };
    let encoded: Value = serde_json::from_slice(request.failure_reply(&failure)?.as_bytes())?;
    assert_eq!(encoded, vectors["valid"]["failureReply"]);
    let changed = state.changed(&session()?)?;
    assert_eq!(changed.kind(), CanvasNativeNotificationKind::EditChanged);
    let actual: wire::CanvasEditChanged = serde_json::from_slice(changed.as_bytes())?;
    assert!(actual.valid());
    assert_eq!(actual, serde_json::from_value(vectors["valid"]["changed"].clone())?);
    Ok(())
}

#[test]
fn edit_values_reject_out_of_range_state() -> TestResult {
    assert!(CanvasBrushOptions::new(0, 0, 0).is_err());
    assert!(CanvasBrushOptions::new(1001, 0, 0).is_err());
    assert!(CanvasBrushOptions::new(1, 101, 0).is_err());
    assert!(CanvasEditSelectionBounds::new(0.0, 0.0, 0.0, 1.0).is_err());
    assert!(CanvasEditSelectionBounds::new(-1_000_001.0, 0.0, 1.0, 1.0).is_err());
    assert!(CanvasEditSelectionBounds::new(f64::NAN, 0.0, 1.0, 1.0).is_err());
    let mut state = editing_state()?;
    state.revision = 1 << 53;
    assert!(state.changed(&session()?).is_err());
    state.revision = (1 << 53) - 1;
    state.tools.push(CanvasEditTool { id: CanvasToolId::Move, available: true });
    assert!(state.changed(&session()?).is_err());
    Ok(())
}
