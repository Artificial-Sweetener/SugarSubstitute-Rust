// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish the native input region and apply gestures the desktop routed into it.
//!
//! The desktop adapter routes Win32 mouse messages against the region on the UI thread, so web
//! controls above the canvas keep their own input without a page round trip. Gestures arrive
//! in physical client pixels and become physical pixels of the native viewport.

use super::*;
use sugarsubstitute_canvas::{ScrollPhase, ToolButton, ToolInput};
use sugarsubstitute_desktop::canvas::{
    Input, MainCanvasButton, MainCanvasGesture, MainCanvasInput, MainCanvasInputRegion,
};

/// Whether a routed gesture reached the navigation tool.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::startup) enum RoutedInput {
    /// The session's navigation tool received the gesture.
    Applied,
    /// No committed final is shown for this lifetime, so the gesture was discarded.
    Refused,
}

impl NativeSurface {
    /// Publish the region only while a committed final image, not a preview or overview grid,
    /// is shown at the acknowledged placement of the current geometry and no modal blocks it.
    pub(in crate::startup::canvas) fn input_region(&self) -> Option<MainCanvasInputRegion<'_>> {
        let binding = self.binding.as_ref()?;
        let presented = binding.presented?;
        let navigable = !self.closed
            && !self.restoring
            && binding.committed
            && binding.selected
            && !binding.overview
            && !binding.modal
            && !self.has_shown_preview()
            && binding.placement == Some(presented.placement)
            && binding.allocation == Some(presented.allocation)
            && self.pending.as_ref().is_none_or(|pending| {
                matches!(pending.kind, PendingKind::Viewport { .. } | PendingKind::Geometry)
            });
        navigable.then_some(MainCanvasInputRegion {
            identity: binding.identity,
            placement: presented.placement,
            allocation: presented.allocation,
            occlusions: &binding.occlusions,
        })
    }

    /// Apply one routed gesture to the session's navigation tool.
    ///
    /// Gestures for another lifetime, or arriving after the region stopped qualifying, are
    /// refused; a cancellation for the current lifetime always ends any held drag.
    ///
    /// # Errors
    /// Preserves session input failures.
    pub(in crate::startup::canvas) fn routed_input(
        &mut self,
        input: MainCanvasInput,
        session: &mut Session,
    ) -> Result<RoutedInput, Box<dyn Error>> {
        let current =
            self.binding.as_ref().is_some_and(|binding| binding.identity == input.identity);
        if current && input.gesture == MainCanvasGesture::Cancel {
            session.input(Input::Unfocused)?;
            self.await_settled_gesture();
            return Ok(RoutedInput::Applied);
        }
        let Some(origin) =
            self.input_region().filter(|_| current).map(|region| region.placement.viewport)
        else {
            return Ok(RoutedInput::Refused);
        };
        let local = |[x, y]: [f64; 2]| [x - f64::from(origin.x), y - f64::from(origin.y)];
        let tool = match input.gesture {
            MainCanvasGesture::Pointer(point) => ToolInput::Pointer(local(point)),
            MainCanvasGesture::Button { button, pressed } => {
                ToolInput::Button { button: tool_button(button), pressed }
            }
            MainCanvasGesture::Wheel(steps) => {
                ToolInput::Wheel { steps, phase: ScrollPhase::Unphased }
            }
            MainCanvasGesture::DoubleClick(point) => ToolInput::DoubleClick(local(point)),
            MainCanvasGesture::Cancel => return Ok(RoutedInput::Refused),
        };
        session.input(Input::Tool(tool))?;
        // Hover moves never change the camera; a held drag stays pending until its release.
        if !matches!(input.gesture, MainCanvasGesture::Pointer(_)) {
            self.await_settled_gesture();
        }
        Ok(RoutedInput::Applied)
    }
}

const fn tool_button(button: MainCanvasButton) -> ToolButton {
    match button {
        MainCanvasButton::Primary => ToolButton::Primary,
        MainCanvasButton::Middle => ToolButton::Middle,
    }
}
