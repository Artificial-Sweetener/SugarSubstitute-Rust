// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Deselect, clear, and live-previewed coverage modification with its navigation hand-off.
use super::*;
use crate::{CoverageShapeTool, MaskAppearance, ShapeKind, ShapeTarget, ToolOverlay};
use sugarsubstitute_contracts::execution::{JobContext, MemoryLease};
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
        Ok(self.tools.complete_deferred(prepared, &mut self.document, Duration::ZERO)?.applied())
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
        let late = canvas.tools.complete_deferred(late, &mut canvas.document, Duration::ZERO)?;
        assert!(matches!(late, DeferredCompletion::Unchanged), "{late:?}");
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
        canvas.tools.temporary_navigation(true, Some(&mut canvas.document), Duration::ZERO)?;
        canvas.tools.temporary_navigation(false, Some(&mut canvas.document), Duration::ZERO)?;
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

/// A canvas document lease in tests: a fixed reservation whose final release needs no owner.
struct Unowned;
impl sugarsubstitute_contracts::execution::MemoryRelease for Unowned {
    fn release(&self, _: u64) {}
}

/// Edge of the large document the reservation regression edits.
const LARGE_EDGE: u32 = 4096;
/// The canvas's reservation for one coverage job over the large document: six bytes per
/// document pixel and 1 MiB.
const LARGE_JOB: u64 = LARGE_EDGE as u64 * LARGE_EDGE as u64 * 6 + (1 << 20);

/// A large document whose soft mask reaches every edge, charged to the document's own lease as
/// the canvas charges input documents, with the mask targeted.
fn spanning(memory: &MemoryLease) -> Result<Fixture, CanvasError> {
    let extent = Extent::new(LARGE_EDGE, LARGE_EDGE)?;
    let mut tools = ToolManager::new(Navigation::new(crate::Viewport::new(extent)?));
    let tool = ToolId::new(2)?;
    tools.register(
        tool,
        CoverageShapeTool::new(ShapeTarget::Mask, ShapeKind::Rectangle),
        memory,
    )?;
    let mut document = Document::new(1, extent, memory)?;
    let mut mask = Coverage::new(extent, 0, memory)?;
    mask.update(extent.region(), memory, |x, y, _| ((x * 37 + y * 11) % 65_536) as u16)?;
    let appearance = MaskAppearance::new([0, 128, 255], 0.5)?;
    let layer =
        Layer::new(LayerId::new(MASK)?, LayerContent::Coverage { coverage: mask, appearance })?;
    document.apply(DocumentEdit::Insert { index: 0, layer })?;
    tools.select(tool, Some(&document), Duration::ZERO)?;
    tools.set_target(Some(LayerId::new(MASK)?));
    Ok(Fixture { tools, document, tool })
}

#[test]
fn feather_and_expand_of_a_mask_spanning_a_large_document_resolve_within_one_job_reservation()
-> harness::TestResult {
    harness::run(LARGE_JOB, |context| {
        let run = || -> Result<(), CanvasError> {
            let memory = MemoryLease::new(1 << 30, Box::new(Unowned));
            let mut canvas = spanning(&memory)?;
            for operation in [CoverageOperation::Feather, CoverageOperation::Expand] {
                let revision = canvas.document.snapshot().revision();
                canvas.modify(ModifyTarget::Mask, operation, 12, ModifyPhase::Preview)?;
                assert!(canvas.settle(context)?, "{operation:?} preview");
                canvas.modify(ModifyTarget::Mask, operation, 12, ModifyPhase::Apply)?;
                assert!(canvas.tools.take_deferred().is_none());
                assert_eq!(canvas.tools.modifying(), None, "{operation:?} stayed open");
                assert_eq!(canvas.document.snapshot().revision(), revision + 1);
                // Nothing of the job's working storage stays charged to it.
                assert_eq!(context.memory().claimed_bytes(), 0);
                assert!(canvas.document.undo()?);
                assert!(canvas.document.redo()?);
            }
            Ok(())
        };
        run().map_err(|cause| ImageError::Source(Box::new(cause)))
    })
}

#[test]
fn work_that_ends_without_a_result_cancels_its_modification() -> harness::TestResult {
    run(|context| {
        let mut canvas = fixture(context)?;
        let revision = canvas.document.snapshot().revision();
        canvas.modify(ModifyTarget::Mask, CoverageOperation::Feather, 8, ModifyPhase::Preview)?;
        let failed = canvas.tools.take_deferred().ok_or(CanvasError::Invalid("No preview"))?;
        canvas.modify(ModifyTarget::Mask, CoverageOperation::Feather, 9, ModifyPhase::Preview)?;
        let latest = canvas.tools.take_deferred().ok_or(CanvasError::Invalid("No preview"))?;
        // Earlier, superseded work failing leaves the open modification alone.
        assert!(!canvas.tools.deferred_failed(
            failed.request(),
            Some(&canvas.document),
            Duration::ZERO
        )?);
        assert_eq!(canvas.tools.modifying(), Some(ModifyTarget::Mask));
        // The latest request failing ends it as Cancel would, and the document is unchanged.
        assert!(canvas.tools.deferred_failed(
            latest.request(),
            Some(&canvas.document),
            Duration::ZERO
        )?);
        assert_eq!((canvas.tools.modifying(), canvas.tools.effective()), (None, canvas.tool));
        assert!(canvas.tools.preview_snapshot().is_none());
        assert_eq!(canvas.document.snapshot().revision(), revision);
        assert!(canvas.tools.deselect(&mut canvas.document)?);
        assert!(canvas.document.undo()?);

        // Work the document keeps outdating is abandoned, and so ends its modification too.
        canvas.modify(ModifyTarget::Mask, CoverageOperation::Expand, 4, ModifyPhase::Preview)?;
        let mut work = canvas.tools.take_deferred().ok_or(CanvasError::Invalid("No preview"))?;
        let selections = [
            Some(
                Path::rectangle([0.0, 0.0, 10.0, 10.0])?
                    .rasterize(canvas.document.snapshot().extent(), context)?,
            ),
            None,
        ];
        for attempt in 0.. {
            let prepared = work.run(context)?;
            canvas.document.apply(DocumentEdit::Select(selections[attempt % 2].clone()))?;
            match canvas.tools.complete_deferred(prepared, &mut canvas.document, Duration::ZERO)? {
                DeferredCompletion::Rerun(next) => work = next,
                DeferredCompletion::Abandoned => break,
                other => {
                    return Err(CanvasError::Invalid(
                        if matches!(other, DeferredCompletion::Applied) {
                            "Stale work applied"
                        } else {
                            "Stale work was dropped"
                        },
                    ));
                }
            }
        }
        assert_eq!((canvas.tools.modifying(), canvas.tools.effective()), (None, canvas.tool));
        assert!(canvas.tools.preview_snapshot().is_none());
        Ok(())
    })
}
