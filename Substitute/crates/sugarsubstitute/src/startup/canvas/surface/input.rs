// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish the native input region and apply gestures the desktop routed into it.
//!
//! The desktop adapter routes Win32 mouse and pen messages against the region on the UI thread,
//! so web controls above the canvas keep their own input without a page round trip. Gestures
//! arrive in physical client pixels and become physical pixels of the native viewport; pointer
//! samples keep their device kind, pressure, eraser and modifier facts. A drag-out request
//! copies the shown original on a worker before the desktop runs the OS drag.

use super::*;
use sugarsubstitute_canvas::{ScrollPhase, ToolButton, ToolInput};
use sugarsubstitute_desktop::canvas::{
    Input, MainCanvasButton, MainCanvasDragOut, MainCanvasGesture, MainCanvasInput,
    MainCanvasInputRegion,
};

/// One routed drag-out, bound to the lifetime and selection whose original it exports.
pub(super) struct DragRequest {
    identity: MainIdentity,
    request: u64,
    selection_revision: u64,
    filename: String,
}

/// Whether a routed gesture reached the navigation tool.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::startup) enum RoutedInput {
    /// The session's navigation tool received the gesture.
    Applied,
    /// No committed final is shown for this lifetime, so the gesture was discarded.
    Refused,
}

impl NativeSurface {
    /// Publish the region only while a committed document, a selected final image or the Input
    /// route's input document but never a preview or overview grid, is shown at the
    /// acknowledged placement of the current geometry and no modal blocks it. It offers
    /// drag-out only for a single selected output whose content cannot pan.
    pub(in crate::startup::canvas) fn input_region(
        &self,
        session: &Session,
    ) -> Option<MainCanvasInputRegion<'_>> {
        let binding = self.binding.as_ref()?;
        let presented = binding.presented?;
        let navigable = !self.closed
            && !self.restoring
            && binding.committed
            && binding.shows_document(session)
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
            drag_out: binding.drag_name.is_some() && session.allows_drag_out(),
            cursor: session.canvas_cursor(),
        })
    }

    /// Hand the copied original of the latest drag-out to the desktop, at most once.
    ///
    /// The request is dropped once its selection, lifetime or drag-out availability ends,
    /// so a late copy never exports a replaced output.
    pub(in crate::startup::canvas) fn take_drag_out(
        &mut self,
        session: &mut Session,
    ) -> Option<MainCanvasDragOut> {
        let pending = self.drag_out.as_ref()?;
        let current =
            self.input_region(session)
                .is_some_and(|region| region.identity == pending.identity && region.drag_out)
                && self.binding.as_ref().is_some_and(|binding| {
                    binding.selection_revision == pending.selection_revision
                });
        if !current {
            self.drag_out = None;
            return None;
        }
        let copied = session.take_drag_out(pending.request)?;
        let pending = self.drag_out.take()?;
        Some(MainCanvasDragOut {
            identity: pending.identity,
            filename: pending.filename,
            bytes: copied.bytes,
            claim: copied.claim,
            staged: copied.staged,
            preview: copied.preview,
        })
    }

    /// End the navigation gesture and start copying the shown original for an OS drag.
    fn request_drag_out(
        &mut self,
        identity: MainIdentity,
        session: &mut Session,
    ) -> Result<RoutedInput, Box<dyn Error>> {
        session.input(Input::Unfocused)?;
        let Some(binding) = self.binding.as_ref() else { return Ok(RoutedInput::Refused) };
        let Some(filename) = binding.drag_name.clone().filter(|_| session.allows_drag_out()) else {
            return Ok(RoutedInput::Refused);
        };
        let selection_revision = binding.selection_revision;
        self.drag_sequence =
            self.drag_sequence.checked_add(1).ok_or("Drag-out identities exhausted")?;
        let request = self.drag_sequence;
        self.drag_out = session
            .request_drag_out(request, filename.clone())?
            .then_some(DragRequest { identity, request, selection_revision, filename });
        Ok(RoutedInput::Applied)
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
            self.input_region(session).filter(|_| current).map(|region| region.placement.viewport)
        else {
            return Ok(RoutedInput::Refused);
        };
        if input.gesture == MainCanvasGesture::DragOut {
            return self.request_drag_out(input.identity, session);
        }
        let Some(tool) = tool_input(input.gesture, [origin.x, origin.y].map(f64::from)) else {
            return Ok(RoutedInput::Refused);
        };
        session.input(Input::Tool(tool))?;
        // Hover moves never change the camera; a held drag stays pending until its release.
        if !matches!(input.gesture, MainCanvasGesture::Pointer(_)) {
            self.await_settled_gesture();
        }
        Ok(RoutedInput::Applied)
    }
}

/// Convert one routed gesture to tool input in viewport pixels, given the viewport's physical
/// client origin. Pointer samples keep their device kind, pressure, eraser and modifier facts;
/// cancellation and drag-out are host lifecycle events with no tool input.
fn tool_input(gesture: MainCanvasGesture, origin: [f64; 2]) -> Option<ToolInput> {
    let local = |[x, y]: [f64; 2]| [x - origin[0], y - origin[1]];
    Some(match gesture {
        MainCanvasGesture::Pointer(sample) => ToolInput::Pointer(sample.at(local(sample.position))),
        MainCanvasGesture::Button { button, pressed, modifiers } => {
            ToolInput::Button { button: tool_button(button), pressed, modifiers }
        }
        MainCanvasGesture::Wheel(steps) => ToolInput::Wheel { steps, phase: ScrollPhase::Unphased },
        MainCanvasGesture::DoubleClick(point) => ToolInput::DoubleClick(local(point)),
        MainCanvasGesture::Cancel | MainCanvasGesture::DragOut => return None,
    })
}

const fn tool_button(button: MainCanvasButton) -> ToolButton {
    match button {
        MainCanvasButton::Primary => ToolButton::Primary,
        MainCanvasButton::Middle => ToolButton::Middle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sugarsubstitute_canvas::{PointerKind, PointerSample, Pressure, ToolModifiers};

    #[test]
    fn routed_samples_move_into_the_viewport_and_keep_their_device_facts() {
        let modifiers = ToolModifiers { shift: true, ctrl: false, alt: true };
        let pen = PointerSample {
            position: [110.5, 70.0],
            kind: PointerKind::Pen,
            pressure: Pressure::from_range(300, 1024),
            eraser: true,
            modifiers,
        };
        let origin = [10.0, 20.0];
        assert_eq!(
            tool_input(MainCanvasGesture::Pointer(pen), origin),
            Some(ToolInput::Pointer(PointerSample { position: [100.5, 50.0], ..pen }))
        );
        assert_eq!(
            tool_input(
                MainCanvasGesture::Button {
                    button: MainCanvasButton::Middle,
                    pressed: true,
                    modifiers
                },
                origin
            ),
            Some(ToolInput::Button { button: ToolButton::Middle, pressed: true, modifiers })
        );
        assert_eq!(
            tool_input(MainCanvasGesture::DoubleClick([30.0, 40.0]), origin),
            Some(ToolInput::DoubleClick([20.0, 20.0]))
        );
        assert_eq!(tool_input(MainCanvasGesture::Cancel, origin), None);
        assert_eq!(tool_input(MainCanvasGesture::DragOut, origin), None);
    }
}
