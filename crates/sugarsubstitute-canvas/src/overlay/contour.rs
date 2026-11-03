// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Closed outlines where coverage crosses 50 %, traced by marching squares.
//!
//! Samples sit at pixel centres and everything outside the coverage extent counts as uncovered,
//! so every contour closes. Crossings are interpolated linearly between neighbouring samples:
//! hard edges land exactly on pixel edges and anti-aliased edges keep their sub-pixel position.
//! Saddle cells are resolved by the mean of their four samples. Every contour runs clockwise on
//! screen around covered area, so holes run counter-clockwise.
use crate::CanvasError;
use std::collections::HashMap;
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
use sugarsubstitute_image_ops::{Coverage, Extent, FULL_COVERAGE, ImageError, Region};

/// Coverage at or above this value is inside an outline.
const INSIDE: u16 = FULL_COVERAGE / 2 + 1;
/// The exact 50 % level crossings are interpolated to.
const LEVEL: f64 = FULL_COVERAGE as f64 / 2.0;

/// One closed outline in document pixel coordinates; the last point connects to the first.
#[derive(Clone, Debug, PartialEq)]
pub struct Contour {
    points: Vec<[f32; 2]>,
}
impl Contour {
    /// Points in order around the outline.
    pub fn points(&self) -> &[[f32; 2]] {
        &self.points
    }
}

/// Every outline of one coverage, identified by that coverage's content fingerprint.
#[derive(Debug)]
pub struct CoverageContours {
    fingerprint: [u8; 32],
    extent: Extent,
    contours: Vec<Contour>,
    _memory: MemoryClaim,
}
impl CoverageContours {
    /// Trace a coverage's outlines inside an owned job; cost follows its covered bounds.
    /// # Errors
    /// Preserves cancellation, deadline and memory pressure.
    pub fn trace(coverage: &Coverage, context: &JobContext) -> Result<Self, CanvasError> {
        let contours = match coverage.bounds() {
            Some(bounds) => trace_region(coverage, bounds, context)?,
            None => Vec::new(),
        };
        let points: usize = contours.iter().map(|contour| contour.points.len()).sum();
        let bytes = (points * size_of::<[f32; 2]>() + contours.len() * size_of::<Contour>()) as u64;
        let memory = context.memory().claim(bytes + 128).map_err(ImageError::from)?;
        Ok(Self {
            fingerprint: coverage.fingerprint(),
            extent: coverage.extent(),
            contours,
            _memory: memory,
        })
    }
    /// Content fingerprint of the coverage these outlines trace.
    pub const fn fingerprint(&self) -> [u8; 32] {
        self.fingerprint
    }
    /// Extent of the traced coverage.
    pub const fn extent(&self) -> Extent {
        self.extent
    }
    /// Whether the traced coverage stays wholly below 50 %.
    pub fn is_empty(&self) -> bool {
        self.contours.is_empty()
    }
    /// Closed outlines, islands and holes alike.
    pub fn contours(&self) -> &[Contour] {
        &self.contours
    }
}

#[derive(Clone, Copy)]
enum Edge {
    Top,
    Right,
    Bottom,
    Left,
}

/// One directed crossing between two cell edges.
struct Segment {
    start: [f32; 2],
    end: u64,
    next: Option<usize>,
}

/// Lattice of pixel centres covering the bounds plus one uncovered sample on every side.
struct Lattice {
    left: i64,
    top: i64,
    width: usize,
}
impl Lattice {
    /// Key of the crossing on one edge of the cell whose top-left sample is `(x, y)`.
    fn key(&self, x: usize, y: usize, edge: Edge) -> u64 {
        let (x, y, vertical) = match edge {
            Edge::Top => (x, y, 0),
            Edge::Bottom => (x, y + 1, 0),
            Edge::Left => (x, y, 1),
            Edge::Right => (x + 1, y, 1),
        };
        ((y * self.width + x) as u64) << 1 | vertical
    }
    /// Document position of the 50 % crossing on one edge of a cell.
    fn point(&self, x: usize, y: usize, edge: Edge, corners: [u16; 4]) -> [f32; 2] {
        let [a, b, c, d] = corners.map(f64::from);
        let cross = |from: f64, to: f64| (LEVEL - from) / (to - from);
        let (px, py) = match edge {
            Edge::Top => (x as f64 + cross(a, b), y as f64),
            Edge::Right => (x as f64 + 1.0, y as f64 + cross(b, c)),
            Edge::Bottom => (x as f64 + cross(d, c), y as f64 + 1.0),
            Edge::Left => (x as f64, y as f64 + cross(a, d)),
        };
        [(px + (self.left as f64) + 0.5) as f32, (py + (self.top as f64) + 0.5) as f32]
    }
}

fn trace_region(
    coverage: &Coverage,
    bounds: Region,
    context: &JobContext,
) -> Result<Vec<Contour>, CanvasError> {
    let lattice = Lattice {
        left: i64::from(bounds.x()) - 1,
        top: i64::from(bounds.y()) - 1,
        width: bounds.width() as usize + 2,
    };
    let height = bounds.height() as usize + 2;
    let row_bytes = (lattice.width * size_of::<u16>()) as u64;
    let _rows = context.memory().claim(2 * row_bytes + 64).map_err(ImageError::from)?;
    let mut above = vec_of(lattice.width)?;
    let mut below = vec_of(lattice.width)?;
    let mut segments: Vec<Segment> = Vec::new();
    let mut starts: HashMap<u64, usize> = HashMap::new();
    let mut claims: Vec<MemoryClaim> = Vec::new();
    let mut claimed = 0_usize;
    for y in 0..height - 1 {
        check(context)?;
        if y == 0 {
            read_row(coverage, bounds, lattice.top, &mut above)?;
        }
        read_row(coverage, bounds, lattice.top + y as i64 + 1, &mut below)?;
        for x in 0..lattice.width - 1 {
            let corners = [above[x], above[x + 1], below[x + 1], below[x]];
            for (from, to) in cell_segments(corners) {
                let start = lattice.point(x, y, from, corners);
                let key = lattice.key(x, y, from);
                if segments.len() == claimed {
                    let chunk = 4096;
                    let bytes = chunk * (size_of::<Segment>() + 2 * size_of::<(u64, usize)>());
                    claims.push(context.memory().claim(bytes as u64).map_err(ImageError::from)?);
                    segments.try_reserve(chunk).map_err(ImageError::from)?;
                    starts.try_reserve(chunk).map_err(ImageError::from)?;
                    claimed += chunk;
                }
                starts.insert(key, segments.len());
                segments.push(Segment { start, end: lattice.key(x, y, to), next: None });
            }
        }
        std::mem::swap(&mut above, &mut below);
    }
    for segment in &mut segments {
        segment.next = starts.get(&segment.end).copied();
    }
    link(&segments, context)
}

/// Follow each unvisited segment around its loop.
fn link(segments: &[Segment], context: &JobContext) -> Result<Vec<Contour>, CanvasError> {
    let mut visited = Vec::new();
    visited.try_reserve_exact(segments.len()).map_err(ImageError::from)?;
    visited.resize(segments.len(), false);
    let mut contours = Vec::new();
    for first in 0..segments.len() {
        if visited[first] {
            continue;
        }
        check(context)?;
        let mut points = Vec::new();
        let mut current = first;
        loop {
            visited[current] = true;
            points.try_reserve(1).map_err(ImageError::from)?;
            points.push(segments[current].start);
            match segments[current].next {
                Some(next) if next == first => break,
                Some(next) if !visited[next] => current = next,
                _ => return Err(CanvasError::Invalid("Coverage contour did not close")),
            }
        }
        contours.try_reserve(1).map_err(ImageError::from)?;
        contours.push(Contour { points });
    }
    Ok(contours)
}

/// Directed segments of one cell with corners `[top-left, top-right, bottom-right,
/// bottom-left]`, each with covered samples on its right as seen on screen (y down).
fn cell_segments(corners: [u16; 4]) -> impl Iterator<Item = (Edge, Edge)> {
    let [a, b, c, d] = corners.map(|value| value >= INSIDE);
    let centre =
        corners.iter().map(|value| u32::from(*value)).sum::<u32>() >= 4 * u32::from(INSIDE);
    let pairs: [Option<(Edge, Edge)>; 2] = match (a, b, c, d) {
        (false, false, false, false) | (true, true, true, true) => [None, None],
        (true, false, false, false) => [Some((Edge::Top, Edge::Left)), None],
        (false, true, false, false) => [Some((Edge::Right, Edge::Top)), None],
        (false, false, true, false) => [Some((Edge::Bottom, Edge::Right)), None],
        (false, false, false, true) => [Some((Edge::Left, Edge::Bottom)), None],
        (true, true, false, false) => [Some((Edge::Right, Edge::Left)), None],
        (false, true, true, false) => [Some((Edge::Bottom, Edge::Top)), None],
        (false, false, true, true) => [Some((Edge::Left, Edge::Right)), None],
        (true, false, false, true) => [Some((Edge::Top, Edge::Bottom)), None],
        (false, true, true, true) => [Some((Edge::Left, Edge::Top)), None],
        (true, false, true, true) => [Some((Edge::Top, Edge::Right)), None],
        (true, true, false, true) => [Some((Edge::Right, Edge::Bottom)), None],
        (true, true, true, false) => [Some((Edge::Bottom, Edge::Left)), None],
        (true, false, true, false) if centre => {
            [Some((Edge::Top, Edge::Right)), Some((Edge::Bottom, Edge::Left))]
        }
        (true, false, true, false) => {
            [Some((Edge::Top, Edge::Left)), Some((Edge::Bottom, Edge::Right))]
        }
        (false, true, false, true) if centre => {
            [Some((Edge::Right, Edge::Top)), Some((Edge::Left, Edge::Bottom))]
        }
        (false, true, false, true) => {
            [Some((Edge::Left, Edge::Top)), Some((Edge::Right, Edge::Bottom))]
        }
    };
    pairs.into_iter().flatten()
}

/// Read one lattice row; samples outside the extent are uncovered.
fn read_row(
    coverage: &Coverage,
    bounds: Region,
    row: i64,
    output: &mut [u16],
) -> Result<(), CanvasError> {
    output.fill(0);
    let Ok(y) = u32::try_from(row) else {
        return Ok(());
    };
    if y < bounds.y() || y >= bounds.bottom() {
        return Ok(());
    }
    let region = Region::new(bounds.x(), y, bounds.width(), 1)?;
    coverage.read(region, &mut output[1..=bounds.width() as usize])?;
    Ok(())
}

fn vec_of(length: usize) -> Result<Vec<u16>, CanvasError> {
    let mut values = Vec::new();
    values.try_reserve_exact(length).map_err(ImageError::from)?;
    values.resize(length, 0);
    Ok(values)
}

fn check(context: &JobContext) -> Result<(), CanvasError> {
    if context.cancellation_requested() {
        return Err(ImageError::Cancelled.into());
    }
    if context.deadline().is_some_and(|deadline| context.now() >= deadline) {
        return Err(ImageError::Deadline.into());
    }
    Ok(())
}
