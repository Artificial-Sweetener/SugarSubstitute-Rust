// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Deselect, clear, and live-previewed coverage modification with its navigation hand-off.
use super::*;
use crate::{CoverageShapeTool, MaskAppearance, ShapeKind, ShapeTarget, ToolOverlay};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_image_ops::{Coverage, Path, Region};

const MASK: u64 = 7;

struct Fixture {
    tools: ToolManager,
    document: Document,
    tool: ToolId,
}

fn fixture(context: &JobContext) -> Result<Fixture, CanvasError> {
    let extent = Extent::new(200, 200)?;
    let mut tools = ToolManager::new(Navigation::new(crate::Viewport::new(extent)?));
    let tool = ToolId::new(2)?;
    tools.register(
        tool,
        CoverageShapeTool::new(ShapeTarget::Selection, ShapeKind::Rectangle),
        context.memory(),
    )?;
    let mut document = Document::new(1, extent, context.memory())?;
    let selection = Path::rectangle([40.0, 40.0, 80.0, 80.0])?.rasterize(extent, context)?;
    let mask = Path::rectangle([0.0, 0.0, 100.0, 200.0])?.rasterize(extent, context)?;
    let appearance = MaskAppearance::new([0, 128, 255], 0.5)?;
    let layer =
        Layer::new(LayerId::new(MASK)?, LayerContent::Coverage { coverage: mask, appearance })?;
    document.apply(DocumentEdit::Insert { index: 0, layer })?;
    document.apply(DocumentEdit::Select(Some(selection)))?;
    tools.select(tool, Some(&document), Duration::ZERO)?;
    tools.set_target(Some(LayerId::new(MASK)?));
    Ok(Fixture { tools, document, tool })
}

impl Fixture {
    fn settle(&mut self, context: &JobContext) -> Result<bool, CanvasError> {
        let work = self.tools.take_deferred().ok_or(CanvasError::Invalid("No deferred work"))?;
        let prepared = work.run(context)?;
        self.tools.complete_deferred(prepared, &mut self.document, Duration::ZERO)
    }
    fn modify(
        &mut self,
        target: ModifyTarget,
        operation: CoverageOperation,
        pixels: u32,
        phase: ModifyPhase,
    ) -> Result<(), CanvasError> {
        let modification = CoverageModification::new(operation, pixels)?;
        self.tools.modify(target, modification, phase, &mut self.document, Duration::ZERO)
    }
    fn selection(&self) -> Option<Region> {
        self.document.snapshot().selection().and_then(Coverage::bounds)
    }
    fn mask(&self) -> Option<Region> {
        self.document.snapshot().layers().iter().find_map(|layer| match layer.content() {
            LayerContent::Coverage { coverage, .. } => coverage.bounds(),
            _ => None,
        })
    }
}

fn run(
    test: impl FnOnce(&JobContext) -> Result<(), CanvasError> + Send + 'static,
) -> harness::TestResult {
    harness::run(32 * 1024 * 1024, |context| {
        test(context).map_err(|cause| ImageError::Source(Box::new(cause)))
    })
}

#[test]
fn deselect_and_clear_are_single_undo_steps() -> harness::TestResult {
    run(|context| {
        let mut canvas = fixture(context)?;
        assert!(canvas.tools.clear_selection(&canvas.document)?);
        assert!(canvas.settle(context)?);
        let snapshot = canvas.document.snapshot();
        let mask = snapshot.layers().iter().find_map(|layer| match layer.content() {
            LayerContent::Coverage { coverage, .. } => Some(coverage.clone()),
            _ => None,
        });
        assert_eq!(
            mask.as_ref().map(|mask| [mask.value(50, 50), mask.value(30, 50)]),
            Some([0, 65535])
        );
        assert_eq!(canvas.selection(), Some(Region::new(40, 40, 40, 40)?));
        assert!(canvas.document.undo()?);
        assert_eq!(canvas.mask(), Some(Region::new(0, 0, 100, 200)?));
        assert!(canvas.tools.deselect(&mut canvas.document)?);
        assert_eq!(canvas.selection(), None);
        assert!(!canvas.tools.clear_selection(&canvas.document)?);
        assert!(canvas.document.undo()?);
        assert_eq!(canvas.selection(), Some(Region::new(40, 40, 40, 40)?));
        canvas.tools.set_target(None);
        assert!(!canvas.tools.clear_selection(&canvas.document)?);
        Ok(())
    })
}

#[test]
fn previews_hand_input_to_navigation_and_apply_the_latest_once() -> harness::TestResult {
    run(|context| {
        let mut canvas = fixture(context)?;
        let revision = canvas.document.snapshot().revision();
        canvas.modify(
            ModifyTarget::Selection,
            CoverageOperation::Expand,
            4,
            ModifyPhase::Preview,
        )?;
        assert_eq!(canvas.tools.modifying(), Some(ModifyTarget::Selection));
        assert_eq!(
            (canvas.tools.effective(), canvas.tools.selected()),
            (ToolId::PAN_ZOOM, canvas.tool)
        );
        let first = canvas.tools.take_deferred().ok_or(CanvasError::Invalid("No preview"))?;
        canvas.modify(
            ModifyTarget::Selection,
            CoverageOperation::Expand,
            10,
            ModifyPhase::Preview,
        )?;
        // The superseded preview finishes late and is ignored.
        let late = first.run(context)?;
        assert!(!canvas.tools.complete_deferred(late, &mut canvas.document, Duration::ZERO)?);
        assert!(canvas.settle(context)?);
        let preview = canvas.tools.preview_snapshot().ok_or(CanvasError::Invalid("No preview"))?;
        assert_eq!(
            preview.selection().and_then(Coverage::bounds),
            Some(Region::new(30, 30, 60, 60)?)
        );
        assert_eq!(canvas.document.snapshot().revision(), revision);
        // Navigation stays available while previewing, and tool choices wait.
        assert!(matches!(
            canvas.tools.navigate(crate::NavigationCommand::Zoom(2.0), Duration::ZERO)?,
            crate::NavigationCommandOutcome::Applied { .. }
                | crate::NavigationCommandOutcome::NoContent
        ));
        canvas.tools.select(ToolId::PAN_ZOOM, Some(&canvas.document), Duration::ZERO)?;
        canvas.tools.select(canvas.tool, Some(&canvas.document), Duration::ZERO)?;
        assert_eq!(canvas.tools.effective(), ToolId::PAN_ZOOM);
        assert!(canvas.tools.deselect(&mut canvas.document).is_err());
        // Losing focus keeps the preview.
        canvas.tools.cancel(ToolCancelReason::FocusLost, Some(&canvas.document), Duration::ZERO)?;
        assert!(canvas.tools.preview_snapshot().is_some());
        // Applying the staged latest preview publishes it without recomputing.
        canvas.modify(
            ModifyTarget::Selection,
            CoverageOperation::Expand,
            10,
            ModifyPhase::Apply,
        )?;
        assert!(canvas.tools.take_deferred().is_none());
        assert_eq!(canvas.selection(), Some(Region::new(30, 30, 60, 60)?));
        assert_eq!(canvas.document.snapshot().revision(), revision + 1);
        assert_eq!((canvas.tools.modifying(), canvas.tools.effective()), (None, canvas.tool));
        assert!(canvas.tools.preview_snapshot().is_none());
        assert!(canvas.document.undo()?);
        assert_eq!(canvas.selection(), Some(Region::new(40, 40, 40, 40)?));
        Ok(())
    })
}

#[test]
fn applying_an_unfinished_preview_computes_it_and_cancel_restores() -> harness::TestResult {
    run(|context| {
        let mut canvas = fixture(context)?;
        canvas.modify(ModifyTarget::Mask, CoverageOperation::Contract, 20, ModifyPhase::Preview)?;
        canvas.modify(ModifyTarget::Mask, CoverageOperation::Contract, 30, ModifyPhase::Apply)?;
        assert!(
            canvas
                .modify(ModifyTarget::Selection, CoverageOperation::Expand, 1, ModifyPhase::Preview)
                .is_err()
        );
        assert_eq!(canvas.tools.modifying(), Some(ModifyTarget::Mask));
        assert!(canvas.settle(context)?);
        assert_eq!(canvas.mask(), Some(Region::new(30, 30, 40, 140)?));
        assert_eq!((canvas.tools.modifying(), canvas.tools.effective()), (None, canvas.tool));

        let revision = canvas.document.snapshot().revision();
        canvas.modify(
            ModifyTarget::Selection,
            CoverageOperation::Feather,
            8,
            ModifyPhase::Preview,
        )?;
        assert!(canvas.settle(context)?);
        // Space during the preview does not hand input back early.
        canvas.tools.temporary_navigation(true, Some(&canvas.document), Duration::ZERO)?;
        canvas.tools.temporary_navigation(false, Some(&canvas.document), Duration::ZERO)?;
        assert_eq!(canvas.tools.effective(), ToolId::PAN_ZOOM);
        canvas.modify(
            ModifyTarget::Selection,
            CoverageOperation::Feather,
            8,
            ModifyPhase::Cancel,
        )?;
        assert_eq!(canvas.document.snapshot().revision(), revision);
        assert!(canvas.tools.preview_snapshot().is_none());
        assert_eq!((canvas.tools.modifying(), canvas.tools.effective()), (None, canvas.tool));

        // Escape cancels an open preview as well; Apply without a preview applies directly.
        canvas.modify(
            ModifyTarget::Selection,
            CoverageOperation::Contract,
            25,
            ModifyPhase::Preview,
        )?;
        canvas.tools.cancel(ToolCancelReason::Requested, Some(&canvas.document), Duration::ZERO)?;
        assert_eq!((canvas.tools.modifying(), canvas.tools.effective()), (None, canvas.tool));
        assert!(canvas.tools.take_deferred().is_none());
        canvas.modify(
            ModifyTarget::Selection,
            CoverageOperation::Contract,
            25,
            ModifyPhase::Apply,
        )?;
        assert!(canvas.settle(context)?);
        assert_eq!(canvas.document.snapshot().selection(), None);
        assert!(
            canvas
                .modify(ModifyTarget::Selection, CoverageOperation::Expand, 4, ModifyPhase::Preview)
                .is_err()
        );
        assert!(CoverageModification::new(CoverageOperation::Expand, 0).is_err());
        assert!(CoverageModification::new(CoverageOperation::Expand, 1000).is_err());
        assert_eq!(canvas.tools.overlay(), None::<ToolOverlay<'_>>);
        Ok(())
    })
}
