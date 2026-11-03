// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The Move tool: drag the target mask, or lift the selected pixels and move them as a floating
//! selection.
//!
//! Behaviour follows CuteCanvas's `MoveTool` and floating pixel session:
//! - With a selection, pressing inside it lifts the selected pixels ("move-cut"; Alt copies) and
//!   the selection travels with them. Pressing outside does nothing. Drags and arrow-key nudges
//!   accumulate in one floating session, a checkpoint each, with no document history until Enter
//!   commits it; Escape discards it.
//! - Without a selection, a drag moves the whole mask and lands as one undo step on release;
//!   each nudge is its own step.
//! - Offsets are whole document pixels. Shift turns the drag to the nearest 45° direction,
//!   keeping its length. Arrow keys move 1 pixel, 10 with Shift; nudges are ignored mid-drag.
//! - Locked layers and non-mask layers are never moved.
//!
//! Masks span the document, so content moved beyond its edge is cropped when the move lands;
//! until then every preview starts from the published mask, so dragging back restores it.
use super::{
    CanvasTool, SessionAction, SessionKind, SessionState, ToolButton, ToolCancelReason,
    ToolCommand, ToolContext, ToolInput, ToolModifiers, ToolView, coverage_work,
    session::{Checkpoints, EditWork, Previewed},
};
use crate::{CanvasError, CursorIntent, DocumentSnapshot, LayerId, NavigationInput};
use sugarsubstitute_image_ops::CoverageAffine;

/// A drag in progress, by whole document pixels from where it began.
#[derive(Debug)]
struct Drag {
    mask: LayerId,
    origin: [f64; 2],
    start: [i64; 2],
    offset: [i64; 2],
}

/// Lifted selected pixels and the offsets they were moved through.
#[derive(Debug)]
struct Floating {
    mask: LayerId,
    copy: bool,
    offsets: Checkpoints<[i64; 2]>,
    preview: Previewed,
}
impl Floating {
    fn new(mask: LayerId, copy: bool) -> Self {
        Self { mask, copy, offsets: Checkpoints::new([0, 0]), preview: Previewed::default() }
    }
    fn work(&self, offset: [i64; 2]) -> EditWork {
        let (mask, copy) = (self.mask, self.copy);
        Box::new(move |base, job| coverage_work::float(base, mask, translation(offset), copy, job))
    }
    /// Preview the current checkpoint; at the base nothing has moved.
    fn show(&mut self, context: &mut ToolContext<'_>) -> Result<(), CanvasError> {
        let offset = *self.offsets.current();
        if offset == [0, 0] {
            self.preview.discard(context);
            return Ok(());
        }
        let work = self.work(offset);
        self.preview.preview(context, work)
    }
    fn publish(mut self, context: &mut ToolContext<'_>) -> Result<(), CanvasError> {
        let offset = *self.offsets.current();
        if offset == [0, 0] {
            self.preview.discard(context);
            return Ok(());
        }
        let work = self.work(offset);
        self.preview.publish(context, work)
    }
}

/// Moves the target mask, or the selected pixels as a floating selection.
#[derive(Debug, Default)]
pub struct MoveTool {
    position: Option<[f64; 2]>,
    modifiers: ToolModifiers,
    drag: Option<Drag>,
    floating: Option<Floating>,
    layer: Previewed,
}
impl MoveTool {
    /// A move tool with nothing lifted.
    pub fn new() -> Self {
        Self::default()
    }
    fn press(&mut self, context: &mut ToolContext<'_>) -> Result<(), CanvasError> {
        let (Some(document), Some(position)) = (context.document(), self.position) else {
            return Ok(());
        };
        let Some(mask) = target(&document, context.target()) else {
            return Ok(());
        };
        let point = context.navigation().viewport().document_at(position)?;
        let start = match &self.floating {
            Some(floating) if inside(&document, point, *floating.offsets.current()) => {
                *floating.offsets.current()
            }
            Some(_) => return Ok(()),
            None if document.selection().is_some() => {
                if !inside(&document, point, [0, 0]) {
                    return Ok(());
                }
                self.floating = Some(Floating::new(mask, self.modifiers.alt));
                [0, 0]
            }
            None => [0, 0],
        };
        let mask = self.floating.as_ref().map_or(mask, |floating| floating.mask);
        self.drag = Some(Drag { mask, origin: point, start, offset: start });
        context.capture_pointer(true);
        Ok(())
    }
    fn moved(&mut self, context: &mut ToolContext<'_>) -> Result<(), CanvasError> {
        let (Some(drag), Some(position)) = (&mut self.drag, self.position) else {
            return Ok(());
        };
        let point = context.navigation().viewport().document_at(position)?;
        let delta =
            constrain([point[0] - drag.origin[0], point[1] - drag.origin[1]], self.modifiers.shift);
        let offset =
            [drag.start[0] + delta[0].round() as i64, drag.start[1] + delta[1].round() as i64];
        if offset == drag.offset {
            return Ok(());
        }
        drag.offset = offset;
        match &mut self.floating {
            Some(floating) => {
                let work = floating.work(offset);
                floating.preview.preview(context, work)
            }
            None if offset == [0, 0] => {
                self.layer.discard(context);
                context.capture_pointer(true);
                Ok(())
            }
            None => self.layer.preview(context, layer_work(drag.mask, offset)),
        }
    }
    fn release(&mut self, context: &mut ToolContext<'_>) -> Result<(), CanvasError> {
        self.moved(context)?;
        let Some(drag) = self.drag.take() else {
            return Ok(());
        };
        context.capture_pointer(false);
        match &mut self.floating {
            Some(floating) => floating.offsets.record(drag.offset).map(drop),
            None if drag.offset == [0, 0] => {
                self.layer.discard(context);
                Ok(())
            }
            None => self.layer.publish(context, layer_work(drag.mask, drag.offset)),
        }
    }
    fn nudge(
        &mut self,
        [dx, dy]: [i32; 2],
        context: &mut ToolContext<'_>,
    ) -> Result<bool, CanvasError> {
        if self.drag.is_some() {
            return Ok(false);
        }
        let delta = [i64::from(dx), i64::from(dy)];
        if let Some(floating) = &mut self.floating {
            let [x, y] = *floating.offsets.current();
            floating.offsets.record([x + delta[0], y + delta[1]])?;
            floating.show(context)?;
            return Ok(true);
        }
        let Some(document) = context.document() else {
            return Ok(false);
        };
        let Some(mask) = target(&document, context.target()) else {
            return Ok(false);
        };
        if document.selection().is_some() {
            let mut floating = Floating::new(mask, false);
            floating.offsets.record(delta)?;
            floating.show(context)?;
            self.floating = Some(floating);
            return Ok(true);
        }
        context.defer(super::Disposition::Commit, layer_work(mask, delta))?;
        Ok(true)
    }
    fn step(
        &mut self,
        action: SessionAction,
        context: &mut ToolContext<'_>,
    ) -> Result<bool, CanvasError> {
        let Some(floating) = self.floating.as_mut().filter(|_| self.drag.is_none()) else {
            return Ok(false);
        };
        let moved = match action {
            SessionAction::Undo => floating.offsets.undo(),
            _ => floating.offsets.redo(),
        };
        if moved {
            floating.show(context)?;
        }
        Ok(moved)
    }
}

impl CanvasTool for MoveTool {
    fn input(
        &mut self,
        input: ToolInput,
        context: &mut ToolContext<'_>,
    ) -> Result<(), CanvasError> {
        match input {
            ToolInput::Pointer(sample) => {
                self.position = Some(sample.position);
                self.modifiers = sample.modifiers;
                self.moved(context)
            }
            ToolInput::Button { button: ToolButton::Primary, pressed, modifiers } => {
                self.modifiers = modifiers;
                match (pressed, self.drag.is_some()) {
                    (true, false) => self.press(context),
                    (false, true) => self.release(context),
                    _ => Ok(()),
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
    fn command(
        &mut self,
        command: ToolCommand,
        context: &mut ToolContext<'_>,
    ) -> Result<bool, CanvasError> {
        match command {
            ToolCommand::Commit | ToolCommand::Session(SessionAction::Apply) => {
                if self.drag.is_some() {
                    return Ok(false);
                }
                let Some(floating) = self.floating.take() else {
                    return Ok(false);
                };
                floating.publish(context)?;
                Ok(true)
            }
            ToolCommand::Cancel | ToolCommand::Session(SessionAction::Cancel) => {
                let active = self.drag.take().is_some() | self.floating.take().is_some();
                if active {
                    self.layer.discard(context);
                }
                Ok(active)
            }
            ToolCommand::Nudge(delta) => self.nudge(delta, context),
            ToolCommand::Session(action) => self.step(action, context),
            ToolCommand::Transform(_) => Ok(false),
        }
    }
    fn cancel(&mut self, reason: ToolCancelReason) {
        let drag = self.drag.take();
        if matches!(
            reason,
            ToolCancelReason::Requested
                | ToolCancelReason::DocumentChanged
                | ToolCancelReason::Shutdown
        ) {
            self.floating = None;
        } else if let (Some(drag), Some(floating)) = (drag, &mut self.floating) {
            // The latest preview already shows the interrupted drag; keep it as a checkpoint.
            if floating.offsets.record(drag.offset).is_err() {
                self.floating = None;
            }
        }
        self.layer = Previewed::default();
    }
    fn cursor(&self, view: &ToolView<'_>) -> CursorIntent {
        let Some(document) = view.document.as_ref() else {
            return CursorIntent::Arrow;
        };
        if self.drag.is_some() {
            return CursorIntent::Move;
        }
        if target(document, view.target).is_none() && self.floating.is_none() {
            return CursorIntent::Arrow;
        }
        let point = self
            .position
            .and_then(|position| view.navigation.viewport().document_at(position).ok());
        let hovering = |offset| point.is_some_and(|point| inside(document, point, offset));
        match &self.floating {
            Some(floating) if hovering(*floating.offsets.current()) => CursorIntent::Move,
            Some(_) => CursorIntent::Arrow,
            None if document.selection().is_none() => CursorIntent::Move,
            None if hovering([0, 0]) && self.modifiers.alt => CursorIntent::Move,
            None if hovering([0, 0]) => CursorIntent::MoveCut,
            None => CursorIntent::Arrow,
        }
    }
    fn session(&self) -> Option<SessionState> {
        self.floating.as_ref().map(|floating| floating.offsets.state(SessionKind::Move))
    }
}

/// The editable mask the tool moves.
fn target(document: &DocumentSnapshot, target: Option<LayerId>) -> Option<LayerId> {
    target.filter(|mask| coverage_work::editable_mask(document, *mask).is_some())
}

/// Whether `point` lies on the selection moved by `offset`.
fn inside(document: &DocumentSnapshot, [x, y]: [f64; 2], [dx, dy]: [i64; 2]) -> bool {
    let Some(selection) = document.selection() else {
        return false;
    };
    let [x, y] = [(x - dx as f64).floor(), (y - dy as f64).floor()];
    // Positions beyond the extent saturate and read as uncovered.
    x >= 0.0 && y >= 0.0 && selection.value(x as u32, y as u32) != 0
}

/// With Shift, the drag turned to the nearest 45° direction, keeping its length.
fn constrain([x, y]: [f64; 2], shift: bool) -> [f64; 2] {
    let length = x.hypot(y);
    if !shift || length == 0.0 {
        return [x, y];
    }
    let step = std::f64::consts::FRAC_PI_4;
    let angle = (y.atan2(x) / step).round() * step;
    [angle.cos() * length, angle.sin() * length]
}

fn translation([dx, dy]: [i64; 2]) -> CoverageAffine {
    CoverageAffine::translation([dx as f64, dy as f64])
}

fn layer_work(mask: LayerId, offset: [i64; 2]) -> EditWork {
    Box::new(move |base, job| coverage_work::transform_mask(base, mask, translation(offset), job))
}

#[cfg(test)]
mod tests;
