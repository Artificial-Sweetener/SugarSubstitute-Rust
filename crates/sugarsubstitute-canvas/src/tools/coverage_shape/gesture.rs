// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The document-space geometry of one rectangle, ellipse or lasso drag.
use super::{ShapeKind, snap::Snapper};
use crate::{CanvasError, overlay::OverlayShape};
use sugarsubstitute_image_ops::{CombineMode, Path};

/// Most lasso points one gesture keeps; later samples move the final point.
const MAX_LASSO_POINTS: usize = 1 << 16;

/// Shape geometry captured on the input thread and rasterized inside the deferred job.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ShapeGeometry {
    Rectangle([f64; 4]),
    Ellipse([f64; 4]),
    Polygon(Vec<[f64; 2]>),
}
impl ShapeGeometry {
    pub(crate) fn path(self) -> Result<Path, CanvasError> {
        Ok(match self {
            Self::Rectangle(bounds) => Path::rectangle(bounds)?,
            Self::Ellipse(bounds) => Path::ellipse(bounds)?,
            Self::Polygon(points) => Path::polygon(points)?,
        })
    }
}

/// One drag from press to release.
#[derive(Debug)]
pub(super) struct Gesture {
    kind: ShapeKind,
    anchor: [f64; 2],
    end: [f64; 2],
    points: Vec<[f64; 2]>,
    mode: CombineMode,
    constrain: bool,
    snap: Option<Snapper>,
}
impl Gesture {
    /// Begin at a document point. The combine mode is fixed for the whole gesture.
    pub(super) fn begin(
        kind: ShapeKind,
        point: [f64; 2],
        mode: CombineMode,
        constrain: bool,
        snap: Option<Snapper>,
    ) -> Self {
        let anchor = snap.as_ref().map_or(point, Snapper::anchor);
        let points = if kind == ShapeKind::Lasso { vec![anchor] } else { Vec::new() };
        Self { kind, anchor, end: anchor, points, mode, constrain, snap }
    }
    /// How the shape joins existing coverage.
    pub(super) const fn mode(&self) -> CombineMode {
        self.mode
    }
    /// Move the dragged corner or extend the lasso. `constrain` squares rectangles and
    /// ellipses; `suppressed` (Ctrl) disables snapping.
    pub(super) fn update(
        &mut self,
        point: [f64; 2],
        scale: f64,
        constrain: bool,
        suppressed: bool,
    ) {
        self.constrain = constrain;
        self.end = match &mut self.snap {
            Some(snap) => snap.update(point, scale, suppressed, constrain),
            None => point,
        };
        if self.kind == ShapeKind::Lasso && self.points.last() != Some(&self.end) {
            if self.points.len() == MAX_LASSO_POINTS {
                self.points.pop();
            }
            self.points.push(self.end);
        }
    }
    /// Positive `[left, top, right, bottom]` spanned by the anchor and the dragged corner.
    fn rectangle(&self) -> Option<[f64; 4]> {
        let end =
            if self.constrain { super::snap::square(self.anchor, self.end) } else { self.end };
        let [left, right] = [self.anchor[0].min(end[0]), self.anchor[0].max(end[0])];
        let [top, bottom] = [self.anchor[1].min(end[1]), self.anchor[1].max(end[1])];
        (left < right && top < bottom).then_some([left, top, right, bottom])
    }
    /// The outline to draw while dragging, once the shape has area.
    pub(super) fn outline(&self) -> Option<OverlayShape<'_>> {
        match self.kind {
            ShapeKind::Rectangle => self.rectangle().map(OverlayShape::Rectangle),
            ShapeKind::Ellipse => self.rectangle().map(OverlayShape::Ellipse),
            ShapeKind::Lasso => {
                (self.points.len() >= 3).then_some(OverlayShape::Polygon(&self.points))
            }
        }
    }
    /// The finished shape, or `None` when it has no area.
    pub(super) fn geometry(&self) -> Option<ShapeGeometry> {
        match self.kind {
            ShapeKind::Rectangle => self.rectangle().map(ShapeGeometry::Rectangle),
            ShapeKind::Ellipse => self.rectangle().map(ShapeGeometry::Ellipse),
            ShapeKind::Lasso => {
                (self.points.len() >= 3).then(|| ShapeGeometry::Polygon(self.points.clone()))
            }
        }
    }
}
