// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Editing commands, tools, deferred coverage work, staged frames and overlays on a shown
//! input document, through the real session, jobs and frame pipeline.

use super::*;
use crate::native_canvas::session::editing::{EditRejection, EditState};
use sugarsubstitute_canvas::{
    BrushOptions, CursorIntent, PointerSample, ScrollPhase, ToolButton, ToolInput, ToolModifiers,
};
use sugarsubstitute_contracts::canvas_native::{
    CanvasBrushOptions, CanvasBrushTool, CanvasCoverageModify, CanvasEditCommand, CanvasEditMode,
    CanvasHistoryAction, CanvasModifyOperation, CanvasModifyPhase, CanvasModifyTarget,
    CanvasSelectionAction, CanvasToolId,
};
use sugarsubstitute_image_ops::FULL_COVERAGE;

const NONE: ToolModifiers = ToolModifiers::NONE;

/// Show a 64×48 red input image with masks `first` and `second`, targeting `mask`, beside a
/// blank `canvas` surface with mask `only`.
fn show(fixture: &mut Fixture, mask: Option<&str>) -> Result<SyncedSurface, Box<dyn Error>> {
    let directory = fixture._directory.path().to_path_buf();
    let red = image(&directory, "red.png", Extent::new(64, 48)?, |_, _| [255, 0, 0, 255])?;
    let reply = fixture.sync(vec![
        surface(
            "image",
            SurfaceBase::File(file("input/red.png", &red).0),
            vec![super::mask("first", None)?, super::mask("second", None)?],
        ),
        surface("canvas", SurfaceBase::Blank(Extent::new(8, 6)?), vec![super::mask("only", None)?]),
    ])?;
    fixture.session.activate_input(WORKFLOW, "image", mask)?;
    fixture.settle_inputs()?;
    Ok(synced(&reply, "image")?.clone())
}

impl Fixture {
    fn edit(&mut self, command: CanvasEditCommand) -> Result<EditState, Box<dyn Error>> {
        self.session.edit(command)?;
        self.session.edit_state()
    }

    /// Move the pointer to a document point.
    fn point(&mut self, point: [f64; 2], held: ToolModifiers) -> Result<(), Box<dyn Error>> {
        let device = self.session.document.tools.navigation().viewport().device_at(point)?;
        self.session.input(Input::Tool(ToolInput::Pointer(PointerSample::mouse(device, held))))
    }

    fn button(&mut self, pressed: bool) -> Result<(), Box<dyn Error>> {
        let button = ToolInput::Button { button: ToolButton::Primary, pressed, modifiers: NONE };
        self.session.input(Input::Tool(button))
    }

    /// Drag the primary button through document points.
    fn drag(&mut self, points: &[[f64; 2]]) -> Result<(), Box<dyn Error>> {
        self.point(points[0], NONE)?;
        self.button(true)?;
        for point in &points[1..] {
            self.point(*point, NONE)?;
        }
        self.button(false)
    }
}

fn availability(state: &EditState, tool: CanvasToolId) -> bool {
    state.tools.iter().any(|(id, available)| *id == tool && *available)
}

fn rejected(result: Result<EditState, Box<dyn Error>>) -> Option<&'static str> {
    match result.err()?.downcast_ref::<EditRejection>()? {
        EditRejection::Unavailable(message) => Some(message),
        EditRejection::Unsupported => Some("unsupported"),
    }
}

#[test]
fn every_tool_is_offered_and_mask_tools_need_an_editable_target() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    {
        let empty = fixture.session.edit_state()?;
        assert_eq!((empty.document, empty.tool), (None, CanvasToolId::PanZoom));
        assert!(rejected(fixture.edit(CanvasEditCommand::Tool(CanvasToolId::Brush))).is_some());
        let image = show(&mut fixture, Some("second"))?;
        let state = fixture.session.edit_state()?;
        assert_eq!(state.document, Some(image.document));
        assert_eq!(state.active_mask, Some(layer(&image, "second")?));
        assert_eq!(state.tools.map(|(id, _)| id), CanvasToolId::ALL);
        for tool in CanvasToolId::ALL {
            let expected = !matches!(
                tool,
                CanvasToolId::Move | CanvasToolId::Transform | CanvasToolId::SharedEdges
            );
            assert_eq!(availability(&state, tool), expected, "{tool:?}");
        }
        assert_eq!(state.history, [false, false]);
        assert_eq!(
            rejected(fixture.edit(CanvasEditCommand::Nudge { dx: 1, dy: 0 })),
            Some("unsupported")
        );
        assert_eq!(
            rejected(fixture.edit(CanvasEditCommand::Tool(CanvasToolId::Move))),
            Some("unsupported")
        );
        let chosen = fixture.edit(CanvasEditCommand::Tool(CanvasToolId::MaskEllipse))?;
        assert_eq!(chosen.tool, CanvasToolId::MaskEllipse);
        assert_eq!(fixture.session.editing_cursor(), Some(CursorIntent::Forbidden));
        // Without a target mask, mask tools are unavailable and the chosen one falls back.
        fixture.session.activate_input(WORKFLOW, "image", None)?;
        fixture.settle_inputs()?;
        let state = fixture.session.edit_state()?;
        assert_eq!((state.active_mask, state.tool), (None, CanvasToolId::PanZoom));
        assert!(!availability(&state, CanvasToolId::Brush));
        assert!(availability(&state, CanvasToolId::SelectLasso));
        assert!(rejected(fixture.edit(CanvasEditCommand::Tool(CanvasToolId::Eraser))).is_some());
        assert_eq!(fixture.session.editing_cursor(), None, "navigation shows its own hands");
    }
    fixture.finish().map(drop)
}

#[test]
fn brush_options_map_whole_percentages_and_share_their_size() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    {
        show(&mut fixture, Some("first"))?;
        let options = CanvasBrushOptions::new(10, 50, 25)?;
        let state =
            fixture.edit(CanvasEditCommand::Options { tool: CanvasBrushTool::Brush, options })?;
        assert_eq!(state.brush, BrushOptions::new(10, 0.5, 0.25)?);
        assert_eq!(state.eraser, BrushOptions::new(10, 1.0, 1.0)?, "only the size is shared");
        fixture.edit(CanvasEditCommand::Tool(CanvasToolId::Eraser))?;
        fixture.point([30.0, 20.0], NONE)?;
        let wheel = ToolInput::Wheel { steps: 2.0, phase: ScrollPhase::Unphased };
        fixture.session.input(Input::Tool(wheel))?;
        let state = fixture.session.edit_state()?;
        assert_eq!([state.brush.size(), state.eraser.size()], [20, 20]);
        assert!(matches!(fixture.session.editing_cursor(), Some(CursorIntent::BrushOutline(_))));
        // A newly shown document receives the same tool and options.
        fixture.session.activate_input(WORKFLOW, "canvas", Some("only"))?;
        fixture.settle_inputs()?;
        let options =
            fixture.session.document.tools.options(sugarsubstitute_canvas::ToolId::new(2)?);
        let shown = sugarsubstitute_canvas::ToolOptions::Brush(BrushOptions::new(20, 0.5, 0.25)?);
        assert_eq!(options, Some(shown));
        assert_eq!(fixture.session.edit_state()?.tool, CanvasToolId::Eraser);
    }
    fixture.finish().map(drop)
}

#[test]
fn strokes_render_staged_steps_and_commit_one_undoable_step() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    {
        let image = show(&mut fixture, Some("first"))?;
        let first = layer(&image, "first")?;
        fixture.edit(CanvasEditCommand::Tool(CanvasToolId::Brush))?;
        let before = fixture.snapshot(image.document)?;
        fixture.point([10.0, 10.0], NONE)?;
        fixture.button(true)?;
        fixture.point([40.0, 10.0], NONE)?;
        fixture.settle_inputs()?;
        let staged = fixture.session.document.staged.as_ref().ok_or("No staged step")?;
        let key = staged.key;
        assert_eq!((key.0, key.1), (before.identity(), before.revision()));
        assert_ne!(key.2, 0, "a staged step is distinct content");
        assert_eq!(coverage(&staged.snapshot, first)?.value(25, 10), FULL_COVERAGE);
        assert!(
            fixture
                .session
                .document
                .presented_documents
                .iter()
                .flatten()
                .any(|(shown, _)| *shown == key),
            "frames render the staged step"
        );
        assert_eq!(fixture.snapshot(image.document)?.revision(), before.revision());
        fixture.button(false)?;
        fixture.settle_inputs()?;
        let after = fixture.snapshot(image.document)?;
        assert_eq!(coverage(&after, first)?.value(25, 10), FULL_COVERAGE);
        assert!(fixture.session.document.staged.is_none());
        let published = (after.identity(), after.revision(), 0);
        assert!(
            fixture
                .session
                .document
                .presented_documents
                .iter()
                .flatten()
                .any(|(shown, _)| *shown == published)
        );
        assert_eq!(fixture.session.edit_state()?.history, [true, false]);
        let undone = fixture.edit(CanvasEditCommand::History(CanvasHistoryAction::Undo))?;
        assert_eq!(undone.history, [false, true]);
        assert_eq!(coverage(&fixture.snapshot(image.document)?, first)?.uniform(), Some(0));
        fixture.edit(CanvasEditCommand::History(CanvasHistoryAction::Redo))?;
        assert_eq!(
            coverage(&fixture.snapshot(image.document)?, first)?.value(25, 10),
            FULL_COVERAGE
        );
        // The coverage panel blocks canvas input.
        fixture.edit(CanvasEditCommand::Mode(CanvasEditMode::Coverage))?;
        assert!(fixture.session.edit_state()?.coverage_mode);
        let revision = fixture.snapshot(image.document)?.revision();
        fixture.drag(&[[10.0, 40.0], [50.0, 40.0]])?;
        fixture.settle_inputs()?;
        assert_eq!(fixture.snapshot(image.document)?.revision(), revision);
        assert_eq!(fixture.session.editing_cursor(), Some(CursorIntent::Arrow));
        fixture.edit(CanvasEditCommand::Mode(CanvasEditMode::None))?;
    }
    fixture.finish().map(drop)
}

#[test]
fn selections_run_as_jobs_draw_ants_and_report_bounds_that_follow_the_camera()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    {
        let image = show(&mut fixture, Some("first"))?;
        let first = layer(&image, "first")?;
        // A stroke before any selection paints across the later selection's edge.
        fixture.edit(CanvasEditCommand::Tool(CanvasToolId::Brush))?;
        fixture.drag(&[[4.0, 14.0], [40.0, 14.0]])?;
        fixture.settle_inputs()?;
        let painted = coverage(&fixture.snapshot(image.document)?, first)?;
        assert_eq!([painted.value(30, 14), painted.value(12, 14)], [FULL_COVERAGE; 2]);
        fixture.edit(CanvasEditCommand::Tool(CanvasToolId::SelectRectangle))?;
        fixture.drag(&[[8.0, 8.0], [24.0, 20.0]])?;
        assert!(fixture.session.document.tools.overlay().is_none(), "the outline ends on release");
        fixture.settle_inputs()?;
        let selected = fixture.snapshot(image.document)?;
        // The drag's right edge may snap to the painted stroke's bounds.
        let bounds = selected.selection().and_then(|selection| selection.bounds());
        let bounds = bounds.ok_or("Nothing is selected")?;
        assert_eq!([bounds.x(), bounds.y(), bounds.height()], [8, 8, 12]);
        let state = fixture.session.edit_state()?;
        let viewport = fixture.session.document.tools.navigation().viewport();
        let [left, top] = viewport.device_at([8.0, 8.0])?;
        let [right, bottom] =
            viewport.device_at([f64::from(bounds.right()), f64::from(bounds.bottom())])?;
        assert_eq!(state.selection, Some([left, top, right - left, bottom - top]));
        let overlay = fixture.session.shown_overlay().ok_or("No ants")?;
        assert!(overlay.animated(), "the traced selection animates");
        let generation = fixture.session.shown_overlay_generation();
        assert_ne!(generation, 0);
        let now = fixture.session.navigation_time();
        assert_eq!(fixture.session.editing_deadline(now), None, "hidden ants do not step");
        fixture.session.drawable = true;
        let next = fixture.session.editing_deadline(now).ok_or("No ant step is scheduled")?;
        fixture.session.drawable = false;
        assert!(next > now && next <= now + sugarsubstitute_canvas::MarchingAnts::STEP);
        let step = u64::try_from(sugarsubstitute_canvas::MarchingAnts::STEP.as_nanos())?;
        fixture.clock.0.fetch_add(step, Ordering::SeqCst);
        fixture.settle_inputs()?;
        assert_ne!(fixture.session.shown_overlay_generation(), generation, "the ants stepped");

        // Clear removes the selected area from the mask through a job; the selection stays.
        fixture.edit(CanvasEditCommand::Selection(CanvasSelectionAction::Clear))?;
        fixture.settle_inputs()?;
        let cleared = fixture.snapshot(image.document)?;
        assert_eq!(coverage(&cleared, first)?.value(12, 14), 0);
        assert_eq!(coverage(&cleared, first)?.value(30, 14), FULL_COVERAGE);
        assert!(cleared.selection().is_some());

        // Deselect drops the selection, its bounds and its ants.
        let state = fixture.edit(CanvasEditCommand::Selection(CanvasSelectionAction::Deselect))?;
        assert_eq!(state.selection, None);
        fixture.settle_inputs()?;
        assert!(fixture.session.shown_overlay().is_none());
        assert_eq!(fixture.session.shown_overlay_generation(), 0);
    }
    fixture.finish().map(drop)
}

#[test]
fn modifications_preview_live_and_enter_applies_while_escape_cancels() -> Result<(), Box<dyn Error>>
{
    let mut fixture = Fixture::new()?;
    {
        let image = show(&mut fixture, Some("first"))?;
        fixture.edit(CanvasEditCommand::Tool(CanvasToolId::SelectRectangle))?;
        fixture.drag(&[[20.0, 16.0], [30.0, 26.0]])?;
        fixture.settle_inputs()?;
        let modify = |phase| {
            CanvasEditCommand::Modify(CanvasCoverageModify {
                target: CanvasModifyTarget::Selection,
                operation: CanvasModifyOperation::Expand,
                pixels: 4,
                phase,
            })
        };
        let previewing = fixture.edit(modify(CanvasModifyPhase::Preview))?;
        assert!(previewing.modifying);
        assert_eq!(previewing.tool, CanvasToolId::SelectRectangle, "the chosen tool returns");
        fixture.settle_inputs()?;
        let staged = fixture.session.document.staged.as_ref().ok_or("No modify preview")?;
        let preview = staged.snapshot.selection().and_then(|selection| selection.bounds());
        assert_eq!(preview, Some(sugarsubstitute_image_ops::Region::new(16, 12, 18, 18)?));
        let revision = fixture.snapshot(image.document)?.revision();
        assert!(
            rejected(fixture.edit(CanvasEditCommand::History(CanvasHistoryAction::Undo))).is_some()
        );
        let applied = fixture.edit(CanvasEditCommand::Commit)?;
        fixture.settle_inputs()?;
        assert!(!applied.modifying && !fixture.session.edit_state()?.modifying);
        let expanded = fixture.snapshot(image.document)?;
        assert_eq!(expanded.revision(), revision + 1, "the modification is one step");
        assert_eq!(expanded.selection().and_then(|selection| selection.bounds()), preview);

        fixture.edit(modify(CanvasModifyPhase::Preview))?;
        fixture.settle_inputs()?;
        let cancelled = fixture.edit(CanvasEditCommand::Cancel)?;
        fixture.settle_inputs()?;
        assert!(!cancelled.modifying);
        assert_eq!(fixture.snapshot(image.document)?.revision(), expanded.revision());
        assert!(fixture.session.document.staged.is_none());
    }
    fixture.finish().map(drop)
}
