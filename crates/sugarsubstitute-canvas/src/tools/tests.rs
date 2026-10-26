// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Tool routing, temporary ownership and atomic gesture publication.
use super::*;
use crate::{Layer, LayerContent, LayerId, tests::harness};
use sugarsubstitute_image_ops::Extent;

#[test]
fn rejected_memory_admission_does_not_register_a_tool_or_consume_its_identity()
-> harness::TestResult {
    execute(|memory| {
        let mut tools = manager()?;
        let blocker = memory.claim(memory.bytes()).map_err(ImageError::from)?;
        let id = ToolId::new(2)?;
        assert!(tools.register(id, EditingTool::default(), memory).is_err());
        assert!(!tools.contains(id));
        drop(blocker);
        tools.register(id, EditingTool::default(), memory)?;
        tools.select(id, None, Duration::ZERO)?;
        assert_eq!(tools.effective(), id);
        Ok(())
    })
}

#[test]
fn geometry_and_policy_changes_cancel_staged_edits_and_registration_is_bounded()
-> harness::TestResult {
    execute(|memory| {
        let mut tools = manager()?;
        for value in 2..18 {
            tools.register(ToolId::new(value)?, EditingTool::default(), memory)?;
        }
        assert!(tools.register(ToolId::new(18)?, EditingTool::default(), memory).is_err());
        let mut document = Document::new(1, Extent::new(1000, 1000)?, memory)?;
        tools.select(ToolId::new(2)?, Some(&document), Duration::ZERO)?;
        for change in 0..3 {
            tools.input(
                ToolInput::Button { button: ToolButton::Primary, pressed: true },
                Some(&mut document),
                Duration::ZERO,
            )?;
            match change {
                0 => tools.resize(Extent::new(200, 100)?, Some(&document), Duration::ZERO)?,
                1 => tools.set_policy(true, false, Some(&document), Duration::ZERO)?,
                _ => tools.set_content(None, Some(&document), Duration::ZERO)?,
            }
            assert!(tools.preview().is_none());
            assert_eq!(document.snapshot().revision(), 1);
        }
        Ok(())
    })
}

#[derive(Default)]
struct EditingTool {
    text: bool,
    reject: bool,
}
impl CanvasTool for EditingTool {
    fn activate(&mut self, _: &ToolView<'_>) -> Result<(), CanvasError> {
        if self.reject {
            return Err(CanvasError::Invalid("Rejected activation"));
        }
        Ok(())
    }
    fn input(
        &mut self,
        input: ToolInput,
        context: &mut ToolContext<'_>,
    ) -> Result<(), CanvasError> {
        match input {
            ToolInput::Button { button: ToolButton::Primary, pressed: true } => {
                context.begin_edit()?;
                context.stage(DocumentEdit::Insert {
                    index: 0,
                    layer: Layer::new(LayerId::new(1)?, LayerContent::Exposure { stops: 1.0 })?,
                })?;
            }
            ToolInput::Button { button: ToolButton::Primary, pressed: false } => {
                context.commit_edit()?;
            }
            _ => {}
        }
        Ok(())
    }
    fn cancel(&mut self, _: ToolCancelReason) {}
    fn cursor(&self, _: &ToolView<'_>) -> CursorIntent {
        CursorIntent::Arrow
    }
    fn captures_space(&self) -> bool {
        self.text
    }
}
fn manager() -> Result<ToolManager, CanvasError> {
    let mut navigation = Navigation::new(crate::Viewport::new(Extent::new(100, 100)?)?);
    navigation.set_content(Some(Extent::new(1000, 1000)?), Duration::ZERO)?;
    Ok(ToolManager::new(navigation))
}
fn execute(
    action: impl FnOnce(&MemoryLease) -> Result<(), CanvasError> + Send + 'static,
) -> harness::TestResult {
    harness::run(2 * 1024 * 1024, |context| {
        action(context.memory()).map_err(|cause| ImageError::Source(Box::new(cause)))
    })
}
#[test]
fn space_restores_latest_selection_and_respects_tool_owned_text_input() -> harness::TestResult {
    execute(|memory| {
        let mut tools = manager()?;
        let edit = ToolId::new(2)?;
        let text = ToolId::new(3)?;
        tools.register(edit, EditingTool::default(), memory)?;
        tools.register(text, EditingTool { text: true, reject: false }, memory)?;
        assert!(tools.register(edit, EditingTool::default(), memory).is_err());
        assert!(tools.register(ToolId::PAN_ZOOM, EditingTool::default(), memory).is_err());
        tools.select(edit, None, Duration::ZERO)?;
        tools.temporary_navigation(true, None, Duration::ZERO)?;
        assert_eq!(tools.selected(), edit);
        assert_eq!(tools.effective(), ToolId::PAN_ZOOM);
        tools.select(text, None, Duration::ZERO)?;
        assert_eq!(tools.effective(), ToolId::PAN_ZOOM);
        tools.temporary_navigation(true, None, Duration::ZERO)?;
        tools.temporary_navigation(false, None, Duration::ZERO)?;
        assert_eq!(tools.selected(), text);
        assert_eq!(tools.effective(), text);
        tools.temporary_navigation(true, None, Duration::ZERO)?;
        assert_eq!(tools.effective(), text);
        Ok(())
    })
}
#[test]
fn lifecycle_loss_discards_preview_while_completed_gesture_uses_one_history_entry()
-> harness::TestResult {
    execute(|memory| {
        let mut tools = manager()?;
        let edit = ToolId::new(2)?;
        tools.register(edit, EditingTool::default(), memory)?;
        let mut document = Document::new(1, Extent::new(1000, 1000)?, memory)?;
        tools.select(edit, Some(&document), Duration::ZERO)?;
        let press = ToolInput::Button { button: ToolButton::Primary, pressed: true };
        let release = ToolInput::Button { button: ToolButton::Primary, pressed: false };
        tools.input(press, Some(&mut document), Duration::ZERO)?;
        assert_eq!(tools.preview().map(|p| p.layers().len()), Some(1));
        assert!(document.snapshot().layers().is_empty());
        tools.temporary_navigation(true, Some(&document), Duration::ZERO)?;
        assert!(tools.preview().is_none());
        tools.cancel(ToolCancelReason::FocusLost, Some(&document), Duration::ZERO)?;
        assert_eq!(tools.effective(), edit);
        assert!(!document.undo()?);
        tools.input(press, Some(&mut document), Duration::ZERO)?;
        tools.input(release, Some(&mut document), Duration::ZERO)?;
        assert_eq!(document.snapshot().revision(), 2);
        assert!(tools.preview().is_none());
        assert!(document.undo()?);
        assert!(document.snapshot().layers().is_empty());
        assert!(!document.undo()?);
        tools.input(press, Some(&mut document), Duration::ZERO)?;
        tools.select(ToolId::PAN_ZOOM, Some(&document), Duration::ZERO)?;
        assert!(tools.preview().is_none());
        assert!(document.redo()?);
        Ok(())
    })
}
#[test]
fn rejected_activation_preserves_capture_and_all_pan_buttons_release_cleanly() -> harness::TestResult
{
    execute(|memory| {
        let mut tools = manager()?;
        tools.input(ToolInput::OneToOne, None, Duration::ZERO)?;
        tools.advance(Duration::from_millis(100))?;
        let now = Duration::from_millis(100);
        for button in [ToolButton::Primary, ToolButton::Middle] {
            tools.input(ToolInput::Button { button, pressed: true }, None, now)?;
        }
        assert_eq!(tools.cursor(None), CursorIntent::ClosedHand);
        let rejected = ToolId::new(2)?;
        tools.register(rejected, EditingTool { text: false, reject: true }, memory)?;
        assert!(tools.select(rejected, None, now).is_err());
        assert_eq!(tools.selected(), ToolId::PAN_ZOOM);
        assert!(tools.navigation().captured());
        tools.resize(Extent::new(100, 100)?, None, now)?;
        assert!(tools.navigation().captured());
        tools.input(
            ToolInput::Button { button: ToolButton::Primary, pressed: false },
            None,
            now,
        )?;
        assert!(tools.navigation().captured());
        tools.input(ToolInput::Button { button: ToolButton::Middle, pressed: false }, None, now)?;
        assert_eq!(tools.cursor(None), CursorIntent::OpenHand);
        tools.cancel(ToolCancelReason::FocusLost, None, now)?;
        tools.input(ToolInput::Button { button: ToolButton::Primary, pressed: true }, None, now)?;
        tools.input(
            ToolInput::Button { button: ToolButton::Primary, pressed: false },
            None,
            now,
        )?;
        assert!(!tools.navigation().captured());
        Ok(())
    })
}
