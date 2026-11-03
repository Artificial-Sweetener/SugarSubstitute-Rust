// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The Resize shared edges tool: drag a seam between adjacent masks to resize them together.
//!
//! Behaviour follows the Python app's shared-edge mode over CuteCanvas's shared-edge resize.
//! Every visible mask of the document takes part, locked or not, as the Python app makes every
//! mask temporarily movable in this mode; other layers never do. Dragging a seam moves it along
//! its normal by whole document pixels, stretching the masks on one side and compressing those
//! on the other, each remapped from its own frame with area-weighted resampling so the masks
//! keep meeting without gaps. Each release is one checkpoint of an edit session with its own
//! Undo, Redo, Apply and Cancel; Apply publishes every resized mask as one undo step. Escape
//! with no session leaves the tool for the one selected before it.
//!
//! Masks are rasters spanning the document, so seams run along their frames' sides: only whole
//! edges move, never single seam endpoints, and drags do not snap to other edges.
mod seams;

use super::{
    CanvasTool, SessionAction, SessionKind, SessionState, ToolButton, ToolCancelReason,
    ToolCommand, ToolContext, ToolInput, ToolView,
    session::{Checkpoints, EditWork, Previewed},
};
use crate::{
    CanvasError, CursorIntent, DocumentEdit, DocumentSnapshot, LayerContent, LayerId,
    NavigationInput, OverlaySeam, ResizeDirection, overlay::ToolOverlay,
};
use seams::{Axis, Seam};

/// Coverage CuteCanvas counts as occupied: 128 of 255.
const OCCUPIED: u16 = 128 * 257;

/// The participating masks, their frames when the session began and the frames they went
/// through.
#[derive(Debug)]
struct Edges {
    base: (u64, u64),
    layers: Vec<LayerId>,
    frames: Checkpoints<Vec<[f64; 4]>>,
    preview: Previewed,
}
impl Edges {
    fn from(document: &DocumentSnapshot) -> Self {
        let (layers, frames) = document
            .layers()
            .iter()
            .filter(|layer| layer.visible())
            .filter_map(|layer| match layer.content() {
                LayerContent::Coverage { coverage, .. } => {
                    let bounds = coverage.bounds_at_least(OCCUPIED)?;
                    let frame = [bounds.x(), bounds.y(), bounds.right(), bounds.bottom()];
                    Some((layer.id(), frame.map(f64::from)))
                }
                _ => None,
            })
            .unzip();
        Self {
            base: (document.identity(), document.revision()),
            layers,
            frames: Checkpoints::new(frames),
            preview: Previewed::default(),
        }
    }
    fn work(&self, frames: &[[f64; 4]]) -> EditWork {
        let layers = self.layers.clone();
        let (from, to) = (self.frames.base().clone(), frames.to_vec());
        Box::new(move |base, job| {
            let mut edits = Vec::new();
            for ((id, from), to) in layers.iter().zip(&from).zip(&to) {
                if from == to {
                    continue;
                }
                let layer = base
                    .layers()
                    .iter()
                    .find(|layer| layer.id() == *id)
                    .ok_or(CanvasError::Invalid("A shared-edge mask is missing"))?;
                let LayerContent::Coverage { coverage, appearance } = layer.content() else {
                    return Err(CanvasError::Invalid("A shared-edge layer is not a mask"));
                };
                let coverage = coverage.stretch(*from, *to, job)?;
                edits.push(DocumentEdit::Replace(
                    layer
                        .clone()
                        .with_content(LayerContent::Coverage { coverage, appearance: *appearance }),
                ));
            }
            Ok(DocumentEdit::Group(edits))
        })
    }
    /// Preview the current checkpoint; at the base nothing has moved.
    fn show(&mut self, context: &mut ToolContext<'_>) -> Result<(), CanvasError> {
        if self.frames.current() == self.frames.base() {
            self.preview.discard(context);
            return Ok(());
        }
        let work = self.work(self.frames.current());
        self.preview.preview(context, work)
    }
}

/// A seam drag in progress.
#[derive(Debug)]
struct Drag {
    seam: Seam,
    origin: [f64; 2],
    start: Vec<[f64; 4]>,
    frames: Vec<[f64; 4]>,
    delta: f64,
}

/// Resizes adjacent masks together by dragging the seams they share.
#[derive(Debug, Default)]
pub struct SharedEdgesTool {
    position: Option<[f64; 2]>,
    edges: Option<Edges>,
    drag: Option<Drag>,
    seams: Vec<Seam>,
    hover: Option<usize>,
    overlay: Vec<OverlaySeam>,
}
impl SharedEdgesTool {
    /// A shared-edge tool with no session.
    pub fn new() -> Self {
        Self::default()
    }
    /// Find the participants again while no session is open, and the seams at this zoom.
    fn refresh(&mut self, document: Option<&DocumentSnapshot>, zoom: f64) {
        let open = self.edges.as_ref().is_some_and(|edges| edges.frames.changed());
        let current = document.map(|document| (document.identity(), document.revision()));
        if !open && self.edges.as_ref().map(|edges| edges.base) != current {
            self.edges = document.map(Edges::from);
        }
        let frames = match (&self.drag, &self.edges) {
            (Some(drag), _) => drag.frames.as_slice(),
            (None, Some(edges)) => edges.frames.current().as_slice(),
            (None, None) => &[],
        };
        self.seams = seams::discover(frames, zoom);
        let focus =
            self.drag.as_ref().map(|drag| (drag.seam.axis, drag.seam.position + drag.delta));
        self.overlay = self
            .seams
            .iter()
            .enumerate()
            .map(|(index, seam)| {
                let dragged = focus.is_some_and(|(axis, position)| {
                    axis == seam.axis && (seam.position - position).abs() <= 0.5
                });
                seam.overlay(dragged || Some(index) == self.hover)
            })
            .collect();
    }
    fn press(&mut self, context: &mut ToolContext<'_>) -> Result<(), CanvasError> {
        let (Some(position), Some(edges)) = (self.position, &self.edges) else {
            return Ok(());
        };
        let viewport = context.navigation().viewport();
        let Some(index) = seams::pick(&self.seams, viewport, position)? else {
            return Ok(());
        };
        let start = edges.frames.current().clone();
        self.drag = Some(Drag {
            seam: self.seams[index].clone(),
            origin: viewport.document_at(position)?,
            frames: start.clone(),
            start,
            delta: 0.0,
        });
        context.capture_pointer(true);
        Ok(())
    }
    fn moved(&mut self, context: &mut ToolContext<'_>) -> Result<(), CanvasError> {
        let (Some(drag), Some(position), Some(edges)) =
            (&mut self.drag, self.position, &mut self.edges)
        else {
            return Ok(());
        };
        let viewport = context.navigation().viewport();
        let point = viewport.document_at(position)?;
        let normal = drag.seam.axis.normal();
        let delta = (point[normal] - drag.origin[normal]).round();
        let (frames, delta) = drag.seam.moved(&drag.start, delta, viewport.zoom());
        if frames == drag.frames {
            return Ok(());
        }
        drag.frames = frames;
        drag.delta = delta;
        if &drag.frames == edges.frames.base() {
            edges.preview.discard(context);
            context.capture_pointer(true);
            return Ok(());
        }
        let work = edges.work(&drag.frames);
        edges.preview.preview(context, work)
    }
    fn release(&mut self, context: &mut ToolContext<'_>) -> Result<(), CanvasError> {
        self.moved(context)?;
        context.capture_pointer(false);
        let (Some(drag), Some(edges)) = (self.drag.take(), &mut self.edges) else {
            return Ok(());
        };
        edges.frames.record(drag.frames).map(drop)
    }
    fn resolve(&mut self, apply: bool, context: &mut ToolContext<'_>) -> Result<bool, CanvasError> {
        let Some(mut edges) = self.edges.take_if(|edges| edges.frames.changed()) else {
            return Ok(false);
        };
        if apply && edges.frames.current() != edges.frames.base() {
            let work = edges.work(edges.frames.current());
            edges.preview.publish(context, work)?;
        } else {
            edges.preview.discard(context);
        }
        Ok(true)
    }
}

impl CanvasTool for SharedEdgesTool {
    fn activate(&mut self, view: &ToolView<'_>) -> Result<(), CanvasError> {
        self.refresh(view.document.as_ref(), view.navigation.viewport().zoom());
        Ok(())
    }
    fn input(
        &mut self,
        input: ToolInput,
        context: &mut ToolContext<'_>,
    ) -> Result<(), CanvasError> {
        let zoom = context.navigation().viewport().zoom();
        let result = match input {
            ToolInput::Pointer(sample) => {
                self.position = Some(sample.position);
                if self.drag.is_some() {
                    self.moved(context)
                } else {
                    self.refresh(context.document().as_ref(), zoom);
                    let viewport = context.navigation().viewport();
                    self.hover = seams::pick(&self.seams, viewport, sample.position)?;
                    Ok(())
                }
            }
            ToolInput::Button { button: ToolButton::Primary, pressed, .. } => {
                match (pressed, self.drag.is_some()) {
                    (true, false) => {
                        self.refresh(context.document().as_ref(), zoom);
                        self.press(context)
                    }
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
        };
        self.refresh(context.document().as_ref(), context.navigation().viewport().zoom());
        result
    }
    fn command(
        &mut self,
        command: ToolCommand,
        context: &mut ToolContext<'_>,
    ) -> Result<bool, CanvasError> {
        let zoom = context.navigation().viewport().zoom();
        let handled = match command {
            ToolCommand::Cancel if self.drag.is_some() => {
                self.drag = None;
                if let Some(edges) = &mut self.edges {
                    edges.show(context)?;
                }
                context.capture_pointer(false);
                true
            }
            _ if self.drag.is_some() => false,
            ToolCommand::Commit | ToolCommand::Session(SessionAction::Apply) => {
                self.resolve(true, context)?
            }
            ToolCommand::Cancel | ToolCommand::Session(SessionAction::Cancel) => {
                if !self.resolve(false, context)? && command == ToolCommand::Cancel {
                    context.settle();
                }
                true
            }
            ToolCommand::Session(action) => {
                let Some(edges) = self.edges.as_mut().filter(|edges| edges.frames.changed()) else {
                    return Ok(false);
                };
                let moved = match action {
                    SessionAction::Undo => edges.frames.undo(),
                    _ => edges.frames.redo(),
                };
                if moved {
                    edges.show(context)?;
                }
                moved
            }
            ToolCommand::Nudge(_) | ToolCommand::Transform(_) => false,
        };
        self.refresh(context.document().as_ref(), zoom);
        Ok(handled)
    }
    fn cancel(&mut self, reason: ToolCancelReason) {
        self.hover = None;
        let drag = self.drag.take();
        if matches!(
            reason,
            ToolCancelReason::Requested
                | ToolCancelReason::DocumentChanged
                | ToolCancelReason::Shutdown
        ) {
            self.edges = None;
        } else if let (Some(drag), Some(edges)) = (drag, &mut self.edges) {
            // The latest preview already shows the interrupted drag; keep it as a checkpoint.
            if edges.frames.record(drag.frames).is_err() {
                self.edges = None;
            }
        }
    }
    fn cursor(&self, _: &ToolView<'_>) -> CursorIntent {
        let axis = match (&self.drag, self.hover) {
            (Some(drag), _) => Some(drag.seam.axis),
            (None, Some(index)) => self.seams.get(index).map(|seam| seam.axis),
            (None, None) => None,
        };
        match axis {
            Some(Axis::Vertical) => CursorIntent::Resize(ResizeDirection::Horizontal),
            Some(Axis::Horizontal) => CursorIntent::Resize(ResizeDirection::Vertical),
            None => CursorIntent::Arrow,
        }
    }
    fn overlay(&self) -> Option<ToolOverlay<'_>> {
        (!self.overlay.is_empty()).then_some(ToolOverlay::Seams(&self.overlay))
    }
    fn session(&self) -> Option<SessionState> {
        self.edges
            .as_ref()
            .filter(|edges| edges.frames.changed())
            .map(|edges| edges.frames.state(SessionKind::SharedEdges))
    }
}

#[cfg(test)]
mod tests;
