// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Map a coverage from one boundary polygon onto another, as a resized shared edge reshapes a
//! mask layer.
//!
//! The mappings and their resampling follow CuteCanvas and its scene package:
//! - an affine map, resampled bilinearly as [`Coverage::transform`] does;
//! - a piecewise-affine map between two simple polygons of 4 to 128 vertices sharing one
//!   winding, triangulated by clipping the first ear valid in both, each target pixel centre
//!   taking the nearest source pixel through its triangle's inverse, later triangles winning;
//! - a bilinear map onto a quadrilateral whose first two corners meet, as a pivot that joins an
//!   edge leaves it, each target pixel centre taking the nearest source pixel.
//!
//! Coverage outside the mapped boundary is dropped, as CuteCanvas draws a reshaped layer only
//! inside it; content leaving the extent is kept beyond it.
use super::{
    Coverage, CoverageAffine,
    beyond::{Frame, extent_of},
};
use crate::{ImageError, Region, check};
use sugarsubstitute_contracts::execution::JobContext;

const GEOMETRY_EPSILON: f64 = 1e-10;
const MAX_VERTICES: usize = 128;

/// One triangle of a piecewise map and the affine map carrying it onto its target.
#[derive(Clone, Debug, PartialEq)]
struct Patch {
    target: [[f64; 2]; 3],
    affine: CoverageAffine,
}

/// A piecewise-affine map between two polygons.
#[derive(Clone, Debug, PartialEq)]
pub struct PiecewiseMapping {
    source: Vec<[f64; 2]>,
    target: Vec<[f64; 2]>,
    patches: Vec<Patch>,
}

/// A bilinear map from a source quadrilateral onto a target whose first two corners meet.
#[derive(Clone, Debug, PartialEq)]
pub struct BilinearMapping {
    source: [[f64; 2]; 4],
    target: [[f64; 2]; 4],
}

/// How a mask layer's original coverage maps onto the document.
#[derive(Clone, Debug, PartialEq)]
pub enum CoverageMapping {
    /// One affine map of the whole coverage.
    Affine(CoverageAffine),
    /// Triangles mapped affinely between two polygons.
    Piecewise(PiecewiseMapping),
    /// A quadrilateral mapped bilinearly onto a collapsed one.
    Bilinear(BilinearMapping),
}

/// The affine map carrying one triangle onto another, or `None` for a degenerate source.
pub fn triangle_affine(source: [[f64; 2]; 3], target: [[f64; 2]; 3]) -> Option<CoverageAffine> {
    let [s0, s1, s2] = source;
    let [t0, t1, t2] = target;
    let (u, v) = (sub(s1, s0), sub(s2, s0));
    let (tu, tv) = (sub(t1, t0), sub(t2, t0));
    let det = cross(u, v);
    if det.abs() <= epsilon(&source) {
        return None;
    }
    let m11 = (tu[0] * v[1] - tv[0] * u[1]) / det;
    let m21 = (tv[0] * u[0] - tu[0] * v[0]) / det;
    let m12 = (tu[1] * v[1] - tv[1] * u[1]) / det;
    let m22 = (tv[1] * u[0] - tu[1] * v[0]) / det;
    let dx = t0[0] - m11 * s0[0] - m21 * s0[1];
    let dy = t0[1] - m12 * s0[0] - m22 * s0[1];
    CoverageAffine::new([m11, m12, m21, m22, dx, dy]).ok()
}

impl PiecewiseMapping {
    /// Map `source` onto `target` vertex by vertex.
    /// # Errors
    /// Rejects mismatched or out-of-range vertex counts, zero-length edges, polygons that are
    /// not simple or enclose no area, opposite windings and polygons with no shared
    /// triangulation.
    pub fn new(source: &[[f64; 2]], target: &[[f64; 2]]) -> Result<Self, ImageError> {
        let invalid = |reason| Err(ImageError::Invalid(reason));
        if source.len() != target.len() || !(4..=MAX_VERTICES).contains(&source.len()) {
            return invalid("Piecewise boundaries need 4 to 128 matching vertices");
        }
        for polygon in [source, target] {
            let finite = polygon.iter().flatten().all(|value| value.is_finite());
            let zero = (0..polygon.len()).any(|i| polygon[i] == polygon[(i + 1) % polygon.len()]);
            if !finite || zero {
                return invalid("Piecewise boundaries need finite nonzero edges");
            }
        }
        let winding = simple_winding(source)?;
        if simple_winding(target)? != winding {
            return invalid("Piecewise boundaries must preserve winding");
        }
        let mut remaining: Vec<usize> = (0..source.len()).collect();
        let mut triangles = Vec::new();
        while remaining.len() > 3 {
            let ear = (0..remaining.len())
                .find(|position| shared_ear(source, target, &remaining, *position, winding))
                .ok_or(ImageError::Invalid("Piecewise boundaries share no triangulation"))?;
            let count = remaining.len();
            triangles.push([
                remaining[(ear + count - 1) % count],
                remaining[ear],
                remaining[(ear + 1) % count],
            ]);
            remaining.remove(ear);
        }
        triangles.push([remaining[0], remaining[1], remaining[2]]);
        let mut patches = Vec::new();
        for triangle in triangles {
            let source_triangle = triangle.map(|index| source[index]);
            let target_triangle = triangle.map(|index| target[index]);
            let affine = triangle_affine(source_triangle, target_triangle)
                .ok_or(ImageError::Invalid("Piecewise triangle is degenerate"))?;
            patches.push(Patch { target: target_triangle, affine });
        }
        Ok(Self { source: source.to_vec(), target: target.to_vec(), patches })
    }
    /// The source polygon.
    pub fn source(&self) -> &[[f64; 2]] {
        &self.source
    }
    /// The target polygon.
    pub fn target(&self) -> &[[f64; 2]] {
        &self.target
    }
    /// The source point mapped onto `point` by the first triangle whose target holds it.
    pub fn inverse_map(&self, point: [f64; 2]) -> Option<[f64; 2]> {
        let patch = self.patches.iter().find(|patch| contains(patch.target, point))?;
        Some(patch.affine.inverse()?.map(point))
    }
}

impl BilinearMapping {
    /// Map the `source` quadrilateral onto `target`, whose first two corners coincide.
    /// # Errors
    /// Rejects nonfinite corners, a target whose first corners differ and a target with no
    /// area.
    pub fn new(source: [[f64; 2]; 4], target: [[f64; 2]; 4]) -> Result<Self, ImageError> {
        let finite = source.iter().chain(&target).flatten().all(|value| value.is_finite());
        let det = cross(sub(target[3], target[0]), sub(target[2], target[0]));
        if !finite || target[0] != target[1] || det.abs() <= 1e-18 {
            return Err(ImageError::Invalid("Bilinear mapping needs a collapsed target quad"));
        }
        Ok(Self { source, target })
    }
    /// The source quadrilateral.
    pub const fn source(&self) -> [[f64; 2]; 4] {
        self.source
    }
    /// The target quadrilateral.
    pub const fn target(&self) -> [[f64; 2]; 4] {
        self.target
    }
    /// The source point mapped onto `point`, or `None` outside the target.
    pub fn inverse_map(&self, point: [f64; 2]) -> Option<[f64; 2]> {
        let apex = self.target[0];
        let (right, left) = (sub(self.target[2], apex), sub(self.target[3], apex));
        let det = cross(left, right);
        let relative = sub(point, apex);
        let (lw, rw) = (cross(relative, right) / det, cross(left, relative) / det);
        let v = lw + rw;
        if lw < -1e-9 || rw < -1e-9 || v > 1.0 + 1e-9 {
            return None;
        }
        let u = if v <= 1e-9 { 0.5 } else { (rw / v).clamp(0.0, 1.0) };
        Some(bilinear(self.source, u, v.clamp(0.0, 1.0)))
    }
    /// CuteCanvas's raster target inverse: the apex is the joined corner, `v` runs from it to
    /// the far edge and `u` across.
    fn parameters(&self, point: [f64; 2]) -> Option<[f64; 2]> {
        let apex = self.target[0];
        let (right, left) = (sub(self.target[2], apex), sub(self.target[3], apex));
        let det = left[0] * right[1] - left[1] * right[0];
        let relative = sub(point, apex);
        let v = (relative[0] * (right[1] - left[1]) + relative[1] * (left[0] - right[0])) / det;
        let u = (relative[1] * left[0] - relative[0] * left[1]) / det;
        if v > 1.0 || v <= 1e-12 || u < 0.0 || v - u < 0.0 {
            return None;
        }
        Some([(u / v).clamp(0.0, 1.0), v])
    }
}

impl CoverageMapping {
    /// The polygon the mapped coverage occupies, when the mapping carries one.
    pub fn target_boundary(&self) -> Option<&[[f64; 2]]> {
        match self {
            Self::Affine(_) => None,
            Self::Piecewise(mapping) => Some(&mapping.target),
            Self::Bilinear(mapping) => Some(&mapping.target),
        }
    }
    /// The source polygon matching [`Self::target_boundary`].
    pub fn source_boundary(&self) -> Option<&[[f64; 2]]> {
        match self {
            Self::Affine(_) => None,
            Self::Piecewise(mapping) => Some(&mapping.source),
            Self::Bilinear(mapping) => Some(&mapping.source),
        }
    }
    /// The source point a document point came from, or `None` outside the mapping.
    pub fn inverse_map(&self, point: [f64; 2]) -> Option<[f64; 2]> {
        match self {
            Self::Affine(affine) => Some(affine.inverse()?.map(point)),
            Self::Piecewise(mapping) => mapping.inverse_map(point),
            Self::Bilinear(mapping) => mapping.inverse_map(point),
        }
    }
}

impl Coverage {
    /// This coverage carried through `mapping`; see the module documentation for sampling.
    /// # Errors
    /// Preserves cancellation, deadline and memory pressure.
    pub fn remap(
        &self,
        mapping: &CoverageMapping,
        context: &JobContext,
    ) -> Result<Self, ImageError> {
        let target = match mapping {
            CoverageMapping::Affine(affine) => return self.transform(*affine, context),
            CoverageMapping::Piecewise(piecewise) => &piecewise.target[..],
            CoverageMapping::Bilinear(bilinear) => &bilinear.target[..],
        };
        let Some(content) = self.content_bounds() else {
            return Self::new(self.extent, 0, context.memory());
        };
        let source = self.whole_in(content, context.memory(), context)?;
        let frame = raster_bounds(target);
        let mut mapped = Self::new(extent_of(frame)?, 0, context.memory())?;
        let Some(bounds) = source.bounds() else {
            return Self::new(self.extent, 0, context.memory());
        };
        let _claim = context.memory().claim(bounds.pixels() * 2 + 64)?;
        let mut values = Vec::new();
        values.try_reserve_exact(bounds.pixels() as usize)?;
        values.resize(bounds.pixels() as usize, 0);
        source.read(bounds, &mut values)?;
        let sample = |[x, y]: [f64; 2]| -> u16 {
            let (column, row) = (x.floor() as i64 - content[0], y.floor() as i64 - content[1]);
            let (left, top) = (i64::from(bounds.x()), i64::from(bounds.y()));
            let inside = column >= left
                && row >= top
                && column < i64::from(bounds.right())
                && row < i64::from(bounds.bottom());
            if !inside {
                return 0;
            }
            values[(row - top) as usize * bounds.width() as usize + (column - left) as usize]
        };
        match mapping {
            CoverageMapping::Piecewise(piecewise) => {
                for patch in &piecewise.patches {
                    check(context)?;
                    let Some(inverse) = patch.affine.inverse() else {
                        continue;
                    };
                    let Some(region) = local(raster_bounds(&patch.target), frame) else {
                        continue;
                    };
                    let triangle = patch.target;
                    mapped.update_in_job(region, context, |x, y, existing| {
                        let center = [
                            (i64::from(x) + frame[0]) as f64 + 0.5,
                            (i64::from(y) + frame[1]) as f64 + 0.5,
                        ];
                        if barycentric_inside(triangle, center) {
                            sample(inverse.map(center))
                        } else {
                            existing
                        }
                    })?;
                }
            }
            CoverageMapping::Bilinear(bilinear) => {
                let region = mapped.extent.region();
                mapped.update_in_job(region, context, |x, y, _| {
                    let center = [
                        (i64::from(x) + frame[0]) as f64 + 0.5,
                        (i64::from(y) + frame[1]) as f64 + 0.5,
                    ];
                    bilinear
                        .parameters(center)
                        .map_or(0, |[u, v]| sample(self::bilinear(bilinear.source, u, v)))
                })?;
            }
            CoverageMapping::Affine(_) => {}
        }
        Self::from_whole(self.extent, &mapped, [frame[0], frame[1]], context.memory(), context)
    }
}

/// `P(u, v)` over a quadrilateral `q0 q1 q2 q3`: `u` along `q0 → q1`, `v` along `q0 → q3`.
fn bilinear(quad: [[f64; 2]; 4], u: f64, v: f64) -> [f64; 2] {
    let [q0, q1, q2, q3] = quad;
    [0, 1].map(|axis| {
        q0[axis]
            + u * (q1[axis] - q0[axis])
            + v * (q3[axis] - q0[axis])
            + u * v * (q0[axis] - q1[axis] - q3[axis] + q2[axis])
    })
}

/// The integer pixel bounds of a polygon, at least one pixel on each axis.
fn raster_bounds(points: &[[f64; 2]]) -> Frame {
    let low = |axis: usize| points.iter().map(|p| p[axis]).fold(f64::INFINITY, f64::min);
    let high = |axis: usize| points.iter().map(|p| p[axis]).fold(f64::NEG_INFINITY, f64::max);
    let (left, top) = (low(0).floor() as i64, low(1).floor() as i64);
    let (right, bottom) = (high(0).ceil() as i64, high(1).ceil() as i64);
    [left, top, right.max(left + 1), bottom.max(top + 1)]
}

/// `rect` inside `frame`, as a region in the frame's coordinates.
fn local(rect: Frame, frame: Frame) -> Option<Region> {
    let overlap = super::beyond::intersection(rect, frame)?;
    Region::new(
        (overlap[0] - frame[0]) as u32,
        (overlap[1] - frame[1]) as u32,
        (overlap[2] - overlap[0]) as u32,
        (overlap[3] - overlap[1]) as u32,
    )
    .ok()
}

/// CuteCanvas's patch membership: barycentric weights at least −1e-9 summing to at most
/// 1 + 1e-9.
fn barycentric_inside([first, second, third]: [[f64; 2]; 3], point: [f64; 2]) -> bool {
    let (a, b) = (sub(second, first), sub(third, first));
    let det = a[0] * b[1] - a[1] * b[0];
    if det.abs() <= 1e-18 {
        return false;
    }
    let relative = sub(point, first);
    let wa = (relative[0] * b[1] - relative[1] * b[0]) / det;
    let wb = (a[0] * relative[1] - a[1] * relative[0]) / det;
    wa >= -1e-9 && wb >= -1e-9 && wa + wb <= 1.0 + 1e-9
}

/// The polygon's winding, `1` or `-1`, once it is simple and encloses area.
fn simple_winding(points: &[[f64; 2]]) -> Result<i32, ImageError> {
    let area: f64 =
        (0..points.len()).map(|i| cross(points[i], points[(i + 1) % points.len()])).sum();
    let invalid = Err(ImageError::Invalid("Piecewise boundary is not simple"));
    if area.abs() <= epsilon(points) {
        return invalid;
    }
    let count = points.len();
    for index in 0..count {
        let (previous, vertex, next) =
            (points[(index + count - 1) % count], points[index], points[(index + 1) % count]);
        let (incoming, outgoing) = (sub(vertex, previous), sub(next, vertex));
        if cross(incoming, outgoing).abs() <= epsilon(&[previous, vertex, next])
            && incoming[0] * outgoing[0] + incoming[1] * outgoing[1] < 0.0
        {
            return invalid;
        }
    }
    for first in 0..count {
        let first_end = (first + 1) % count;
        for second in first + 1..count {
            let second_end = (second + 1) % count;
            let distinct = [first, first_end, second, second_end];
            if (0..4).any(|a| (a + 1..4).any(|b| distinct[a] == distinct[b])) {
                continue;
            }
            if segments_intersect(
                points[first],
                points[first_end],
                points[second],
                points[second_end],
            ) {
                return invalid;
            }
        }
    }
    Ok(if area > 0.0 { 1 } else { -1 })
}

fn shared_ear(
    source: &[[f64; 2]],
    target: &[[f64; 2]],
    remaining: &[usize],
    position: usize,
    winding: i32,
) -> bool {
    let count = remaining.len();
    let indexes = [
        remaining[(position + count - 1) % count],
        remaining[position],
        remaining[(position + 1) % count],
    ];
    let triangles = [indexes.map(|i| source[i]), indexes.map(|i| target[i])];
    if triangles.iter().any(|triangle| {
        f64::from(winding) * cross(sub(triangle[1], triangle[0]), sub(triangle[2], triangle[0]))
            <= epsilon(triangle)
    }) {
        return false;
    }
    !remaining.iter().filter(|index| !indexes.contains(index)).any(|index| {
        strictly_inside(triangles[0], source[*index])
            || strictly_inside(triangles[1], target[*index])
    })
}

fn signs(triangle: [[f64; 2]; 3], point: [f64; 2]) -> [f64; 3] {
    [0, 1, 2].map(|i| cross(sub(triangle[(i + 1) % 3], triangle[i]), sub(point, triangle[i])))
}

fn strictly_inside(triangle: [[f64; 2]; 3], point: [f64; 2]) -> bool {
    let tolerance = epsilon(&[triangle[0], triangle[1], triangle[2], point]);
    let signs = signs(triangle, point);
    signs.iter().all(|sign| *sign > tolerance) || signs.iter().all(|sign| *sign < -tolerance)
}

/// qpane's tolerant containment: on or inside the triangle.
fn contains(triangle: [[f64; 2]; 3], point: [f64; 2]) -> bool {
    let tolerance = epsilon(&[triangle[0], triangle[1], triangle[2], point]);
    let signs = signs(triangle, point);
    signs.iter().all(|sign| *sign >= -tolerance) || signs.iter().all(|sign| *sign <= tolerance)
}

fn segments_intersect(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let tolerance = epsilon(&[a, b, c, d]);
    let o = [
        cross(sub(b, a), sub(c, a)),
        cross(sub(b, a), sub(d, a)),
        cross(sub(d, c), sub(a, c)),
        cross(sub(d, c), sub(b, c)),
    ];
    if o[0] * o[1] < -(tolerance * tolerance) && o[2] * o[3] < -(tolerance * tolerance) {
        return true;
    }
    [(o[0], c, a, b), (o[1], d, a, b), (o[2], a, c, d), (o[3], b, c, d)].into_iter().any(
        |(orientation, point, start, end)| {
            orientation.abs() <= tolerance && on_segment(point, start, end)
        },
    )
}

fn on_segment(point: [f64; 2], start: [f64; 2], end: [f64; 2]) -> bool {
    let scale =
        [point, start, end].iter().flatten().fold(1.0_f64, |high, value| high.max(value.abs()));
    let tolerance = GEOMETRY_EPSILON * scale;
    (0..2).all(|axis| {
        start[axis].min(end[axis]) - tolerance <= point[axis]
            && point[axis] <= start[axis].max(end[axis]) + tolerance
    })
}

/// qpane's scaled epsilon: 1e-10 times the squared largest coordinate magnitude, at least one.
fn epsilon(points: &[[f64; 2]]) -> f64 {
    let scale = points.iter().flatten().fold(1.0_f64, |high, value| high.max(value.abs()));
    GEOMETRY_EPSILON * scale * scale
}

const fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

const fn cross(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
