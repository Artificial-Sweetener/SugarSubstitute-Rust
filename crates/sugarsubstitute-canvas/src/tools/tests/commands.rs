// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Toolbar camera commands preserve the selected tool and its transient edit/capture ownership.

use super::*;

#[test]
fn freezing_retires_live_edits_and_restores_persistent_tool_selection() -> harness::TestResult {
    execute(|memory| {
        let mut tools = manager()?;
        let id = ToolId::new(2)?;
        tools.register(id, EditingTool::default(), memory)?;
        let mut document = Document::new(1, Extent::new(1000, 1000)?, memory)?;
        tools.select(id, Some(&document), Duration::ZERO)?;
        let accepted = tools.navigation().viewport();
        tools.input(
            ToolInput::Button {
                button: ToolButton::Primary,
                pressed: true,
                modifiers: ToolModifiers::NONE,
            },
            Some(&mut document),
            Duration::ZERO,
        )?;
        let foreign = crate::Viewport::new(Extent::new(12, 13)?)?;
        assert!(tools.freeze_at(foreign, Some(&document), Duration::ZERO).is_err());
        assert!(tools.captured());
        assert!(tools.preview().is_some());
        tools.freeze_at(accepted, Some(&document), Duration::ZERO)?;
        assert!(!tools.captured());
        assert!(tools.preview().is_none());
        assert_eq!(document.snapshot().revision(), 1);
        tools.temporary_navigation(true, Some(&document), Duration::ZERO)?;
        tools.navigate(NavigationCommand::OneToOne, Duration::ZERO)?;
        tools.freeze_at(accepted, Some(&document), Duration::from_millis(40))?;
        assert_eq!(tools.selected(), id);
        assert_eq!(tools.effective(), id);
        assert_eq!(tools.navigation().viewport(), accepted);
        assert!(!tools.navigation().interacting());
        tools.advance(Duration::from_secs(5))?;
        assert_eq!(tools.navigation().viewport(), accepted);
        Ok(())
    })
}

#[test]
fn camera_commands_preserve_live_edits_and_temporary_tool_selection() -> harness::TestResult {
    execute(|memory| {
        let mut tools = manager()?;
        let id = ToolId::new(2)?;
        tools.register(id, EditingTool::default(), memory)?;
        let mut document = Document::new(1, Extent::new(1000, 1000)?, memory)?;
        tools.select(id, Some(&document), Duration::ZERO)?;
        tools.input(
            ToolInput::Button {
                button: ToolButton::Primary,
                pressed: true,
                modifiers: ToolModifiers::NONE,
            },
            Some(&mut document),
            Duration::ZERO,
        )?;
        let before = tools.navigation().viewport();
        assert!(!tools.cancel_navigation(before, ZoomMode::Fit, Duration::ZERO)?);
        assert_eq!(
            tools.navigate(NavigationCommand::OneToOne, Duration::ZERO)?,
            NavigationCommandOutcome::Denied
        );
        assert!(tools.captured());
        assert!(tools.preview().is_some());
        assert_eq!(tools.navigation().viewport(), before);
        assert_eq!(tools.selected(), id);
        assert_eq!(tools.effective(), id);
        tools.input(
            ToolInput::Button {
                button: ToolButton::Primary,
                pressed: false,
                modifiers: ToolModifiers::NONE,
            },
            Some(&mut document),
            Duration::ZERO,
        )?;
        tools.cancel(ToolCancelReason::FocusLost, Some(&document), Duration::ZERO)?;
        tools.temporary_navigation(true, Some(&document), Duration::ZERO)?;
        assert!(matches!(
            tools.navigate(NavigationCommand::OneToOne, Duration::ZERO)?,
            NavigationCommandOutcome::Applied { .. }
        ));
        assert_eq!(tools.selected(), id);
        assert_eq!(tools.effective(), ToolId::PAN_ZOOM);
        tools.temporary_navigation(false, Some(&document), Duration::from_millis(80))?;
        assert_eq!(tools.effective(), id);
        assert_eq!(document.snapshot().revision(), 2);
        Ok(())
    })
}

#[test]
fn framing_preserves_live_edits_and_applies_immediately_once_navigation_is_free()
-> harness::TestResult {
    execute(|memory| {
        let mut tools = manager()?;
        let id = ToolId::new(2)?;
        tools.register(id, EditingTool::default(), memory)?;
        let mut document = Document::new(1, Extent::new(1000, 1000)?, memory)?;
        tools.select(id, Some(&document), Duration::ZERO)?;
        let frame = NavigationCommand::Frame { center: [0.25, 0.75], span: [0.0005; 2] };
        tools.input(
            ToolInput::Button {
                button: ToolButton::Primary,
                pressed: true,
                modifiers: ToolModifiers::NONE,
            },
            Some(&mut document),
            Duration::ZERO,
        )?;
        let before = tools.navigation().viewport();
        assert_eq!(tools.navigate(frame, Duration::ZERO)?, NavigationCommandOutcome::Denied);
        assert!(tools.preview().is_some());
        assert_eq!(tools.navigation().viewport(), before);
        tools.input(
            ToolInput::Button {
                button: ToolButton::Primary,
                pressed: false,
                modifiers: ToolModifiers::NONE,
            },
            Some(&mut document),
            Duration::ZERO,
        )?;
        let NavigationCommandOutcome::Applied { target } = tools.navigate(frame, Duration::ZERO)?
        else {
            return Err(crate::CanvasError::Invalid("Frame was not admitted"));
        };
        assert_eq!(tools.navigation().viewport(), target);
        assert!(!tools.navigation().interacting());
        assert_eq!(tools.navigation().zoom_mode(), ZoomMode::Custom);
        assert_eq!(tools.selected(), id);
        assert_eq!(tools.effective(), id);
        let center = target.document_at([50.0, 50.0])?;
        assert!((center[0] - 250.0).abs() < 1e-9 && (center[1] - 750.0).abs() < 1e-9);
        Ok(())
    })
}
