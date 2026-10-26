// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Interpret primary/middle-button state through the shared navigation owner.
use super::*;

#[derive(Default)]
pub(super) struct PanZoom {
    buttons: u8,
}
impl CanvasTool for PanZoom {
    fn input(
        &mut self,
        input: ToolInput,
        context: &mut ToolContext<'_>,
    ) -> Result<(), CanvasError> {
        let input = match input {
            ToolInput::Pointer(point) => NavigationInput::Pointer(point),
            ToolInput::Button { button, pressed } => {
                let bit = match button {
                    ToolButton::Primary => 1,
                    ToolButton::Middle => 2,
                    ToolButton::Secondary => return Ok(()),
                };
                let next = if pressed { self.buttons | bit } else { self.buttons & !bit };
                context.navigate(NavigationInput::Pan(next != 0))?;
                self.buttons = next;
                return Ok(());
            }
            ToolInput::Wheel { steps, phase } => NavigationInput::Wheel { steps, phase },
            ToolInput::DoubleClick(point) => NavigationInput::DoubleClick(point),
            ToolInput::Fit => NavigationInput::Fit,
            ToolInput::OneToOne => NavigationInput::OneToOne,
        };
        context.navigate(input)?;
        if matches!(
            input,
            NavigationInput::DoubleClick(_) | NavigationInput::Fit | NavigationInput::OneToOne
        ) {
            self.buttons = 0;
        }
        Ok(())
    }
    fn cancel(&mut self, _: ToolCancelReason) {
        self.buttons = 0;
    }
    fn cursor(&self, view: &ToolView<'_>) -> CursorIntent {
        view.navigation.cursor()
    }
}
