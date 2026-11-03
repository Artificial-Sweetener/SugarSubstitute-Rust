// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Tool routing, temporary ownership and atomic gesture publication.
use super::*;
use crate::{Layer, LayerContent, LayerId, ZoomMode, tests::harness};
use sugarsubstitute_image_ops::Extent;
mod commands;
mod deferred;
mod modify;

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
                ToolInput::Button {
                    button: ToolButton::Primary,
                    pressed: true,
                    modifiers: ToolModifiers::NONE,
                },
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
            ToolInput::Button { button: ToolButton::Primary, pressed: true, .. } => {
                context.begin_edit()?;
                context.capture_pointer(true);
                context.stage(DocumentEdit::Insert {
                    index: 0,
                    layer: Layer::new(LayerId::new(1)?, LayerContent::Exposure { stops: 1.0 })?,
                })?;
            }
            ToolInput::Button { button: ToolButton::Primary, pressed: false, .. } => {
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
        let press = ToolInput::Button {
            button: ToolButton::Primary,
            pressed: true,
            modifiers: ToolModifiers::NONE,
        };
        let release = ToolInput::Button {
            button: ToolButton::Primary,
            pressed: false,
            modifiers: ToolModifiers::NONE,
        };
        tools.input(press, Some(&mut document), Duration::ZERO)?;
        assert_eq!(tools.preview().map(|p| p.layers().len()), Some(1));
        assert!(tools.captured());
        assert!(document.snapshot().layers().is_empty());
        tools.temporary_navigation(true, Some(&mut document), Duration::ZERO)?;
        assert!(tools.preview().is_none());
        assert!(!tools.captured());
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
            tools.input(
                ToolInput::Button { button, pressed: true, modifiers: ToolModifiers::NONE },
                None,
                now,
            )?;
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
            ToolInput::Button {
                button: ToolButton::Primary,
                pressed: false,
                modifiers: ToolModifiers::NONE,
            },
            None,
            now,
        )?;
        assert!(tools.navigation().captured());
        tools.input(
            ToolInput::Button {
                button: ToolButton::Middle,
                pressed: false,
                modifiers: ToolModifiers::NONE,
            },
            None,
            now,
        )?;
        assert_eq!(tools.cursor(None), CursorIntent::OpenHand);
        tools.cancel(ToolCancelReason::FocusLost, None, now)?;
        tools.input(
            ToolInput::Button {
                button: ToolButton::Primary,
                pressed: true,
                modifiers: ToolModifiers::NONE,
            },
            None,
            now,
        )?;
        tools.input(
            ToolInput::Button {
                button: ToolButton::Primary,
                pressed: false,
                modifiers: ToolModifiers::NONE,
            },
            None,
            now,
        )?;
        assert!(!tools.navigation().captured());
        Ok(())
    })
}

#[test]
fn tao_double_click_order_preserves_fit_roundtrip_after_release_and_pointer_motion()
-> harness::TestResult {
    execute(|_| {
        let mut tools = manager()?;
        let anchor = [27.0, 61.0];
        let moved = [43.0, 38.0];
        let initial_fit = tools.navigation().viewport();
        let anchored_document = initial_fit.document_at(anchor)?;
        let press = ToolInput::Button {
            button: ToolButton::Primary,
            pressed: true,
            modifiers: ToolModifiers::NONE,
        };
        let release = ToolInput::Button {
            button: ToolButton::Primary,
            pressed: false,
            modifiers: ToolModifiers::NONE,
        };
        let ordered_double_click = |tools: &mut ToolManager, first: Duration, second: Duration| {
            tools.input(press, None, first)?;
            tools.input(release, None, first)?;
            tools.input(press, None, second)?;
            tools.input(ToolInput::DoubleClick(anchor), None, second)?;
            tools.input(release, None, second)?;
            tools.input(
                ToolInput::Pointer(PointerSample::mouse(moved, ToolModifiers::NONE)),
                None,
                second,
            )
        };

        ordered_double_click(&mut tools, Duration::ZERO, Duration::from_millis(10))?;
        let actual_size = tools.navigation().target_viewport();
        assert_eq!(tools.navigation().zoom_mode(), ZoomMode::OneToOne);
        assert_eq!(actual_size.zoom(), 1.0);
        assert_eq!(actual_size.document_at(anchor)?, anchored_document);
        assert!(!tools.navigation().captured());
        assert!(tools.navigation().interacting());
        tools.advance(Duration::from_millis(91))?;
        assert_eq!(tools.navigation().viewport(), actual_size);
        assert_eq!(tools.navigation().zoom_mode(), ZoomMode::OneToOne);
        assert!(!tools.navigation().interacting());

        ordered_double_click(&mut tools, Duration::from_millis(100), Duration::from_millis(110))?;
        assert_eq!(tools.navigation().zoom_mode(), ZoomMode::Fit);
        assert_eq!(tools.navigation().target_viewport(), initial_fit);
        assert!(!tools.navigation().captured());
        assert!(tools.navigation().interacting());
        tools.advance(Duration::from_millis(191))?;
        assert_eq!(tools.navigation().viewport(), initial_fit);
        assert_eq!(tools.navigation().zoom_mode(), ZoomMode::Fit);
        assert!(!tools.navigation().interacting());
        Ok(())
    })
}

#[test]
fn drag_out_yields_to_any_other_effective_tool() -> harness::TestResult {
    execute(|memory| {
        let mut tools = manager()?;
        assert!(tools.allows_drag_out(), "fitted navigation exports instead of panning");
        let id = ToolId::new(2)?;
        tools.register(id, EditingTool::default(), memory)?;
        tools.select(id, None, Duration::ZERO)?;
        assert!(!tools.allows_drag_out(), "a selected editing tool takes priority");
        tools.select(ToolId::PAN_ZOOM, None, Duration::ZERO)?;
        assert!(tools.allows_drag_out());
        Ok(())
    })
}

#[test]
fn pressure_normalizes_device_ranges_and_defaults_to_full() {
    assert_eq!(Pressure::from_range(0, 1024).get(), 0.0);
    assert_eq!(Pressure::from_range(256, 1024).get(), 0.25);
    assert_eq!(Pressure::from_range(1024, 1024), Pressure::FULL);
    assert_eq!(Pressure::from_range(9000, 1024), Pressure::FULL, "out-of-range readings clamp");
    assert_eq!(Pressure::from_range(7, 0), Pressure::FULL, "a device without a range");
    let modifiers = ToolModifiers { shift: true, ctrl: false, alt: false };
    let mouse = PointerSample::mouse([3.0, 4.0], modifiers);
    assert_eq!(
        (mouse.kind, mouse.pressure, mouse.eraser, mouse.modifiers),
        (PointerKind::Mouse, Pressure::FULL, false, modifiers)
    );
    assert_eq!(mouse.at([5.0, 6.0]), PointerSample { position: [5.0, 6.0], ..mouse });
}

/// Records every observation so device facts can be compared after routing.
#[derive(Default)]
struct RecordingTool {
    seen: std::sync::Arc<std::sync::Mutex<Vec<ToolInput>>>,
}
impl CanvasTool for RecordingTool {
    fn input(&mut self, input: ToolInput, _: &mut ToolContext<'_>) -> Result<(), CanvasError> {
        self.seen.lock().map_err(|_| CanvasError::Invalid("Recording poisoned"))?.push(input);
        Ok(())
    }
    fn cancel(&mut self, _: ToolCancelReason) {}
    fn cursor(&self, _: &ToolView<'_>) -> CursorIntent {
        CursorIntent::Arrow
    }
}

#[test]
fn tools_receive_pen_samples_and_button_modifiers_unchanged() -> harness::TestResult {
    execute(|memory| {
        let mut tools = manager()?;
        let tool = RecordingTool::default();
        let seen = tool.seen.clone();
        let id = ToolId::new(2)?;
        tools.register(id, tool, memory)?;
        tools.select(id, None, Duration::ZERO)?;
        let modifiers = ToolModifiers { shift: false, ctrl: true, alt: true };
        let sample = PointerSample {
            position: [12.5, 40.0],
            kind: PointerKind::Pen,
            pressure: Pressure::from_range(600, 1024),
            eraser: true,
            modifiers,
        };
        let inputs = [
            ToolInput::Pointer(sample),
            ToolInput::Button { button: ToolButton::Primary, pressed: true, modifiers },
            ToolInput::Button { button: ToolButton::Primary, pressed: false, modifiers },
        ];
        for input in inputs {
            tools.input(input, None, Duration::ZERO)?;
        }
        assert_eq!(*seen.lock().map_err(|_| CanvasError::Invalid("Recording poisoned"))?, inputs);
        Ok(())
    })
}

#[test]
fn navigation_pans_identically_for_pen_and_mouse_samples() -> harness::TestResult {
    execute(|_| {
        type Pan = Result<(crate::Viewport, crate::Viewport), CanvasError>;
        let pan = |kind: PointerKind| -> Pan {
            let mut tools = manager()?;
            tools.input(ToolInput::OneToOne, None, Duration::ZERO)?;
            tools.advance(Duration::from_millis(100))?;
            let now = Duration::from_millis(100);
            let before = tools.navigation().target_viewport();
            let sample = |position| PointerSample {
                position,
                kind,
                pressure: Pressure::from_range(300, 1024),
                eraser: kind == PointerKind::Pen,
                modifiers: ToolModifiers::NONE,
            };
            let button = |pressed| ToolInput::Button {
                button: ToolButton::Primary,
                pressed,
                modifiers: ToolModifiers::NONE,
            };
            tools.input(ToolInput::Pointer(sample([50.0, 50.0])), None, now)?;
            tools.input(button(true), None, now)?;
            tools.input(ToolInput::Pointer(sample([30.0, 20.0])), None, now)?;
            tools.input(button(false), None, now)?;
            Ok((before, tools.navigation().target_viewport()))
        };
        let (before, mouse) = pan(PointerKind::Mouse)?;
        assert_ne!(before, mouse, "the drag panned");
        assert_eq!(pan(PointerKind::Pen)?, (before, mouse));
        Ok(())
    })
}
