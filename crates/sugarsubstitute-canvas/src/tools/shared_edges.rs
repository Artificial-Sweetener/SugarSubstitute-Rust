// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The Resize shared edges tool: drag a seam between adjacent masks to reshape them together.
//!
//! Behaviour follows the Python app's shared-edge mode over CuteCanvas's shared-edge resize.
//! Every visible mask of the document takes part, locked or not, as the Python app makes every
//! mask temporarily movable in this mode; other layers only bound and snap. Each mask is framed
//! by the convex boundary of its original coverage under its current mapping, and seams are
//! found and dragged as the `seams` and `gesture` modules describe: the middle handle moves an
//! axis-aligned edge whole, by any fraction of a pixel, and an endpoint slides along the masks'
//! shared outer rail. Each release is one checkpoint of an edit session with its own Undo,
//! Redo, Apply and Cancel; every later drag must move the same masks. Each preview and Apply
//! resamples the original coverage through its mapping: affine maps bilinearly, piecewise and
//! bilinear maps by nearest pixel, as CuteCanvas resamples them. Apply publishes every reshaped
//! mask as one undo step; Escape with no session leaves for the tool selected before.
mod collapse;
mod edges;
mod gesture;
mod masks;
mod rail;
mod seams;

use super::{
    CanvasTool, SessionAction, SessionKind, SessionState, ToolButton, ToolCancelReason,
    ToolCommand, ToolContext, ToolInput, ToolModifiers, ToolView,
};
use crate::{
    AffineCursor, CanvasError, CursorIntent, DocumentSnapshot, NavigationInput, OverlaySeam,
    ResizeDirection, SeamHandle, overlay::ToolOverlay,
};
use edges::{Edge, Owner, distance, rectangle_edges};
use gesture::{Gesture, Pivot};
use masks::{Edges, shapes};
use seams::{Discovery, Handle, handle_at};
use sugarsubstitute_image_ops::CoverageMapping;

/// A seam drag and the mappings it shows now.
#[derive(Debug)]
struct Drag {
    gesture: Gesture,
    mappings: Vec<CoverageMapping>,
}

/// Reshapes adjacent masks together by dragging the seams they share.
#[derive(Debug, Default)]
pub struct SharedEdgesTool {
    position: Option<[f64; 2]>,
    modifiers: ToolModifiers,
    zoom: f64,
    edges: Option<Edges>,
    drag: Option<Drag>,
    discovery: Discovery,
    pivots: Vec<[Option<Pivot>; 2]>,
    hover: Option<(usize, Handle)>,
    overlay: Vec<OverlaySeam>,
}
impl SharedEdgesTool {
    /// A shared-edge tool with no session.
    pub fn new() -> Self {
        Self::default()
    }

    /// Find the participants again while no session is open, and the seams at this zoom.
    fn refresh(&mut self, document: Option<&DocumentSnapshot>, zoom: f64) {
        self.zoom = zoom;
        let open = self.edges.as_ref().is_some_and(|edges| edges.mappings.changed());
        let current = document.map(|document| (document.identity(), document.revision()));
        if !open && self.edges.as_ref().map(|edges| edges.base) != current {
            self.edges = document.map(Edges::from);
        }
        let (Some(document), Some(edges)) = (document, &self.edges) else {
            self.discovery = Discovery::default();
            self.pivots.clear();
            self.overlay.clear();
            return;
        };
        let mappings = self.drag.as_ref().map_or(edges.mappings.current(), |drag| &drag.mappings);
        let shapes = shapes(document, edges, mappings);
        let scale = 1.0 / zoom.max(1e-9);
        self.discovery = Discovery::new(&shapes, scale);
        let discovery = &self.discovery;
        self.pivots = discovery
            .seams
            .iter()
            .map(|seam| {
                [Handle::Start, Handle::End]
                    .map(|handle| Pivot::of(seam, handle, &discovery.edges, discovery.tolerance))
            })
            .collect();
        let dragged = self.drag.as_ref().map(|drag| drag.gesture.ends);
        self.overlay = discovery
            .seams
            .iter()
            .zip(&self.pivots)
            .enumerate()
            .map(|(index, (seam, [start, end]))| {
                let active = dragged.is_some_and(|[a, b]| {
                    distance(a, seam.start()).max(distance(b, seam.end())) <= discovery.tolerance
                });
                let hovered =
                    self.hover.filter(|(seam, _)| *seam == index).map(|(_, handle)| handle);
                let handle = self
                    .drag
                    .as_ref()
                    .filter(|_| active)
                    .map(|drag| drag.gesture.handle)
                    .or(hovered);
                OverlaySeam {
                    start: seam.start(),
                    end: seam.end(),
                    focused: active || hovered.is_some(),
                    handle: handle.map(overlay_handle),
                    enabled: [start.is_some(), seam.translatable(), end.is_some()],
                }
            })
            .collect();
    }

    fn press(&mut self, context: &mut ToolContext<'_>) -> Result<(), CanvasError> {
        let (Some(position), Some(edges)) = (self.position, &self.edges) else {
            return Ok(());
        };
        let viewport = context.navigation().viewport();
        let point = viewport.document_at(position)?;
        let scale = 1.0 / viewport.zoom();
        let Some(index) = self.discovery.seam_at(point, scale) else {
            return Ok(());
        };
        let seam = self.discovery.seams[index].clone();
        let layers = seam.layers();
        if edges.participants.as_ref().is_some_and(|participants| *participants != layers) {
            return Ok(());
        }
        let handle = handle_at(&seam, point, scale);
        let pivot = (handle != Handle::Middle)
            .then(|| Pivot::of(&seam, handle, &self.discovery.edges, self.discovery.tolerance))
            .flatten();
        let mut targets: Vec<Edge> = self
            .discovery
            .edges
            .iter()
            .filter(|edge| !matches!(edge.owner, Owner::Layer(layer) if layers.contains(&layer)))
            .copied()
            .collect();
        if let Some(document) = context.document() {
            let extent = document.extent();
            let canvas = [0.0, 0.0, f64::from(extent.width()), f64::from(extent.height())];
            targets.extend(rectangle_edges(Owner::Canvas, canvas, 20));
        }
        let Some(gesture) = Gesture::begin(seam, handle, pivot, point, targets) else {
            return Ok(());
        };
        let mappings = edges.mappings.current().clone();
        self.drag = Some(Drag { gesture, mappings });
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
        let Some(changes) = drag.gesture.resolve(point, 1.0 / viewport.zoom(), self.modifiers.ctrl)
        else {
            return Ok(());
        };
        let mappings = edges.with(&changes);
        if mappings == drag.mappings {
            return Ok(());
        }
        drag.mappings = mappings;
        edges.show(&drag.mappings, context)?;
        context.capture_pointer(true);
        Ok(())
    }

    fn release(&mut self, context: &mut ToolContext<'_>) -> Result<(), CanvasError> {
        self.moved(context)?;
        context.capture_pointer(false);
        let (Some(drag), Some(edges)) = (self.drag.take(), &mut self.edges) else {
            return Ok(());
        };
        if edges.mappings.record(drag.mappings)? && edges.participants.is_none() {
            edges.participants = Some(drag.gesture.seam.layers());
        }
        Ok(())
    }

    fn resolve(&mut self, apply: bool, context: &mut ToolContext<'_>) -> Result<bool, CanvasError> {
        let Some(mut edges) = self.edges.take_if(|edges| edges.mappings.changed()) else {
            return Ok(false);
        };
        if apply && edges.mappings.current() != edges.mappings.base() {
            let work = edges.work(edges.mappings.current());
            edges.preview.publish(context, work)?;
        } else {
            edges.preview.discard(context);
        }
        Ok(true)
    }

    fn step(
        &mut self,
        action: SessionAction,
        context: &mut ToolContext<'_>,
    ) -> Result<bool, CanvasError> {
        let Some(edges) = self.edges.as_mut().filter(|edges| edges.mappings.changed()) else {
            return Ok(false);
        };
        let moved = match action {
            SessionAction::Undo => edges.mappings.undo(),
            _ => edges.mappings.redo(),
        };
        if moved {
            let current = edges.mappings.current().clone();
            edges.show(&current, context)?;
        }
        Ok(moved)
    }
}

const fn overlay_handle(handle: Handle) -> SeamHandle {
    match handle {
        Handle::Start => SeamHandle::Start,
        Handle::Middle => SeamHandle::Middle,
        Handle::End => SeamHandle::End,
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
                self.modifiers = sample.modifiers;
                if self.drag.is_some() {
                    self.moved(context)
                } else {
                    self.refresh(context.document().as_ref(), zoom);
                    let viewport = context.navigation().viewport();
                    let point = viewport.document_at(sample.position)?;
                    let scale = 1.0 / zoom;
                    self.hover = self.discovery.seam_at(point, scale).map(|index| {
                        (index, handle_at(&self.discovery.seams[index], point, scale))
                    });
                    Ok(())
                }
            }
            ToolInput::Button { button: ToolButton::Primary, pressed, modifiers } => {
                self.modifiers = modifiers;
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
        let handled = match command {
            ToolCommand::Cancel if self.drag.is_some() => {
                self.drag = None;
                if let Some(edges) = &mut self.edges {
                    let current = edges.mappings.current().clone();
                    edges.show(&current, context)?;
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
            ToolCommand::Session(action) => self.step(action, context)?,
            ToolCommand::Nudge(_) | ToolCommand::Transform(_) => false,
        };
        let zoom = context.navigation().viewport().zoom();
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
            self.overlay.clear();
        } else if let (Some(drag), Some(edges)) = (drag, &mut self.edges) {
            // The latest preview already shows the interrupted drag; keep it as a checkpoint.
            let layers = drag.gesture.seam.layers();
            match edges.mappings.record(drag.mappings) {
                Ok(true) if edges.participants.is_none() => edges.participants = Some(layers),
                Ok(_) => {}
                Err(_) => self.edges = None,
            }
        }
    }
    fn cursor(&self, _: &ToolView<'_>) -> CursorIntent {
        let gripped = match (&self.drag, self.hover) {
            (Some(drag), _) => Some((
                &drag.gesture.seam,
                drag.gesture.handle,
                self.discovery.seams.iter().position(|seam| *seam == drag.gesture.seam),
            )),
            (None, Some((index, handle))) => {
                self.discovery.seams.get(index).map(|seam| (seam, handle, Some(index)))
            }
            (None, None) => None,
        };
        let Some((seam, handle, index)) = gripped else {
            return CursorIntent::Arrow;
        };
        match handle {
            Handle::Middle if seam.translatable() || self.drag.is_some() => {
                CursorIntent::Resize(ResizeDirection::along(seam.edge.normal()))
            }
            Handle::Middle => CursorIntent::Forbidden,
            Handle::Start | Handle::End => {
                let side = usize::from(handle == Handle::End);
                let pivot = index
                    .and_then(|index| self.pivots.get(index))
                    .and_then(|pivots| pivots[side].as_ref());
                match pivot {
                    Some(pivot) => {
                        let [start, end] = pivot.rail();
                        let degrees = (end[1] - start[1]).atan2(end[0] - start[0]).to_degrees();
                        CursorIntent::Affine(AffineCursor::resize(degrees))
                    }
                    None => CursorIntent::Forbidden,
                }
            }
        }
    }
    fn overlay(&self) -> Option<ToolOverlay<'_>> {
        (!self.overlay.is_empty()).then_some(ToolOverlay::Seams(&self.overlay))
    }
    fn session(&self) -> Option<SessionState> {
        self.edges
            .as_ref()
            .filter(|edges| edges.mappings.changed())
            .map(|edges| edges.mappings.state(SessionKind::SharedEdges))
    }
}

#[cfg(test)]
mod tests;
