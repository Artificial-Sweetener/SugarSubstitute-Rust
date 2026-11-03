// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The protocol form of editing state and the revisions reports carry.

use super::*;
use sugarsubstitute_canvas::{LayerId, SessionState};
use sugarsubstitute_contracts::canvas_native::{CanvasLayerId, CanvasToolId};

fn state(brush: BrushOptions) -> EditState {
    EditState {
        document: None,
        active_mask: LayerId::new(3).ok(),
        tool: CanvasToolId::Brush,
        tools: CanvasToolId::ALL.map(|tool| (tool, tool == CanvasToolId::PanZoom)),
        brush,
        eraser: BrushOptions::DEFAULT,
        selection: Some([10.0, 20.0, 30.0, 40.0]),
        history: [true, false],
        session: Some(SessionState { kind: SessionKind::Transform, undo: true, redo: false }),
        coverage_mode: true,
    }
}

#[test]
fn reports_carry_protocol_values_and_advance_their_revision_only_on_change()
-> Result<(), Box<dyn Error>> {
    let mut surface = NativeSurface::new();
    let first = surface.report_edit_state(state(BrushOptions::new(37, 0.255, 0.5)?))?;
    assert_eq!(first.revision, 1);
    assert_eq!(first.brush, CanvasBrushOptions::new(37, 26, 50)?, "fractions round to percent");
    assert_eq!(first.active_mask, Some(CanvasLayerId::new("layer-3")?));
    assert_eq!(first.tools.len(), 12);
    assert_eq!((first.undo, first.redo), (true, false));
    assert_eq!(first.mode, CanvasEditMode::Coverage);
    let session =
        CanvasEditSession { kind: CanvasEditSessionKind::Transform, undo: true, redo: false };
    assert_eq!(first.session, Some(session));
    assert_eq!(first.selection, None, "without an allocation there are no CSS bounds");
    let same = surface.report_edit_state(state(BrushOptions::new(37, 0.255, 0.5)?))?;
    assert_eq!(same.revision, 1, "an unchanged state keeps its revision");
    let changed = surface.report_edit_state(state(BrushOptions::new(38, 0.255, 0.5)?))?;
    assert_eq!(changed.revision, 2);
    assert_eq!(surface.editing_deadline(Moment::from_nanos(0)), None, "nothing is held back");
    Ok(())
}
