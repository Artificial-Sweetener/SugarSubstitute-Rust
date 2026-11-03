// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transform box geometry: its handles, what a press grips, and the map every drag produces.
//!
//! The box is the transformed content's source rectangle in local coordinates under one affine
//! map. Following CuteCanvas, a press within 8 device pixels of one of the eight handles grips
//! it, a press inside the box moves it, and a press within 28 device pixels outside the box
//! rotates it about its centre. Corner handles scale proportionally unless Shift frees the
//! aspect ratio, side handles scale one axis, Ctrl+Shift on a side handle skews, Alt scales or
//! skews about the centre instead of the opposite handle, and Shift snaps rotation to 15°.
//! Moves are whole document pixels, so moved content stays exact.
use crate::tools::{ToolModifiers, TransformAction};
use crate::{CanvasError, CursorIntent, ResizeDirection, Viewport};
use sugarsubstitute_image_ops::CoverageAffine;

/// Device-pixel reach of a handle.
const HANDLE_REACH: f64 = 8.0;
/// Device-pixel band outside the box that rotates it.
const ROTATE_REACH: f64 = 28.0;
/// Rotation step while Shift is held.
const ROTATION_STEP: f64 = std::f64::consts::PI / 12.0;

/// One of the eight scale handles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Handle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}
impl Handle {
    const ALL: [Self; 8] = [
        Self::TopLeft,
        Self::Top,
        Self::TopRight,
        Self::Right,
        Self::BottomRight,
        Self::Bottom,
        Self::BottomLeft,
        Self::Left,
    ];
    /// Position across the box as fractions of its width and height.
    const fn fraction(self) -> [f64; 2] {
        match self {
            Self::TopLeft => [0.0, 0.0],
            Self::Top => [0.5, 0.0],
            Self::TopRight => [1.0, 0.0],
            Self::Right => [1.0, 0.5],
            Self::BottomRight => [1.0, 1.0],
            Self::Bottom => [0.5, 1.0],
            Self::BottomLeft => [0.0, 1.0],
            Self::Left => [0.0, 0.5],
        }
    }
    fn local(self, [left, top, right, bottom]: [f64; 4]) -> [f64; 2] {
        let [x, y] = self.fraction();
        [left + x * (right - left), top + y * (bottom - top)]
    }
    /// The handle across the box, which stays put while this one is dragged.
    fn opposite(self, rect: [f64; 4]) -> [f64; 2] {
        let [x, y] = self.fraction();
        let [left, top, right, bottom] = rect;
        [left + (1.0 - x) * (right - left), top + (1.0 - y) * (bottom - top)]
    }
    const fn corner(self) -> bool {
        matches!(self, Self::TopLeft | Self::TopRight | Self::BottomRight | Self::BottomLeft)
    }
    /// Whether this is the top or bottom side handle.
    const fn across(self) -> bool {
        matches!(self, Self::Top | Self::Bottom)
    }
}

/// What a press on the box takes hold of.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Grip {
    Scale(Handle),
    Skew(Handle),
    Move,
    Rotate,
}

/// The box: a local source rectangle `[left, top, right, bottom]` under an affine map.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Frame {
    pub(super) rect: [f64; 4],
    pub(super) affine: CoverageAffine,
}
impl Frame {
    /// Document corners, clockwise from the local top left.
    pub(super) fn corners(self) -> [[f64; 2]; 4] {
        [Handle::TopLeft, Handle::TopRight, Handle::BottomRight, Handle::BottomLeft]
            .map(|handle| self.affine.map(handle.local(self.rect)))
    }
    fn centre(self) -> [f64; 2] {
        let [left, top, right, bottom] = self.rect;
        [(left + right) / 2.0, (top + bottom) / 2.0]
    }
    /// What a press at a device point would grip.
    pub(super) fn grip(
        self,
        viewport: Viewport,
        device: [f64; 2],
        modifiers: ToolModifiers,
    ) -> Result<Option<Grip>, CanvasError> {
        let mut nearest: Option<(f64, Handle)> = None;
        for handle in Handle::ALL {
            let point = viewport.device_at(self.affine.map(handle.local(self.rect)))?;
            let distance = (point[0] - device[0]).hypot(point[1] - device[1]);
            if distance <= HANDLE_REACH && nearest.is_none_or(|(best, _)| distance < best) {
                nearest = Some((distance, handle));
            }
        }
        if let Some((_, handle)) = nearest {
            let skew = !handle.corner() && modifiers.ctrl && modifiers.shift;
            return Ok(Some(if skew { Grip::Skew(handle) } else { Grip::Scale(handle) }));
        }
        let mut polygon = [[0.0; 2]; 4];
        for (device, corner) in polygon.iter_mut().zip(self.corners()) {
            *device = viewport.device_at(corner)?;
        }
        if contains(&polygon, device) {
            return Ok(Some(Grip::Move));
        }
        Ok((distance(&polygon, device) <= ROTATE_REACH).then_some(Grip::Rotate))
    }
    /// The map a drag of `grip` from document point `origin` to `point` produces, or `None`
    /// when it would collapse the box.
    pub(super) fn drag(
        self,
        grip: Grip,
        origin: [f64; 2],
        point: [f64; 2],
        modifiers: ToolModifiers,
    ) -> Option<CoverageAffine> {
        match grip {
            Grip::Move => {
                let delta = [(point[0] - origin[0]).round(), (point[1] - origin[1]).round()];
                Some(self.affine.then(CoverageAffine::translation(delta)))
            }
            Grip::Rotate => {
                let centre = self.affine.map(self.centre());
                let angle = |[x, y]: [f64; 2]| (y - centre[1]).atan2(x - centre[0]);
                let mut turn = angle(point) - angle(origin);
                if modifiers.shift {
                    turn = (turn / ROTATION_STEP).round() * ROTATION_STEP;
                }
                Some(self.affine.then(rotation(centre, turn)?))
            }
            Grip::Scale(handle) | Grip::Skew(handle) => {
                let local = self.affine.inverse()?.map(point);
                let anchor = if modifiers.alt { self.centre() } else { handle.opposite(self.rect) };
                let reach = handle.local(self.rect);
                let local_map = if matches!(grip, Grip::Skew(_)) {
                    skew(handle, anchor, reach, local)?
                } else {
                    scale(handle, anchor, reach, local, modifiers.shift)?
                };
                Some(local_map.then(self.affine))
            }
        }
    }
    /// The map after a toolbar turn or flip about the box centre. The change is composed in
    /// local coordinates about one fixed local centre and snapped to the pixel lattice, so
    /// quarter turns and flips of untransformed content stay exact and opposite actions cancel.
    pub(super) fn turned(self, action: TransformAction) -> CoverageAffine {
        let Some(inverse) = self.affine.inverse() else {
            return self.affine;
        };
        let [x, y] = self.affine.map(self.centre());
        let change = match action {
            TransformAction::RotateLeft => rotation([x, y], -std::f64::consts::FRAC_PI_2),
            TransformAction::RotateRight => rotation([x, y], std::f64::consts::FRAC_PI_2),
            TransformAction::FlipHorizontal => {
                CoverageAffine::new([-1.0, 0.0, 0.0, 1.0, 2.0 * x, 0.0]).ok()
            }
            TransformAction::FlipVertical => {
                CoverageAffine::new([1.0, 0.0, 0.0, -1.0, 0.0, 2.0 * y]).ok()
            }
        };
        let Some(change) = change else {
            return self.affine;
        };
        self.affine.then(change).then(inverse).aligned().then(self.affine)
    }
    /// The cursor for a grip: resize arrows follow the handle's screen direction.
    pub(super) fn cursor(self, viewport: Viewport, grip: Grip) -> CursorIntent {
        let direction = |handle: Handle, across: bool| -> Option<ResizeDirection> {
            let centre = viewport.device_at(self.affine.map(self.centre())).ok()?;
            let point = viewport.device_at(self.affine.map(handle.local(self.rect))).ok()?;
            let [x, y] = [point[0] - centre[0], point[1] - centre[1]];
            Some(ResizeDirection::along(if across { [-y, x] } else { [x, y] }))
        };
        let resize = |handle, across| {
            direction(handle, across).map_or(CursorIntent::Arrow, CursorIntent::Resize)
        };
        match grip {
            Grip::Move => CursorIntent::Move,
            Grip::Rotate => CursorIntent::Rotate,
            Grip::Scale(handle) => resize(handle, false),
            Grip::Skew(handle) => resize(handle, true),
        }
    }
}

/// A turn by `angle` radians about `centre`, clockwise on screen for positive angles.
fn rotation([x, y]: [f64; 2], angle: f64) -> Option<CoverageAffine> {
    let (sin, cos) = angle.sin_cos();
    CoverageAffine::new([cos, sin, -sin, cos, x - (cos * x - sin * y), y - (sin * x + cos * y)])
        .ok()
}

/// A local scale about `anchor` carrying `reach` towards `point`.
fn scale(
    handle: Handle,
    anchor: [f64; 2],
    reach: [f64; 2],
    point: [f64; 2],
    free: bool,
) -> Option<CoverageAffine> {
    let span = [reach[0] - anchor[0], reach[1] - anchor[1]];
    let drag = [point[0] - anchor[0], point[1] - anchor[1]];
    let [sx, sy] = if handle.corner() && !free {
        let factor = (drag[0] * span[0] + drag[1] * span[1]) / (span[0].powi(2) + span[1].powi(2));
        [factor, factor]
    } else if handle.corner() {
        [drag[0] / span[0], drag[1] / span[1]]
    } else if handle.across() {
        [1.0, drag[1] / span[1]]
    } else {
        [drag[0] / span[0], 1.0]
    };
    CoverageAffine::new([sx, 0.0, 0.0, sy, anchor[0] * (1.0 - sx), anchor[1] * (1.0 - sy)]).ok()
}

/// A local shear about `anchor` carrying side handle `reach` along its side towards `point`.
fn skew(
    handle: Handle,
    anchor: [f64; 2],
    reach: [f64; 2],
    point: [f64; 2],
) -> Option<CoverageAffine> {
    let matrix = if handle.across() {
        let shear = (point[0] - reach[0]) / (reach[1] - anchor[1]);
        [1.0, 0.0, shear, 1.0, -shear * anchor[1], 0.0]
    } else {
        let shear = (point[1] - reach[1]) / (reach[0] - anchor[0]);
        [1.0, shear, 0.0, 1.0, 0.0, -shear * anchor[0]]
    };
    CoverageAffine::new(matrix).ok()
}

/// Whether a point lies inside a convex or concave polygon.
fn contains(polygon: &[[f64; 2]], [x, y]: [f64; 2]) -> bool {
    let mut inside = false;
    for (index, a) in polygon.iter().enumerate() {
        let b = polygon[(index + 1) % polygon.len()];
        if (a[1] > y) != (b[1] > y) && x < a[0] + (y - a[1]) * (b[0] - a[0]) / (b[1] - a[1]) {
            inside = !inside;
        }
    }
    inside
}

/// Distance from a point to a closed polygon's outline.
fn distance(polygon: &[[f64; 2]], point: [f64; 2]) -> f64 {
    (0..polygon.len())
        .map(|index| segment_distance(polygon[index], polygon[(index + 1) % polygon.len()], point))
        .fold(f64::INFINITY, f64::min)
}

/// Distance from `point` to the segment from `a` to `b`.
pub(in crate::tools) fn segment_distance(a: [f64; 2], b: [f64; 2], point: [f64; 2]) -> f64 {
    let along = [b[0] - a[0], b[1] - a[1]];
    let length = along[0].powi(2) + along[1].powi(2);
    let t = if length == 0.0 {
        0.0
    } else {
        (((point[0] - a[0]) * along[0] + (point[1] - a[1]) * along[1]) / length).clamp(0.0, 1.0)
    };
    (a[0] + t * along[0] - point[0]).hypot(a[1] + t * along[1] - point[1])
}
