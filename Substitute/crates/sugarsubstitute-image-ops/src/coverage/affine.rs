// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Map coverage through an affine transform, and place a moved fragment over coverage.
//!
//! Pixel `(x, y)` covers `[x, x + 1) × [y, y + 1)` and is sampled at its centre. Every target
//! pixel takes the bilinear sample of the source at its inverse-mapped centre, with everything
//! outside the source's covered bounds uncovered, as CuteCanvas resamples transformed selection
//! fragments with linear filtering and transparent edges. A map that carries pixel centres onto
//! pixel centres, such as a quarter turn, a flip or a whole-pixel move after
//! [`CoverageAffine::aligned`], lands every sample exactly on a source centre, so it permutes
//! values without blending and round trips exactly.
use super::{
    Coverage, FULL_COVERAGE,
    beyond::{self, Frame, extent_of, intersection, rect},
    multiply,
};
use crate::{ImageError, Region, check};
use sugarsubstitute_contracts::execution::JobContext;

/// Largest matrix entry error still treated as an exact quarter turn or flip.
const LATTICE_TOLERANCE: f64 = 1e-9;

/// Affine map of document points, `x' = m11·x + m21·y + dx` and `y' = m12·x + m22·y + dy`,
/// in the order Qt's `QTransform` stores it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoverageAffine {
    matrix: [f64; 6],
}

impl CoverageAffine {
    /// The map that leaves every point in place.
    pub const IDENTITY: Self = Self { matrix: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0] };

    /// A map from `[m11, m12, m21, m22, dx, dy]`.
    /// # Errors
    /// Rejects nonfinite entries and maps that collapse area, which have no inverse.
    pub fn new(matrix: [f64; 6]) -> Result<Self, ImageError> {
        let affine = Self { matrix };
        if !matrix.iter().all(|value| value.is_finite()) || affine.inverse().is_none() {
            return Err(ImageError::Invalid("Coverage transform is not invertible"));
        }
        Ok(affine)
    }

    /// A move by `[dx, dy]` document pixels.
    pub const fn translation([dx, dy]: [f64; 2]) -> Self {
        Self { matrix: [1.0, 0.0, 0.0, 1.0, dx, dy] }
    }

    /// `[m11, m12, m21, m22, dx, dy]`.
    pub const fn matrix(self) -> [f64; 6] {
        self.matrix
    }

    /// Map one document point.
    pub fn map(self, [x, y]: [f64; 2]) -> [f64; 2] {
        let [m11, m12, m21, m22, dx, dy] = self.matrix;
        [m11 * x + m21 * y + dx, m12 * x + m22 * y + dy]
    }

    /// This map followed by `next`.
    pub fn then(self, next: Self) -> Self {
        let [a11, a12, a21, a22, adx, ady] = self.matrix;
        let [b11, b12, b21, b22, bdx, bdy] = next.matrix;
        Self {
            matrix: [
                a11 * b11 + a12 * b21,
                a11 * b12 + a12 * b22,
                a21 * b11 + a22 * b21,
                a21 * b12 + a22 * b22,
                adx * b11 + ady * b21 + bdx,
                adx * b12 + ady * b22 + bdy,
            ],
        }
    }

    /// The map undoing this one, or `None` when it collapses area.
    pub fn inverse(self) -> Option<Self> {
        let [m11, m12, m21, m22, dx, dy] = self.matrix;
        let determinant = m11 * m22 - m12 * m21;
        if !determinant.is_finite() || determinant.abs() < 1e-12 {
            return None;
        }
        let [i11, i12, i21, i22] =
            [m22 / determinant, -m12 / determinant, -m21 / determinant, m11 / determinant];
        Some(Self { matrix: [i11, i12, i21, i22, -(dx * i11 + dy * i21), -(dx * i12 + dy * i22)] })
    }

    /// The same map snapped onto the pixel lattice when its linear part is a quarter turn or a
    /// flip: entries become exactly `-1`, `0` or `1`, and the translation moves by less than
    /// one pixel so pixel centres land on pixel centres. Any other map is returned unchanged.
    pub fn aligned(self) -> Self {
        let [m11, m12, m21, m22, dx, dy] = self.matrix;
        let snap = |value: f64| {
            let rounded = value.round();
            ((value - rounded).abs() <= LATTICE_TOLERANCE && rounded.abs() <= 1.0)
                .then_some(rounded)
        };
        let (Some(a), Some(b), Some(c), Some(d)) = (snap(m11), snap(m12), snap(m21), snap(m22))
        else {
            return self;
        };
        let permutation =
            (a.abs() + c.abs() == 1.0) && (b.abs() + d.abs() == 1.0) && a * d != b * c;
        if !permutation {
            return self;
        }
        // Centre (i + ½) maps to L·i + L·(½, ½) + t; choosing t = k + ½ − L·(½, ½) for an integer
        // k carries every centre onto a centre. A pivot half a pixel off the lattice makes k a
        // tie; clearing rounding noise first breaks every tie symmetrically, so a turn and its
        // opposite about the same pivot stay exact inverses.
        let lattice = |value: f64| {
            let halves = (value * 2.0).round();
            let value = if (value * 2.0 - halves).abs() <= LATTICE_TOLERANCE {
                halves / 2.0
            } else {
                value
            };
            value.round()
        };
        let half = [0.5 * (a + c), 0.5 * (b + d)];
        let k = [lattice(dx - 0.5 + half[0]), lattice(dy - 0.5 + half[1])];
        Self { matrix: [a, b, c, d, k[0] + 0.5 - half[0], k[1] + 0.5 - half[1]] }
    }

    /// Whether this map leaves every point in place.
    pub fn is_identity(self) -> bool {
        self == Self::IDENTITY
    }

    /// The signed pixel rectangle covering `frame` after mapping, grown by one pixel for the
    /// bilinear footprint.
    fn reach(self, [left, top, right, bottom]: Frame) -> Frame {
        let [left, top, right, bottom] = [left, top, right, bottom].map(|value| value as f64);
        let corners =
            [[left, top], [right, top], [right, bottom], [left, bottom]].map(|p| self.map(p));
        let low = |axis: usize| corners.iter().map(|p| p[axis]).fold(f64::INFINITY, f64::min);
        let high = |axis: usize| corners.iter().map(|p| p[axis]).fold(f64::NEG_INFINITY, f64::max);
        [
            low(0).floor() as i64 - 1,
            low(1).floor() as i64 - 1,
            high(0).ceil() as i64 + 1,
            high(1).ceil() as i64 + 1,
        ]
    }
}

impl Coverage {
    /// This coverage's whole content mapped through `affine` and resampled bilinearly. Content
    /// leaving the extent is kept beyond it; the area it uncovers is empty.
    /// # Errors
    /// Rejects a map without an inverse and preserves cancellation, deadline and memory pressure.
    pub fn transform(
        &self,
        affine: CoverageAffine,
        context: &JobContext,
    ) -> Result<Self, ImageError> {
        check(context)?;
        if affine.is_identity() {
            return Ok(self.clone());
        }
        let inverse =
            affine.inverse().ok_or(ImageError::Invalid("Coverage transform is not invertible"))?;
        let Some(content) = self.content_bounds() else {
            return Self::new(self.extent, 0, context.memory());
        };
        let document = rect(self.extent, [0, 0]);
        let reach = affine.reach(content);
        let inside = self.within(document);
        let (source, source_frame) = if inside {
            (self.clone(), document)
        } else {
            (self.whole_in(content, context.memory(), context)?, content)
        };
        if inside && intersection(reach, document) == Some(reach) {
            return source.sample(source_frame, document, reach, inverse, context);
        }
        let target = source.sample(source_frame, reach, reach, inverse, context)?;
        Self::from_whole(self.extent, &target, [reach[0], reach[1]], context.memory(), context)
    }

    /// Sample this coverage, whose top left lies at `source[..2]`, through `inverse` into a new
    /// coverage over the frame `target`, computing only the pixels inside `reach`.
    fn sample(
        &self,
        source: Frame,
        target: Frame,
        reach: Frame,
        inverse: CoverageAffine,
        context: &JobContext,
    ) -> Result<Self, ImageError> {
        let mut moved = Self::new(extent_of(target)?, 0, context.memory())?;
        let (Some(bounds), Some(computed)) = (self.bounds(), intersection(reach, target)) else {
            return Ok(moved);
        };
        let _claim = context.memory().claim(bounds.pixels() * 2 + 64)?;
        let mut values = Vec::new();
        values.try_reserve_exact(bounds.pixels() as usize)?;
        values.resize(bounds.pixels() as usize, 0);
        self.read(bounds, &mut values)?;
        check(context)?;
        let width = bounds.width() as usize;
        let at = |x: f64, y: f64| -> f64 {
            let (left, top) = (f64::from(bounds.x()), f64::from(bounds.y()));
            if x < left
                || y < top
                || x >= f64::from(bounds.right())
                || y >= f64::from(bounds.bottom())
            {
                return 0.0;
            }
            f64::from(values[(y - top) as usize * width + (x - left) as usize])
        };
        let region = Region::new(
            (computed[0] - target[0]) as u32,
            (computed[1] - target[1]) as u32,
            (computed[2] - computed[0]) as u32,
            (computed[3] - computed[1]) as u32,
        )?;
        let (shift, unshift) = ([target[0], target[1]], [source[0], source[1]]);
        moved.update_in_job(region, context, |x, y, _| {
            let point =
                [(i64::from(x) + shift[0]) as f64 + 0.5, (i64::from(y) + shift[1]) as f64 + 0.5];
            let [u, v] = inverse.map(point);
            let (u, v) = (u - unshift[0] as f64 - 0.5, v - unshift[1] as f64 - 0.5);
            let (x0, y0) = (u.floor(), v.floor());
            let (fx, fy) = (u - x0, v - y0);
            let sample = if fx == 0.0 && fy == 0.0 {
                at(x0, y0)
            } else {
                (1.0 - fy) * ((1.0 - fx) * at(x0, y0) + fx * at(x0 + 1.0, y0))
                    + fy * ((1.0 - fx) * at(x0, y0 + 1.0) + fx * at(x0 + 1.0, y0 + 1.0))
            };
            sample.round().clamp(0.0, f64::from(FULL_COVERAGE)) as u16
        })?;
        Ok(moved)
    }

    /// Place a moved fragment over this coverage: `self · (1 − alpha) + fragment`, where
    /// `fragment` is premultiplied by `alpha`, as a floating selection lands on its mask.
    /// # Errors
    /// Rejects different extents and preserves cancellation and memory pressure.
    pub fn over(
        &self,
        fragment: &Self,
        alpha: &Self,
        context: &JobContext,
    ) -> Result<Self, ImageError> {
        if fragment.extent != self.extent || alpha.extent != self.extent {
            return Err(ImageError::Invalid("Placed coverage extents differ"));
        }
        if self.beyond.is_some() || fragment.beyond.is_some() || alpha.beyond.is_some() {
            let bounds = [self, fragment, alpha].map(Self::content_bounds);
            let Some(frame) = bounds.into_iter().reduce(beyond::union).flatten() else {
                return Ok(self.clone());
            };
            let memory = context.memory();
            let [kept, fragment, alpha] =
                [self, fragment, alpha].map(|coverage| coverage.whole_in(frame, memory, context));
            let placed = kept?.over(&fragment?, &alpha?, context)?;
            return Self::from_whole(self.extent, &placed, [frame[0], frame[1]], memory, context);
        }
        let Some(region) = union(alpha.bounds(), fragment.bounds()) else {
            return Ok(self.clone());
        };
        let mut placed = self.clone();
        placed.update_in_job(region, context, |x, y, existing| {
            let kept = multiply(existing, FULL_COVERAGE - alpha.value(x, y));
            kept.saturating_add(fragment.value(x, y))
        })?;
        Ok(placed)
    }
}

/// The smallest region holding both regions.
fn union(left: Option<Region>, right: Option<Region>) -> Option<Region> {
    match (left, right) {
        (None, other) | (other, None) => other,
        (Some(a), Some(b)) => {
            let (x, y) = (a.x().min(b.x()), a.y().min(b.y()));
            let (right, bottom) = (a.right().max(b.right()), a.bottom().max(b.bottom()));
            Region::new(x, y, right - x, bottom - y).ok()
        }
    }
}
