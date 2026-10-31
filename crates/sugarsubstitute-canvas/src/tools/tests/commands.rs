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
            ToolInput::Button { button: ToolButton::Primary, pressed: true },
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
            ToolInput::Button { button: ToolButton::Primary, pressed: true },
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
            ToolInput::Button { button: ToolButton::Primary, pressed: false },
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
