// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coverage kept beyond the extent, so content moved off the document comes back when moved
//! back, as CuteCanvas keeps a moved layer's pixels under its layer offset.
//!
//! Reads, rendering, damage and capture see only the extent. Geometric edits work on the whole
//! content: they gather the extent and what lies beyond into one frame, edit it, and split the
//! result again, keeping whatever falls outside the extent. Content beyond is held as one
//! coverage cropped to exactly what it covers, uncovered wherever it overlaps the extent, so a
//! split result is canonical and persists and compares exactly.
use super::{CombineMode, Coverage};
use crate::{Extent, ImageError, Region, check};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{JobContext, MemoryLease};

/// Signed `[left, top, right, bottom]` pixel rectangle in a coverage's own coordinates.
pub(super) type Frame = [i64; 4];

/// Content beyond the extent: `frame` with its top left corner at `origin`.
#[derive(Debug)]
pub(super) struct Beyond {
    origin: [i64; 2],
    frame: Coverage,
}

impl Coverage {
    /// Content kept beyond the extent and the position of its top left corner, or `None` when
    /// nothing lies beyond. The returned coverage covers its whole extent's bounds and nothing
    /// where it overlaps this coverage's extent.
    pub fn outside(&self) -> Option<(&Coverage, [i64; 2])> {
        self.beyond.as_deref().map(|beyond| (&beyond.frame, beyond.origin))
    }

    /// The same coverage without the content kept beyond its extent, as a selection keeps only
    /// what lies on the document.
    pub fn without_outside(&self) -> Self {
        Self { beyond: None, ..self.clone() }
    }

    /// Attach content kept beyond the extent, as restoring a persisted mask does.
    /// # Errors
    /// Rejects content that is not canonical: a frame that itself keeps content beyond, does not
    /// cover its own bounds exactly, covers anything inside this extent, or replaces content
    /// already kept beyond.
    pub fn with_outside(self, frame: Coverage, origin: [i64; 2]) -> Result<Self, ImageError> {
        let canonical = self.beyond.is_none()
            && frame.beyond.is_none()
            && frame.bounds() == Some(frame.extent.region())
            && frame.default_value() == 0;
        let overlap = intersection(rect(frame.extent, origin), rect(self.extent, [0, 0]));
        let clear = overlap.is_none_or(|[left, top, right, bottom]| {
            let local = Region::new(
                (left - origin[0]) as u32,
                (top - origin[1]) as u32,
                (right - left) as u32,
                (bottom - top) as u32,
            );
            local.is_ok_and(|region| {
                (region.y()..region.bottom())
                    .all(|y| (region.x()..region.right()).all(|x| frame.value(x, y) == 0))
            })
        });
        if !canonical || !clear {
            return Err(ImageError::Invalid("Coverage beyond its extent is not canonical"));
        }
        Ok(Self { beyond: Some(Arc::new(Beyond { origin, frame })), ..self })
    }

    /// Signed `[left, top, right, bottom]` bounds of everything covered, inside the extent and
    /// beyond it, or `None` when nothing is.
    pub fn content_bounds(&self) -> Option<[i64; 4]> {
        let inside = self
            .bounds()
            .map(|bounds| [bounds.x(), bounds.y(), bounds.right(), bounds.bottom()].map(i64::from));
        let outside = self.outside().map(|(frame, origin)| rect(frame.extent, origin));
        union(inside, outside)
    }

    /// Whether every covered pixel lies inside `frame`, so an edit confined to it needs no
    /// content beyond the extent.
    pub(super) fn within(&self, frame: Frame) -> bool {
        self.beyond.is_none()
            && self
                .content_bounds()
                .is_none_or(|bounds| intersection(bounds, frame) == Some(bounds))
    }

    /// The whole content, inside the extent and beyond, as one coverage over `frame`.
    pub(super) fn whole_in(
        &self,
        frame: Frame,
        memory: &MemoryLease,
        context: &JobContext,
    ) -> Result<Self, ImageError> {
        if frame == rect(self.extent, [0, 0]) && self.beyond.is_none() {
            return Ok(self.clone());
        }
        let extent = extent_of(frame)?;
        let mut whole = self.place(extent, [-frame[0], -frame[1]], memory, context)?;
        if let Some((outside, origin)) = self.outside() {
            let placed = outside.place(
                extent,
                [origin[0] - frame[0], origin[1] - frame[1]],
                memory,
                context,
            )?;
            whole = whole.combine(&placed, CombineMode::Add, context)?;
        }
        Ok(whole)
    }

    /// Split whole content whose top left corner lies at `origin` into a coverage over `extent`
    /// and the content beyond it.
    pub(super) fn from_whole(
        extent: Extent,
        whole: &Self,
        origin: [i64; 2],
        memory: &MemoryLease,
        context: &JobContext,
    ) -> Result<Self, ImageError> {
        let inside = if whole.extent == extent && origin == [0, 0] {
            whole.without_outside()
        } else {
            whole.place(extent, origin, memory, context)?
        };
        // Content beyond is held over an uncovered default, so it covers only what it holds.
        let mut rest = if whole.default_value() == 0 {
            whole.without_outside()
        } else {
            whole.place(whole.extent, [0, 0], memory, context)?
        };
        let overlap = intersection(rect(extent, [0, 0]), rect(whole.extent, origin));
        if let Some([left, top, right, bottom]) = overlap {
            let region = Region::new(
                (left - origin[0]) as u32,
                (top - origin[1]) as u32,
                (right - left) as u32,
                (bottom - top) as u32,
            )?;
            rest.update_in_job(region, context, |_, _, _| 0)?;
        }
        check(context)?;
        let Some(bounds) = rest.bounds() else {
            return Ok(inside);
        };
        let frame = if bounds == rest.extent.region() {
            rest
        } else {
            let cropped = Extent::new(bounds.width(), bounds.height())?;
            rest.place(cropped, [-i64::from(bounds.x()), -i64::from(bounds.y())], memory, context)?
        };
        let origin = [origin[0] + i64::from(bounds.x()), origin[1] + i64::from(bounds.y())];
        Ok(Self { beyond: Some(Arc::new(Beyond { origin, frame })), ..inside })
    }

    /// Edit the whole content within `frame`, which must hold everything the edit can cover,
    /// and split the result again.
    pub(super) fn edit_whole(
        &self,
        frame: Frame,
        memory: &MemoryLease,
        context: &JobContext,
        edit: impl FnOnce(&Self) -> Result<Self, ImageError>,
    ) -> Result<Self, ImageError> {
        let whole = self.whole_in(frame, memory, context)?;
        let edited = edit(&whole)?;
        Self::from_whole(self.extent, &edited, [frame[0], frame[1]], memory, context)
    }
}

impl Coverage {
    /// Edit the whole content within its bounds grown by `margin` on every side.
    pub(super) fn grown_edit(
        &self,
        margin: u32,
        context: &JobContext,
        edit: impl FnOnce(&Self) -> Result<Self, ImageError>,
    ) -> Result<Self, ImageError> {
        let Some([left, top, right, bottom]) = self.content_bounds() else {
            return Ok(self.clone());
        };
        let margin = i64::from(margin);
        let frame = [left - margin, top - margin, right + margin, bottom + margin];
        self.edit_whole(frame, context.memory(), context, edit)
    }
}

/// The rectangle an extent covers with its top left corner at `origin`.
pub(super) fn rect(extent: Extent, [x, y]: [i64; 2]) -> Frame {
    [x, y, x + i64::from(extent.width()), y + i64::from(extent.height())]
}

/// The extent of a nonempty rectangle.
pub(super) fn extent_of([left, top, right, bottom]: Frame) -> Result<Extent, ImageError> {
    let size = |low: i64, high: i64| u32::try_from(high - low).ok();
    match (size(left, right), size(top, bottom)) {
        (Some(width), Some(height)) => Extent::new(width, height),
        _ => Err(ImageError::Invalid("Coverage frame is too large")),
    }
}

/// The smallest rectangle holding both.
pub(super) fn union(left: Option<Frame>, right: Option<Frame>) -> Option<Frame> {
    match (left, right) {
        (None, other) | (other, None) => other,
        (Some(a), Some(b)) => {
            Some([a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])])
        }
    }
}

/// The overlap of two rectangles, or `None` when they do not overlap.
pub(super) fn intersection(a: Frame, b: Frame) -> Option<Frame> {
    let overlap = [a[0].max(b[0]), a[1].max(b[1]), a[2].min(b[2]), a[3].min(b[3])];
    (overlap[0] < overlap[2] && overlap[1] < overlap[3]).then_some(overlap)
}
