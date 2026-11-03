// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Tools see their edit target and hand large coverage work to an owned job.
use super::*;
use sugarsubstitute_image_ops::{Coverage, FULL_COVERAGE, Path};

/// Selects a rectangle through deferred work on press, previewing or committing it.
struct DeferringTool {
    disposition: Disposition,
    seen: Option<Option<LayerId>>,
}
impl CanvasTool for DeferringTool {
    fn input(
        &mut self,
        input: ToolInput,
        context: &mut ToolContext<'_>,
    ) -> Result<(), CanvasError> {
        if let ToolInput::Button { button: ToolButton::Primary, pressed: true, .. } = input {
            self.seen = Some(context.target());
            assert!(context.memory().is_ok());
            context.defer(self.disposition, |base, job| {
                let coverage =
                    Path::rectangle([0.0, 0.0, 10.0, 10.0])?.rasterize(base.extent(), job)?;
                Ok(DocumentEdit::Select(Some(coverage)))
            })?;
        }
        Ok(())
    }
    fn cancel(&mut self, _: ToolCancelReason) {}
    fn cursor(&self, _: &ToolView<'_>) -> CursorIntent {
        CursorIntent::Arrow
    }
}

fn press() -> ToolInput {
    ToolInput::Button { button: ToolButton::Primary, pressed: true, modifiers: ToolModifiers::NONE }
}

#[test]
fn deferred_work_commits_once_and_drops_stale_results() -> harness::TestResult {
    harness::run(8 * 1024 * 1024, |context| {
        let run = || -> Result<(), CanvasError> {
            let memory = context.memory();
            let mut tools = manager()?;
            let id = ToolId::new(2)?;
            tools.register(
                id,
                DeferringTool { disposition: Disposition::Commit, seen: None },
                memory,
            )?;
            let mut document = Document::new(1, Extent::new(64, 64)?, memory)?;
            tools.select(id, Some(&document), Duration::ZERO)?;
            tools.set_target(Some(LayerId::new(9)?));
            assert_eq!(tools.target(), Some(LayerId::new(9)?));
            tools.input(press(), Some(&mut document), Duration::ZERO)?;
            let deferred = tools.take_deferred().ok_or(CanvasError::Invalid("No deferred work"))?;
            assert!(tools.take_deferred().is_none());
            let prepared = deferred.run(context)?;
            assert!(tools.complete_deferred(prepared, &mut document, Duration::ZERO)?);
            let selection = document.snapshot().selection().cloned();
            assert_eq!(selection.as_ref().map(|s| s.value(5, 5)), Some(FULL_COVERAGE));
            assert!(document.undo()?);
            assert_eq!(document.snapshot().selection(), None);

            // Work computed from an older revision changes nothing.
            tools.input(press(), Some(&mut document), Duration::ZERO)?;
            let stale = tools
                .take_deferred()
                .ok_or(CanvasError::Invalid("No deferred work"))?
                .run(context)?;
            let empty = Coverage::new(Extent::new(64, 64)?, 0, memory)?;
            document.apply(DocumentEdit::Select(Some(empty)))?;
            let revision = document.snapshot().revision();
            assert!(!tools.complete_deferred(stale, &mut document, Duration::ZERO)?);
            assert_eq!(document.snapshot().revision(), revision);

            // Cancelling a gesture drops work not yet taken.
            tools.input(press(), Some(&mut document), Duration::ZERO)?;
            tools.cancel(ToolCancelReason::SelectionChanged, Some(&document), Duration::ZERO)?;
            assert!(tools.take_deferred().is_none());
            Ok(())
        };
        run().map_err(|cause| ImageError::Source(Box::new(cause)))
    })
}

#[test]
fn deferred_previews_stage_renderable_snapshots_without_publishing() -> harness::TestResult {
    harness::run(8 * 1024 * 1024, |context| {
        let run = || -> Result<(), CanvasError> {
            let memory = context.memory();
            let mut tools = manager()?;
            let id = ToolId::new(2)?;
            tools.register(
                id,
                DeferringTool { disposition: Disposition::Preview, seen: None },
                memory,
            )?;
            let mut document = Document::new(1, Extent::new(64, 64)?, memory)?;
            tools.select(id, Some(&document), Duration::ZERO)?;
            tools.input(press(), Some(&mut document), Duration::ZERO)?;
            let prepared = tools
                .take_deferred()
                .ok_or(CanvasError::Invalid("No deferred work"))?
                .run(context)?;
            assert!(tools.complete_deferred(prepared, &mut document, Duration::ZERO)?);
            assert_eq!(document.snapshot().revision(), 1);
            assert_eq!(document.snapshot().selection(), None);
            let preview = tools.preview_snapshot().ok_or(CanvasError::Invalid("No preview"))?;
            assert_eq!((preview.identity(), preview.revision(), preview.stage()), (1, 1, 1));
            assert!(preview.selection().is_some());
            assert_eq!(document.snapshot().stage(), 0);
            tools.cancel(ToolCancelReason::SelectionChanged, Some(&document), Duration::ZERO)?;
            assert!(tools.preview_snapshot().is_none());
            Ok(())
        };
        run().map_err(|cause| ImageError::Source(Box::new(cause)))
    })
}
