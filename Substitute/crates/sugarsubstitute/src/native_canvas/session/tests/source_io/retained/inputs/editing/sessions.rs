// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Move, Transform and Resize shared edges through `edit.request`: fractional and snapped moves
//! with smart guides, nudges, edit sessions whose undo and redo stay inside them, apply and
//! cancel, and the cursors each tool asks for.

use super::*;
use sugarsubstitute_canvas::{ResizeDirection, SessionState};
use sugarsubstitute_contracts::canvas_native::{CanvasSessionAction, CanvasTransformAction};

const CTRL: ToolModifiers = ToolModifiers { shift: false, ctrl: true, alt: false };

impl Fixture {
    /// Drag the primary button through document points with `held` modifiers throughout.
    fn drag_held(
        &mut self,
        points: &[[f64; 2]],
        held: ToolModifiers,
    ) -> Result<(), Box<dyn Error>> {
        self.point(points[0], held)?;
        self.press(true, held)?;
        for point in &points[1..] {
            self.point(*point, held)?;
        }
        self.press(false, held)?;
        self.settle_inputs()
    }

    fn press(&mut self, pressed: bool, modifiers: ToolModifiers) -> Result<(), Box<dyn Error>> {
        let button = ToolInput::Button { button: ToolButton::Primary, pressed, modifiers };
        self.session.input(Input::Tool(button))
    }

    /// Fill `[left, top, right, bottom]` of the active mask with the mask rectangle tool.
    fn fill(&mut self, [left, top, right, bottom]: [f64; 4]) -> Result<(), Box<dyn Error>> {
        self.edit(CanvasEditCommand::Tool(CanvasToolId::MaskRectangle))?;
        self.drag_held(&[[left, top], [right, bottom]], CTRL)
    }

    fn history(&mut self, action: CanvasHistoryAction) -> Result<EditState, Box<dyn Error>> {
        let state = self.edit(CanvasEditCommand::History(action))?;
        self.settle_inputs()?;
        Ok(state)
    }

    /// The overlay's smart guides now.
    fn snaps(&self) -> Vec<sugarsubstitute_canvas::SnapGuide> {
        self.session.shown_overlay().map(|overlay| overlay.snaps.to_vec()).unwrap_or_default()
    }

    fn zoom(&self) -> f64 {
        self.session.document.tools.navigation().viewport().zoom()
    }
}

fn session(state: &EditState) -> Option<SessionState> {
    state.session
}

const fn open(kind: SessionKind, undo: bool, redo: bool) -> Option<SessionState> {
    Some(SessionState { kind, undo, redo })
}

fn partial(value: u16) -> bool {
    value > 0 && value < FULL_COVERAGE
}

#[test]
fn move_lands_fractional_and_snapped_drags_and_floats_selections_in_a_session()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    {
        let image = show(&mut fixture, Some("first"))?;
        let first = layer(&image, "first")?;
        fixture.fill([10.0, 10.0, 30.0, 30.0])?;
        let state = fixture.edit(CanvasEditCommand::Tool(CanvasToolId::Move))?;
        assert_eq!(state.tool, CanvasToolId::Move);
        fixture.point([20.0, 20.0], NONE)?;
        assert_eq!(fixture.session.canvas_cursor(), CursorIntent::Move);
        let revision = fixture.snapshot(image.document)?.revision();

        // Without a selection the mask follows the pointer by fractions of a pixel.
        fixture.drag_held(&[[20.0, 20.0], [25.5, 20.0]], CTRL)?;
        let moved = fixture.snapshot(image.document)?;
        assert_eq!(moved.revision(), revision + 1, "the move is one undo step");
        let mask = coverage(&moved, first)?;
        assert!(partial(mask.value(15, 20)) && partial(mask.value(35, 20)));
        assert_eq!([mask.value(10, 20), mask.value(25, 20)], [0, FULL_COVERAGE]);
        fixture.history(CanvasHistoryAction::Undo)?;
        let restored = coverage(&fixture.snapshot(image.document)?, first)?;
        assert_eq!([restored.value(10, 20), restored.value(31, 20)], [FULL_COVERAGE, 0]);

        // Close to the canvas edge the mask snaps onto it and shows a guide until release.
        let near = 10.0 - 2.0 / fixture.zoom();
        fixture.point([20.0, 20.0], NONE)?;
        fixture.press(true, NONE)?;
        fixture.point([20.0 - near, 20.0], NONE)?;
        let vertical = |guide: &sugarsubstitute_canvas::SnapGuide| {
            guide.start[0] == 0.0 && guide.end[0] == 0.0
        };
        assert!(fixture.snaps().iter().any(vertical), "{:?}", fixture.snaps());
        fixture.press(false, NONE)?;
        fixture.settle_inputs()?;
        assert!(fixture.snaps().is_empty());
        let snapped = coverage(&fixture.snapshot(image.document)?, first)?;
        assert_eq!(
            [snapped.value(0, 20), snapped.value(19, 20), snapped.value(20, 20)],
            [FULL_COVERAGE, FULL_COVERAGE, 0]
        );

        // A nudge is its own undo step.
        let revision = fixture.snapshot(image.document)?.revision();
        fixture.edit(CanvasEditCommand::Nudge { dx: 1, dy: 0 })?;
        fixture.settle_inputs()?;
        let nudged = fixture.snapshot(image.document)?;
        assert_eq!(nudged.revision(), revision + 1);
        assert_eq!(
            [coverage(&nudged, first)?.value(0, 20), coverage(&nudged, first)?.value(20, 20)],
            [0, FULL_COVERAGE]
        );

        // Selected pixels float in a Move session; Undo steps the session, not the document.
        fixture.edit(CanvasEditCommand::Tool(CanvasToolId::SelectRectangle))?;
        fixture.drag_held(&[[4.0, 12.0], [10.0, 18.0]], CTRL)?;
        fixture.edit(CanvasEditCommand::Tool(CanvasToolId::Move))?;
        let revision = fixture.snapshot(image.document)?.revision();
        fixture.drag_held(&[[6.0, 15.0], [12.0, 15.0]], CTRL)?;
        assert_eq!(session(&fixture.session.edit_state()?), open(SessionKind::Move, true, false));
        let state = fixture.edit(CanvasEditCommand::Nudge { dx: 0, dy: 1 })?;
        assert_eq!(session(&state), open(SessionKind::Move, true, false));
        fixture.settle_inputs()?;
        let undone = fixture.history(CanvasHistoryAction::Undo)?;
        assert_eq!(session(&undone), open(SessionKind::Move, true, true));
        assert_eq!(undone.history, [true, false], "the document's own history is untouched");
        assert_eq!(fixture.snapshot(image.document)?.revision(), revision);
        assert!(
            rejected(fixture.edit(CanvasEditCommand::Tool(CanvasToolId::Brush))).is_some(),
            "the session must be resolved before changing tools"
        );
        let committed = fixture.edit(CanvasEditCommand::Commit)?;
        fixture.settle_inputs()?;
        assert_eq!(session(&committed), None);
        let landed = fixture.snapshot(image.document)?;
        assert_eq!(landed.revision(), revision + 1, "the float lands as one step");
        let bounds = landed.selection().and_then(|selection| selection.bounds());
        assert_eq!(bounds, Some(sugarsubstitute_image_ops::Region::new(10, 12, 6, 6)?));
        let mask = coverage(&landed, first)?;
        assert_eq!([mask.value(5, 15), mask.value(14, 15)], [0, FULL_COVERAGE]);
    }
    fixture.finish().map(drop)
}

#[test]
fn transform_sessions_resolve_in_place_by_key_and_back_to_the_previous_tool_by_toolbar()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    {
        let image = show(&mut fixture, Some("first"))?;
        let first = layer(&image, "first")?;
        fixture.fill([10.0, 10.0, 40.0, 20.0])?;
        let state = fixture.edit(CanvasEditCommand::Tool(CanvasToolId::Transform))?;
        assert_eq!((state.tool, session(&state)), (CanvasToolId::Transform, None));
        let overlay = fixture.session.shown_overlay().ok_or("No transform box")?;
        assert!(matches!(overlay.guides, Some(sugarsubstitute_canvas::OverlayGuides::Frame(_))));
        // A corner handle shows a turned resize arrow; outside the box, a rotation cursor.
        fixture.point([40.0, 20.0], NONE)?;
        assert!(matches!(fixture.session.canvas_cursor(), CursorIntent::Affine(_)));
        let revision = fixture.snapshot(image.document)?.revision();

        let turn = |action| CanvasEditCommand::Transform(action);
        let turned = fixture.edit(turn(CanvasTransformAction::RotateRight))?;
        fixture.settle_inputs()?;
        assert_eq!(session(&turned), open(SessionKind::Transform, true, false));
        assert!(fixture.session.document.staged.is_some(), "the turn previews live");
        // Undo and Redo step the session's checkpoints, never the document.
        let undone = fixture.history(CanvasHistoryAction::Undo)?;
        assert_eq!(session(&undone), open(SessionKind::Transform, false, true));
        let redone = fixture.history(CanvasHistoryAction::Redo)?;
        assert_eq!(session(&redone), open(SessionKind::Transform, true, false));
        let undone = fixture.edit(CanvasEditCommand::Session(CanvasSessionAction::Undo))?;
        assert_eq!(session(&undone), open(SessionKind::Transform, false, true));
        fixture.edit(CanvasEditCommand::Session(CanvasSessionAction::Redo))?;
        assert_eq!(fixture.snapshot(image.document)?.revision(), revision);

        // Enter applies the turn as one step in place; Transform stays chosen.
        let applied = fixture.edit(CanvasEditCommand::Commit)?;
        fixture.settle_inputs()?;
        assert_eq!((applied.tool, session(&applied)), (CanvasToolId::Transform, None));
        let turned = fixture.snapshot(image.document)?;
        assert_eq!(turned.revision(), revision + 1);
        // The 30×10 bar turned a quarter about its centre (25, 15) stands 10×30.
        let mask = coverage(&turned, first)?;
        assert_eq!(
            [mask.value(25, 2), mask.value(12, 15), mask.value(25, 28)],
            [FULL_COVERAGE, 0, FULL_COVERAGE]
        );

        // Escape discards a flip in place; without a change Enter and Escape do nothing.
        fixture.edit(turn(CanvasTransformAction::FlipVertical))?;
        fixture.settle_inputs()?;
        let cancelled = fixture.edit(CanvasEditCommand::Cancel)?;
        fixture.settle_inputs()?;
        assert_eq!((cancelled.tool, session(&cancelled)), (CanvasToolId::Transform, None));
        assert_eq!(fixture.snapshot(image.document)?.revision(), revision + 1);
        assert!(fixture.session.document.staged.is_none());
        for command in [CanvasEditCommand::Commit, CanvasEditCommand::Cancel] {
            let unchanged = fixture.edit(command)?;
            assert_eq!((unchanged.tool, session(&unchanged)), (CanvasToolId::Transform, None));
        }
        assert_eq!(fixture.snapshot(image.document)?.revision(), revision + 1);

        // The toolbar's Cancel leaves Transform before any change, and its Apply after one.
        let left = fixture.edit(CanvasEditCommand::Session(CanvasSessionAction::Cancel))?;
        assert_eq!((left.tool, session(&left)), (CanvasToolId::MaskRectangle, None));
        fixture.edit(CanvasEditCommand::Tool(CanvasToolId::Transform))?;
        fixture.edit(turn(CanvasTransformAction::RotateLeft))?;
        fixture.settle_inputs()?;
        let applied = fixture.edit(CanvasEditCommand::Session(CanvasSessionAction::Apply))?;
        fixture.settle_inputs()?;
        assert_eq!((applied.tool, session(&applied)), (CanvasToolId::MaskRectangle, None));
        assert_eq!(fixture.snapshot(image.document)?.revision(), revision + 2);
        // Elsewhere, session actions without a session stay refused.
        assert!(fixture.edit(CanvasEditCommand::Session(CanvasSessionAction::Cancel)).is_err());
    }
    fixture.finish().map(drop)
}

#[test]
fn shared_edges_reshape_adjacent_masks_in_a_session_and_escape_leaves_the_tool()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    {
        let image = show(&mut fixture, Some("first"))?;
        let (first, second) = (layer(&image, "first")?, layer(&image, "second")?);
        fixture.fill([10.0, 10.0, 30.0, 30.0])?;
        fixture.session.activate_input(WORKFLOW, "image", Some("second"))?;
        fixture.settle_inputs()?;
        fixture.fill([30.0, 10.0, 50.0, 30.0])?;
        let state = fixture.edit(CanvasEditCommand::Tool(CanvasToolId::SharedEdges))?;
        assert_eq!(state.tool, CanvasToolId::SharedEdges);
        let overlay = fixture.session.shown_overlay().ok_or("No seams")?;
        assert!(matches!(overlay.guides, Some(sugarsubstitute_canvas::OverlayGuides::Seams(_))));
        fixture.point([30.0, 20.0], NONE)?;
        let resize = CursorIntent::Resize(ResizeDirection::Horizontal);
        assert_eq!(fixture.session.canvas_cursor(), resize);
        let revision = fixture.snapshot(image.document)?.revision();

        fixture.drag_held(&[[30.0, 20.0], [36.0, 20.0]], CTRL)?;
        let dragged = fixture.session.edit_state()?;
        assert_eq!(session(&dragged), open(SessionKind::SharedEdges, true, false));
        let staged = fixture.session.document.staged.as_ref().ok_or("No seam preview")?;
        let shown = [coverage(&staged.snapshot, first)?, coverage(&staged.snapshot, second)?];
        assert_eq!([shown[0].value(33, 20), shown[1].value(33, 20)], [FULL_COVERAGE, 0]);
        let undone = fixture.history(CanvasHistoryAction::Undo)?;
        assert_eq!(session(&undone), open(SessionKind::SharedEdges, false, true));
        fixture.history(CanvasHistoryAction::Redo)?;
        assert_eq!(fixture.snapshot(image.document)?.revision(), revision);

        let applied = fixture.edit(CanvasEditCommand::Session(CanvasSessionAction::Apply))?;
        fixture.settle_inputs()?;
        assert_eq!(session(&applied), None);
        let reshaped = fixture.snapshot(image.document)?;
        assert_eq!(reshaped.revision(), revision + 1, "both masks change in one step");
        let masks = [coverage(&reshaped, first)?, coverage(&reshaped, second)?];
        assert_eq!([masks[0].value(33, 20), masks[1].value(33, 20)], [FULL_COVERAGE, 0]);
        assert_eq!(masks[1].value(40, 20), FULL_COVERAGE);

        // Escape with no session leaves for the tool chosen before.
        let left = fixture.edit(CanvasEditCommand::Cancel)?;
        assert_eq!(left.tool, CanvasToolId::MaskRectangle);
    }
    fixture.finish().map(drop)
}
