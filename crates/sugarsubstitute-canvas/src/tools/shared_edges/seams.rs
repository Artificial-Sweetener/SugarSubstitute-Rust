// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Find the edges adjacent masks share, and move one without collapsing a mask.
//!
//! Each mask is framed by the bounds of its pixels at half coverage or more, as CuteCanvas finds
//! a mask's occupied area. Following CuteCanvas, two masks share an edge where their frames'
//! sides coincide within 0.25 device pixels and overlap by at least 8 device pixels, and shared
//! edges touching along one line join into one seam with every mask beside it. A press within
//! 6 device pixels of exactly one seam grips it; a drag never leaves a mask thinner than
//! 2 device pixels, or than it started when it was already thinner.
use crate::{CanvasError, OverlaySeam, Viewport};

const SEAM_TOLERANCE: f64 = 0.25;
const MIN_OVERLAP: f64 = 8.0;
const SEAM_REACH: f64 = 6.0;
const MIN_THICKNESS: f64 = 2.0;

/// Which way a seam runs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Axis {
    /// A vertical line at constant x, dragged left and right.
    Vertical,
    /// A horizontal line at constant y, dragged up and down.
    Horizontal,
}
impl Axis {
    /// Index of the coordinate a drag along this seam's normal changes.
    pub(super) const fn normal(self) -> usize {
        match self {
            Self::Vertical => 0,
            Self::Horizontal => 1,
        }
    }
}

/// One seam: the masks ending at it (`before`, left or above) and starting at it (`after`).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Seam {
    pub(super) axis: Axis,
    pub(super) position: f64,
    span: [f64; 2],
    before: Vec<usize>,
    after: Vec<usize>,
}
impl Seam {
    /// The seam's ends in document coordinates.
    pub(super) fn ends(&self) -> ([f64; 2], [f64; 2]) {
        match self.axis {
            Axis::Vertical => ([self.position, self.span[0]], [self.position, self.span[1]]),
            Axis::Horizontal => ([self.span[0], self.position], [self.span[1], self.position]),
        }
    }
    pub(super) fn overlay(&self, focused: bool) -> OverlaySeam {
        let (start, end) = self.ends();
        OverlaySeam { start, end, focused }
    }
    /// Frames after moving the seam by `delta` document pixels along its normal, clamped so no
    /// mask collapses, and the delta actually applied.
    pub(super) fn moved(&self, frames: &[[f64; 4]], delta: f64, zoom: f64) -> (Vec<[f64; 4]>, f64) {
        let near = self.axis.normal();
        let far = near + 2;
        let (mut low, mut high) = (f64::NEG_INFINITY, f64::INFINITY);
        for index in &self.before {
            let size = self.position - frames[*index][near];
            low = low.max(MIN_THICKNESS.min(size * zoom) / zoom - size);
        }
        for index in &self.after {
            let size = frames[*index][far] - self.position;
            high = high.min(size - MIN_THICKNESS.min(size * zoom) / zoom);
        }
        let delta = delta.clamp(low.ceil().min(0.0), high.floor().max(0.0));
        let mut moved = frames.to_vec();
        for index in &self.before {
            moved[*index][far] = self.position + delta;
        }
        for index in &self.after {
            moved[*index][near] = self.position + delta;
        }
        (moved, delta)
    }
}

/// Every seam between the frames `[left, top, right, bottom]` at this camera zoom.
pub(super) fn discover(frames: &[[f64; 4]], zoom: f64) -> Vec<Seam> {
    let mut seams: Vec<Seam> = Vec::new();
    for (first, a) in frames.iter().enumerate() {
        for (second, b) in frames.iter().enumerate() {
            for axis in [Axis::Vertical, Axis::Horizontal] {
                let (near, along) = (axis.normal(), 1 - axis.normal());
                if first == second || (a[near + 2] - b[near]).abs() * zoom > SEAM_TOLERANCE {
                    continue;
                }
                let span = [a[along].max(b[along]), a[along + 2].min(b[along + 2])];
                if (span[1] - span[0]) * zoom < MIN_OVERLAP {
                    continue;
                }
                let position = (a[near + 2] + b[near]) / 2.0;
                join(
                    &mut seams,
                    Seam { axis, position, span, before: vec![first], after: vec![second] },
                    zoom,
                );
            }
        }
    }
    seams
}

/// Merge a seam into one already found on the same line that it touches, or add it.
fn join(seams: &mut Vec<Seam>, mut seam: Seam, zoom: f64) {
    while let Some(index) = seams.iter().position(|other| {
        other.axis == seam.axis
            && (other.position - seam.position).abs() * zoom <= SEAM_TOLERANCE
            && other.span[0] <= seam.span[1]
            && seam.span[0] <= other.span[1]
    }) {
        let other = seams.swap_remove(index);
        seam.span = [seam.span[0].min(other.span[0]), seam.span[1].max(other.span[1])];
        for (target, source) in [(&mut seam.before, other.before), (&mut seam.after, other.after)] {
            for participant in source {
                if !target.contains(&participant) {
                    target.push(participant);
                }
            }
        }
    }
    seams.push(seam);
}

/// The one seam within reach of a device point, or `None` when none or several are.
pub(super) fn pick(
    seams: &[Seam],
    viewport: Viewport,
    device: [f64; 2],
) -> Result<Option<usize>, CanvasError> {
    let mut found = None;
    for (index, seam) in seams.iter().enumerate() {
        let (start, end) = seam.ends();
        let (start, end) = (viewport.device_at(start)?, viewport.device_at(end)?);
        if super::super::transform::segment_distance(start, end, device) <= SEAM_REACH {
            if found.is_some() {
                return Ok(None);
            }
            found = Some(index);
        }
    }
    Ok(found)
}
