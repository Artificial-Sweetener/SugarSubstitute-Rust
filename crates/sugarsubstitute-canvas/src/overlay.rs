// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Presentation-only marks drawn over prepared canvas frames: the committed selection's marching
//! ants and the outline of a shape being drawn.
//!
//! Overlays are view feedback, never authored content. Their geometry is in document pixel
//! coordinates, so renderers project it through the presented camera and draw it on top of the
//! prepared frame; animating the ants only changes [`MarchingAnts`], never document pixels.
mod boundary;
mod outlines;

pub use boundary::{BoundaryRun, CoverageBoundary};
pub use outlines::{BoundaryWork, SelectionOutlines};

use std::{sync::Arc, time::Duration};
use sugarsubstitute_image_ops::Extent;

/// Light ant colour as sRGB bytes.
pub const ANT_LIGHT: [u8; 3] = [245, 245, 245];
/// Dark ant colour as sRGB bytes.
pub const ANT_DARK: [u8; 3] = [25, 25, 25];
/// Length of one light dash, and of one dark dash, in device pixels.
pub const ANT_DASH: f64 = 4.0;
/// Light dash length of a shape outline being drawn, in device pixels.
pub const OUTLINE_DASH: f64 = 8.0;
/// Dark gap length of a shape outline being drawn, in device pixels.
pub const OUTLINE_GAP: f64 = 4.0;

/// One shape outline in document pixel coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OverlayShape<'a> {
    /// Axis-aligned `[left, top, right, bottom]` rectangle.
    Rectangle([f64; 4]),
    /// Ellipse inscribed in a `[left, top, right, bottom]` rectangle.
    Ellipse([f64; 4]),
    /// Closed polygon through these points.
    Polygon(&'a [[f64; 2]]),
    /// Open segment from the first point to the second, such as the straight line a Shift-click
    /// brush stroke would paint.
    Line([[f64; 2]; 2]),
}

/// Transient feedback the effective tool asks presentation to draw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ToolOverlay<'a> {
    /// The shape a drag is drawing, as a dashed white-and-black outline.
    Outline(OverlayShape<'a>),
    /// The committed selection's ants displaced by whole document pixels while its outline is
    /// dragged.
    SelectionOffset([i64; 2]),
}

/// Animation state of the marching ants: which way the light and dark dashes are shifted.
///
/// Matching CuteCanvas, the dashes step one device pixel every 100 ms through an 8-pixel cycle.
/// Under reduced motion they stay still.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MarchingAnts {
    phase: u8,
}
impl MarchingAnts {
    /// Time between animation steps.
    pub const STEP: Duration = Duration::from_millis(100);
    /// Steps in one complete cycle.
    pub const STEPS: u8 = 8;
    /// The animation at `elapsed` since the overlay first appeared; still under reduced motion.
    pub fn at(elapsed: Duration, reduced_motion: bool) -> Self {
        if reduced_motion {
            return Self::default();
        }
        let steps = elapsed.as_millis() / Self::STEP.as_millis();
        Self { phase: (steps % u128::from(Self::STEPS)) as u8 }
    }
    /// The elapsed time of the next animation step after `elapsed`, or `None` under reduced
    /// motion; hosts present a new overlay frame then and not before.
    pub fn next_step(elapsed: Duration, reduced_motion: bool) -> Option<Duration> {
        if reduced_motion {
            return None;
        }
        let step = Self::STEP.as_nanos();
        let next = (elapsed.as_nanos() / step + 1) * step;
        Some(Duration::from_nanos(u64::try_from(next).unwrap_or(u64::MAX)))
    }
    /// Dash shift in device pixels, `0..8`.
    pub const fn phase(self) -> u8 {
        self.phase
    }
    /// Whether the light dash covers the point `distance` device pixels along a boundary run.
    pub fn light(self, distance: f64) -> bool {
        (distance + f64::from(self.phase)).rem_euclid(2.0 * ANT_DASH) < ANT_DASH
    }
}

/// Whether a drawn shape's outline is light at `distance` device pixels along it.
pub fn outline_light(distance: f64) -> bool {
    distance.rem_euclid(OUTLINE_DASH + OUTLINE_GAP) < OUTLINE_DASH
}

/// An owned shape outline that can travel with a presentation job.
#[derive(Clone, Debug, PartialEq)]
pub enum OverlayOutline {
    /// Axis-aligned `[left, top, right, bottom]` rectangle.
    Rectangle([f64; 4]),
    /// Ellipse inscribed in a `[left, top, right, bottom]` rectangle.
    Ellipse([f64; 4]),
    /// Closed polygon through these points.
    Polygon(Arc<[[f64; 2]]>),
    /// Open segment from the first point to the second.
    Line([[f64; 2]; 2]),
}
impl From<OverlayShape<'_>> for OverlayOutline {
    fn from(shape: OverlayShape<'_>) -> Self {
        match shape {
            OverlayShape::Rectangle(bounds) => Self::Rectangle(bounds),
            OverlayShape::Ellipse(bounds) => Self::Ellipse(bounds),
            OverlayShape::Polygon(points) => Self::Polygon(Arc::from(points)),
            OverlayShape::Line(points) => Self::Line(points),
        }
    }
}

/// Everything presentation draws over one frame, in document coordinates. It owns its geometry
/// so hosts can hand it to the job preparing the frame; boundaries are shared, not copied.
#[derive(Clone, Debug)]
pub struct PresentationOverlay {
    /// The committed or previewed selection's boundary, when it is traced.
    pub selection: Option<Arc<CoverageBoundary>>,
    /// Whole-pixel displacement applied to the selection boundary.
    pub selection_offset: [i64; 2],
    /// The outline of a shape being drawn.
    pub outline: Option<OverlayOutline>,
    /// Ant animation state.
    pub ants: MarchingAnts,
    /// Document extent; drawn outlines are clipped to it like the coverage they produce.
    pub document: Extent,
}
impl PresentationOverlay {
    /// Combine the displayed selection's boundary with the effective tool's feedback.
    pub fn new(
        document: Extent,
        selection: Option<Arc<CoverageBoundary>>,
        tool: Option<ToolOverlay<'_>>,
        ants: MarchingAnts,
    ) -> Self {
        let (outline, selection_offset) = match tool {
            Some(ToolOverlay::Outline(shape)) => (Some(shape.into()), [0, 0]),
            Some(ToolOverlay::SelectionOffset(offset)) => (None, offset),
            None => (None, [0, 0]),
        };
        Self { selection, selection_offset, outline, ants, document }
    }
    /// Whether nothing would be drawn.
    pub fn is_empty(&self) -> bool {
        self.outline.is_none() && !self.animated()
    }
    /// Whether the overlay changes over time, so hosts schedule [`MarchingAnts::next_step`].
    pub fn animated(&self) -> bool {
        self.selection.as_ref().is_some_and(|boundary| !boundary.is_empty())
    }
}

#[cfg(test)]
mod tests;
