// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The Transform tool: rotate, flip, scale, skew and move the target mask or the selection.
//!
//! Behaviour follows the Python app's transform contextual toolbar over CuteCanvas's transform
//! tool. With a selection the tool transforms the selected pixels of the target mask as a
//! floating fragment, and the selection with them; otherwise it transforms the whole mask. The
//! box frames the selection's bounds or the mask's content bounds. Every handle drag and every
//! toolbar turn or flip is one session checkpoint. Enter or a double click inside the box
//! applies the transform as one undo step, Escape cancels it, and either returns to the tool
//! selected before Transform. Moving the box follows the pointer and snaps to the canvas, the
//! selection and other layers. The result is resampled once, from the published mask, when the
//! transform lands: quarter turns, flips and whole-pixel moves permute pixels exactly; anything
//! else is sampled bilinearly, as CuteCanvas resamples transformed fragments.
mod frame;

use super::{
    CanvasTool, SessionAction, SessionKind, SessionState, ToolButton, ToolCancelReason,
    ToolCommand, ToolContext, ToolInput, ToolModifiers, ToolView,
    box_snap::MoveSnap,
    coverage_work,
    session::{Checkpoints, EditWork, Previewed},
};
use crate::{
    CanvasError, CursorIntent, DocumentSnapshot, LayerId, NavigationInput, SnapGuide,
    overlay::ToolOverlay,
};
use frame::{Frame, Grip};
use sugarsubstitute_image_ops::CoverageAffine;

/// What the box transforms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Target {
    /// The whole mask.
    Layer(LayerId),
    /// The selected pixels of the mask, with the selection.
    Selection(LayerId),
}

/// The framed content and the transforms it went through.
#[derive(Debug)]
struct Prepared {
    base: (u64, u64),
    target: Target,
    rect: [f64; 4],
    affines: Checkpoints<CoverageAffine>,
    preview: Previewed,
}
impl Prepared {
    /// Frame the selection's bounds, or the target mask's whole content, including what lies
    /// beyond the document.
    fn from(document: &DocumentSnapshot, mask: Option<LayerId>) -> Option<Self> {
        let mask = mask?;
        let layer = coverage_work::editable_mask(document, mask)?;
        let (target, bounds) = match document.selection() {
            Some(selection) => {
                let bounds = selection.bounds()?;
                let bounds = [bounds.x(), bounds.y(), bounds.right(), bounds.bottom()];
                (Target::Selection(mask), bounds.map(i64::from))
            }
            None => {
                let crate::LayerContent::Coverage { coverage, .. } = layer.content() else {
                    return None;
                };
                (Target::Layer(mask), coverage.content_bounds()?)
            }
        };
        Some(Self {
            base: (document.identity(), document.revision()),
            target,
            rect: bounds.map(|value| value as f64),
            affines: Checkpoints::new(CoverageAffine::IDENTITY),
            preview: Previewed::default(),
        })
    }
    fn frame(&self) -> Frame {
        Frame { rect: self.rect, affine: *self.affines.current() }
    }
    fn work(&self, affine: CoverageAffine) -> EditWork {
        match self.target {
            Target::Layer(mask) => {
                Box::new(move |base, job| coverage_work::transform_mask(base, mask, affine, job))
            }
            Target::Selection(mask) => {
                Box::new(move |base, job| coverage_work::float(base, mask, affine, false, job))
            }
        }
    }
    fn show(
        &mut self,
        affine: CoverageAffine,
        context: &mut ToolContext<'_>,
    ) -> Result<(), CanvasError> {
        if affine.is_identity() {
            self.preview.discard(context);
            return Ok(());
        }
        let work = self.work(affine);
        self.preview.preview(context, work)
    }
}

/// A handle, rotation or move drag in progress.
#[derive(Debug)]
struct Gesture {
    grip: Grip,
    origin: [f64; 2],
    start: Frame,
    snap: Option<MoveSnap>,
    latest: CoverageAffine,
}

/// Transforms the target mask or the selected pixels inside a box with handles.
#[derive(Debug, Default)]
pub struct TransformTool {
    position: Option<[f64; 2]>,
    modifiers: ToolModifiers,
    prepared: Option<Prepared>,
    gesture: Option<Gesture>,
    hover: Option<Grip>,
}
impl TransformTool {
    /// A transform tool with nothing framed yet.
    pub fn new() -> Self {
        Self::default()
    }
    /// Frame the current content again while no change is unresolved.
    fn refresh(&mut self, document: Option<&DocumentSnapshot>, mask: Option<LayerId>) {
        let open = self.prepared.as_ref().is_some_and(|prepared| prepared.affines.changed());
        if open {
            return;
        }
        let current = document.map(|document| (document.identity(), document.revision()));
        let stale = self.prepared.as_ref().is_none_or(|prepared| {
            Some(prepared.base) != current
                || !matches!(prepared.target, Target::Layer(id) | Target::Selection(id) if Some(id) == mask)
        });
        if stale {
            self.prepared = document.and_then(|document| Prepared::from(document, mask));
        }
    }
    fn frame(&self) -> Option<Frame> {
        let prepared = self.prepared.as_ref()?;
        let mut frame = prepared.frame();
        if let Some(gesture) = &self.gesture {
            frame.affine = gesture.latest;
        }
        Some(frame)
    }
    fn press(&mut self, context: &mut ToolContext<'_>) -> Result<(), CanvasError> {
        let (Some(frame), Some(position)) = (self.frame(), self.position) else {
            return Ok(());
        };
        let viewport = context.navigation().viewport();
        let Some(grip) = frame.grip(viewport, position, self.modifiers)? else {
            return Ok(());
        };
        let origin = viewport.document_at(position)?;
        let snap = match (grip, &self.prepared, context.document()) {
            (Grip::Move, Some(prepared), Some(document)) => {
                let (mask, selection) = match prepared.target {
                    Target::Layer(mask) => (mask, false),
                    Target::Selection(mask) => (mask, true),
                };
                Some(MoveSnap::begin(&document, mask, selection, frame.bounds()))
            }
            _ => None,
        };
        self.gesture = Some(Gesture { grip, origin, start: frame, snap, latest: frame.affine });
        context.capture_pointer(true);
        Ok(())
    }
    fn moved(&mut self, context: &mut ToolContext<'_>) -> Result<(), CanvasError> {
        let Some(position) = self.position else {
            return Ok(());
        };
        let viewport = context.navigation().viewport();
        let Some(gesture) = &mut self.gesture else {
            self.hover = match self.frame() {
                Some(frame) => frame.grip(viewport, position, self.modifiers)?,
                None => None,
            };
            return Ok(());
        };
        let point = viewport.document_at(position)?;
        let point = match &mut gesture.snap {
            Some(snap) => {
                let raw = [point[0] - gesture.origin[0], point[1] - gesture.origin[1]];
                let delta = snap.resolve(raw, viewport.zoom(), self.modifiers.ctrl);
                [gesture.origin[0] + delta[0], gesture.origin[1] + delta[1]]
            }
            None => point,
        };
        let Some(affine) = gesture.start.drag(gesture.grip, gesture.origin, point, self.modifiers)
        else {
            return Ok(());
        };
        if affine == gesture.latest {
            return Ok(());
        }
        gesture.latest = affine;
        match &mut self.prepared {
            Some(prepared) => prepared.show(affine, context),
            None => Ok(()),
        }
    }
    fn release(&mut self, context: &mut ToolContext<'_>) -> Result<(), CanvasError> {
        self.moved(context)?;
        context.capture_pointer(false);
        let (Some(gesture), Some(prepared)) = (self.gesture.take(), &mut self.prepared) else {
            return Ok(());
        };
        prepared.affines.record(gesture.latest).map(drop)
    }
    /// Apply or cancel the transform and return to the tool selected before Transform.
    fn settle(&mut self, apply: bool, context: &mut ToolContext<'_>) -> Result<(), CanvasError> {
        self.gesture = None;
        if let Some(mut prepared) = self.prepared.take() {
            let affine = *prepared.affines.current();
            if apply && prepared.affines.changed() && !affine.is_identity() {
                let work = prepared.work(affine);
                prepared.preview.publish(context, work)?;
            } else {
                prepared.preview.discard(context);
            }
        }
        context.settle();
        Ok(())
    }
}

impl CanvasTool for TransformTool {
    fn activate(&mut self, view: &ToolView<'_>) -> Result<(), CanvasError> {
        self.refresh(view.document.as_ref(), view.target);
        Ok(())
    }
    fn input(
        &mut self,
        input: ToolInput,
        context: &mut ToolContext<'_>,
    ) -> Result<(), CanvasError> {
        if self.gesture.is_none() {
            self.refresh(context.document().as_ref(), context.target());
        }
        match input {
            ToolInput::Pointer(sample) => {
                self.position = Some(sample.position);
                self.modifiers = sample.modifiers;
                self.moved(context)
            }
            ToolInput::Button { button: ToolButton::Primary, pressed, modifiers } => {
                self.modifiers = modifiers;
                match (pressed, self.gesture.is_some()) {
                    (true, false) => self.press(context),
                    (false, true) => self.release(context),
                    _ => Ok(()),
                }
            }
            ToolInput::DoubleClick(position) => {
                let viewport = context.navigation().viewport();
                let inside = match self.frame() {
                    Some(frame) => {
                        frame.grip(viewport, position, self.modifiers)? == Some(Grip::Move)
                    }
                    None => false,
                };
                if inside && self.gesture.is_none() {
                    self.settle(true, context)?;
                }
                Ok(())
            }
            ToolInput::Button { .. } => Ok(()),
            ToolInput::Wheel { steps, phase } => {
                context.navigate(NavigationInput::Wheel { steps, phase })
            }
            ToolInput::Fit => context.navigate(NavigationInput::Fit),
            ToolInput::OneToOne => context.navigate(NavigationInput::OneToOne),
        }
    }
    fn command(
        &mut self,
        command: ToolCommand,
        context: &mut ToolContext<'_>,
    ) -> Result<bool, CanvasError> {
        if self.gesture.is_some() && command != ToolCommand::Cancel {
            return Ok(false);
        }
        self.refresh(context.document().as_ref(), context.target());
        match command {
            ToolCommand::Commit | ToolCommand::Session(SessionAction::Apply) => {
                self.settle(true, context)?;
                Ok(true)
            }
            ToolCommand::Cancel | ToolCommand::Session(SessionAction::Cancel) => {
                self.settle(false, context)?;
                Ok(true)
            }
            ToolCommand::Transform(action) => {
                let Some(prepared) = &mut self.prepared else {
                    return Ok(false);
                };
                let affine = prepared.frame().turned(action);
                prepared.affines.record(affine)?;
                prepared.show(affine, context)?;
                Ok(true)
            }
            ToolCommand::Session(action) => {
                let Some(prepared) = &mut self.prepared else {
                    return Ok(false);
                };
                let moved = match action {
                    SessionAction::Undo => prepared.affines.undo(),
                    _ => prepared.affines.redo(),
                };
                if moved {
                    let affine = *prepared.affines.current();
                    prepared.show(affine, context)?;
                }
                Ok(moved)
            }
            ToolCommand::Nudge(_) => Ok(false),
        }
    }
    fn cancel(&mut self, reason: ToolCancelReason) {
        self.hover = None;
        let gesture = self.gesture.take();
        if matches!(
            reason,
            ToolCancelReason::Requested
                | ToolCancelReason::DocumentChanged
                | ToolCancelReason::Shutdown
        ) {
            self.prepared = None;
        } else if let (Some(gesture), Some(prepared)) = (gesture, &mut self.prepared) {
            // The latest preview already shows the interrupted drag; keep it as a checkpoint.
            if prepared.affines.record(gesture.latest).is_err() {
                self.prepared = None;
            }
        }
    }
    fn cursor(&self, view: &ToolView<'_>) -> CursorIntent {
        let grip = self.gesture.as_ref().map(|gesture| gesture.grip).or(self.hover);
        match (self.frame(), grip) {
            (Some(frame), Some(grip)) => {
                frame.cursor(view.navigation.viewport(), grip, self.position.unwrap_or_default())
            }
            _ => CursorIntent::Arrow,
        }
    }
    fn overlay(&self) -> Option<ToolOverlay<'_>> {
        self.frame().map(|frame| ToolOverlay::Frame(frame.corners()))
    }
    fn snap_guides(&self) -> &[SnapGuide] {
        self.gesture
            .as_ref()
            .and_then(|gesture| gesture.snap.as_ref())
            .map_or(&[], MoveSnap::guides)
    }
    fn session(&self) -> Option<SessionState> {
        self.prepared
            .as_ref()
            .filter(|prepared| prepared.affines.changed())
            .map(|prepared| prepared.affines.state(SessionKind::Transform))
    }
}

#[cfg(test)]
mod tests;
