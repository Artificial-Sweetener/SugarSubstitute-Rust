// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Rectangle, ellipse and lasso tools that draw the selection or add to the target mask.
//!
//! Gestures follow CuteCanvas: a drag replaces the selection, Shift held at the press adds and
//! Alt subtracts, with subtraction winning. Shift squares rectangles and ellipses; before the
//! first selection exists, Alt squares them too and the drag replaces. Ctrl suppresses snapping.
//! Pressing inside the selection without subtracting drags its outline instead. Mask shapes add
//! to the target mask by default and subtract with Alt. On release the shape is rasterized and
//! combined in an owned job and lands as one undo step.
mod gesture;
mod snap;

use super::{
    CanvasTool, Disposition, ToolButton, ToolCancelReason, ToolContext, ToolInput, ToolModifiers,
    ToolView, coverage_work,
};
use crate::{CanvasError, CursorIntent, DocumentSnapshot, NavigationInput, overlay::ToolOverlay};
use gesture::Gesture;
use snap::Snapper;
use sugarsubstitute_image_ops::CombineMode;

/// The outline a shape tool draws.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShapeKind {
    /// Corner-to-corner rectangle.
    Rectangle,
    /// Ellipse inscribed in the dragged rectangle.
    Ellipse,
    /// Freehand polygon through the dragged path.
    Lasso,
}

/// What a shape tool edits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShapeTarget {
    /// The document's selection.
    Selection,
    /// The tool target mask layer ([`super::ToolManager::set_target`]).
    Mask,
}

/// Feedback a shape tool asks the pointer to show.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Feedback {
    /// Nothing can be edited: no document, or no editable target mask.
    Unavailable,
    /// A precise crosshair for drawing.
    Precise,
    /// Drawing adds to existing coverage.
    PreciseAdd,
    /// Drawing subtracts from existing coverage.
    PreciseSubtract,
    /// The press would drag, or is dragging, the selection outline.
    Translate,
}

/// A selection outline being dragged by whole document pixels.
#[derive(Debug)]
struct Translation {
    origin: [f64; 2],
    offset: [i64; 2],
}

/// One rectangle, ellipse or lasso tool for the selection or the target mask.
#[derive(Debug)]
pub struct CoverageShapeTool {
    target: ShapeTarget,
    kind: ShapeKind,
    position: Option<[f64; 2]>,
    modifiers: ToolModifiers,
    available: bool,
    gesture: Option<Gesture>,
    translation: Option<Translation>,
}
impl CoverageShapeTool {
    /// A tool drawing `kind` shapes into `target`.
    pub const fn new(target: ShapeTarget, kind: ShapeKind) -> Self {
        Self {
            target,
            kind,
            position: None,
            modifiers: ToolModifiers::NONE,
            available: false,
            gesture: None,
            translation: None,
        }
    }
    /// Whether Alt squares the shape and replaces: only before a selection exists.
    fn alt_constrains(&self, document: &DocumentSnapshot) -> bool {
        self.target == ShapeTarget::Selection
            && self.modifiers.alt
            && document.selection().is_none()
    }
    fn subtracting(&self, document: &DocumentSnapshot) -> bool {
        self.modifiers.alt && !self.alt_constrains(document)
    }
    fn mode(&self, document: &DocumentSnapshot) -> CombineMode {
        if self.alt_constrains(document) {
            CombineMode::Replace
        } else if self.subtracting(document) {
            CombineMode::Subtract
        } else if self.modifiers.shift || self.target == ShapeTarget::Mask {
            CombineMode::Add
        } else {
            CombineMode::Replace
        }
    }
    /// Whether a press at `point` would drag the selection outline.
    fn over_selection(&self, document: &DocumentSnapshot, point: [f64; 2]) -> bool {
        if self.target != ShapeTarget::Selection || self.subtracting(document) {
            return false;
        }
        let Some(selection) = document.selection() else {
            return false;
        };
        let [x, y] = point.map(f64::floor);
        // Positions beyond the extent saturate and read as uncovered.
        x >= 0.0 && y >= 0.0 && selection.value(x as u32, y as u32) != 0
    }
    fn refresh(&mut self, context: &ToolContext<'_>) -> Option<DocumentSnapshot> {
        let document = context.document();
        self.available = document.as_ref().is_some_and(|document| match self.target {
            ShapeTarget::Selection => true,
            ShapeTarget::Mask => context
                .target()
                .is_some_and(|target| coverage_work::editable_mask(document, target).is_some()),
        });
        document.filter(|_| self.available)
    }
    fn press(&mut self, context: &mut ToolContext<'_>) -> Result<(), CanvasError> {
        let (Some(document), Some(position)) = (self.refresh(context), self.position) else {
            return Ok(());
        };
        let viewport = context.navigation().viewport();
        let point = viewport.document_at(position)?;
        if self.over_selection(&document, point) {
            self.translation = Some(Translation { origin: point, offset: [0, 0] });
        } else {
            let snap = (self.kind != ShapeKind::Lasso).then(|| {
                Snapper::begin(&document, point, 1.0 / viewport.zoom(), self.modifiers.ctrl)
            });
            let constrain = self.modifiers.shift || self.alt_constrains(&document);
            self.gesture =
                Some(Gesture::begin(self.kind, point, self.mode(&document), constrain, snap));
        }
        context.capture_pointer(true);
        Ok(())
    }
    fn moved(&mut self, context: &ToolContext<'_>) -> Result<(), CanvasError> {
        let Some(position) = self.position else {
            return Ok(());
        };
        if self.translation.is_none() && self.gesture.is_none() {
            return Ok(());
        }
        let viewport = context.navigation().viewport();
        let point = viewport.document_at(position)?;
        if let Some(translation) = &mut self.translation {
            translation.offset = [
                (point[0] - translation.origin[0]).round() as i64,
                (point[1] - translation.origin[1]).round() as i64,
            ];
        }
        let alt_constrains =
            context.document().is_some_and(|document| self.alt_constrains(&document));
        let (shift, ctrl) = (self.modifiers.shift, self.modifiers.ctrl);
        if let Some(gesture) = &mut self.gesture {
            gesture.update(point, 1.0 / viewport.zoom(), shift || alt_constrains, ctrl);
        }
        Ok(())
    }
    fn release(&mut self, context: &mut ToolContext<'_>) -> Result<(), CanvasError> {
        self.moved(context)?;
        context.capture_pointer(false);
        if let Some(Translation { offset, .. }) = self.translation.take() {
            if offset != [0, 0] {
                context.defer(Disposition::Commit, move |base, job| {
                    coverage_work::reshape_selection(base, |selection| {
                        Ok(selection.translate(offset, job)?)
                    })
                })?;
            }
            return Ok(());
        }
        let Some(gesture) = self.gesture.take() else {
            return Ok(());
        };
        let (Some(geometry), true) = (gesture.geometry(), self.available) else {
            return Ok(());
        };
        let mode = gesture.mode();
        match self.target {
            ShapeTarget::Selection => context.defer(Disposition::Commit, move |base, job| {
                let incoming = geometry.path()?.rasterize(base.extent(), job)?;
                coverage_work::select(base, &incoming, mode, job)
            }),
            ShapeTarget::Mask => {
                let target = context.target().ok_or(CanvasError::Invalid("No target mask"))?;
                context.defer(Disposition::Commit, move |base, job| {
                    let incoming = geometry.path()?.rasterize(base.extent(), job)?;
                    coverage_work::reshape_mask(base, target, |coverage| {
                        Ok(coverage.combine(&incoming, mode, job)?)
                    })
                })
            }
        }
    }
    fn feedback(&self, view: &ToolView<'_>) -> Feedback {
        let Some(document) = view.document.as_ref().filter(|_| self.available) else {
            return Feedback::Unavailable;
        };
        let hovering = self.position.is_some_and(|position| {
            view.navigation
                .viewport()
                .document_at(position)
                .is_ok_and(|point| self.over_selection(document, point))
        });
        if self.translation.is_some() || (self.gesture.is_none() && hovering) {
            Feedback::Translate
        } else if self.subtracting(document) {
            Feedback::PreciseSubtract
        } else if self.modifiers.shift {
            Feedback::PreciseAdd
        } else {
            Feedback::Precise
        }
    }
}

impl CanvasTool for CoverageShapeTool {
    fn input(
        &mut self,
        input: ToolInput,
        context: &mut ToolContext<'_>,
    ) -> Result<(), CanvasError> {
        match input {
            ToolInput::Pointer(sample) => {
                self.position = Some(sample.position);
                self.modifiers = sample.modifiers;
                self.refresh(context);
                self.moved(context)
            }
            ToolInput::Button { button: ToolButton::Primary, pressed, modifiers } => {
                self.modifiers = modifiers;
                if pressed {
                    if self.gesture.is_none() && self.translation.is_none() {
                        self.press(context)?;
                    }
                    Ok(())
                } else {
                    self.release(context)
                }
            }
            ToolInput::Button { .. } | ToolInput::DoubleClick(_) => Ok(()),
            ToolInput::Wheel { steps, phase } => {
                context.navigate(NavigationInput::Wheel { steps, phase })
            }
            ToolInput::Fit => context.navigate(NavigationInput::Fit),
            ToolInput::OneToOne => context.navigate(NavigationInput::OneToOne),
        }
    }
    fn cancel(&mut self, _: ToolCancelReason) {
        self.gesture = None;
        self.translation = None;
    }
    fn cursor(&self, view: &ToolView<'_>) -> CursorIntent {
        intent(self.feedback(view))
    }
    fn overlay(&self) -> Option<ToolOverlay<'_>> {
        if let Some(translation) = &self.translation {
            return Some(ToolOverlay::SelectionOffset(translation.offset));
        }
        self.gesture.as_ref()?.outline().map(ToolOverlay::Outline)
    }
}

/// The native cursor for shape feedback, following CuteCanvas: crosshairs marked by the combine
/// mode, the translate cursor over a draggable outline and the forbidden cursor with nothing to
/// edit.
const fn intent(feedback: Feedback) -> CursorIntent {
    match feedback {
        Feedback::Unavailable => CursorIntent::Forbidden,
        Feedback::Precise => CursorIntent::Precise,
        Feedback::PreciseAdd => CursorIntent::PreciseAdd,
        Feedback::PreciseSubtract => CursorIntent::PreciseSubtract,
        Feedback::Translate => CursorIntent::SelectionTranslate,
    }
}

#[cfg(test)]
mod tests;
