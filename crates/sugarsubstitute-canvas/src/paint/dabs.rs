// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Place brush dabs along stroke segments the way CuteCanvas's dab engine does.
//!
//! A segment joins two contacts. Dabs are spaced at a fifth of the smaller contact diameter, at
//! least half a pixel apart; the segment is divided into equal steps no longer than that spacing,
//! and the diameter is interpolated along it. A stroke's first segment includes its start dab;
//! every later segment starts one step in, since the previous segment already placed a dab there.

/// Dab spacing as a fraction of the smaller diameter of a segment's two contacts.
pub(crate) const SPACING: f64 = 0.2;
/// Smallest pen-pressure diameter, as a fraction of the brush size.
pub(crate) const MINIMUM_PRESSURE_RATIO: f64 = 0.15;

/// One pointer contact in document pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Contact {
    /// Document position; pixel `(x, y)` covers `[x, x + 1) × [y, y + 1)`.
    pub(crate) position: [f64; 2],
    /// Brush diameter at this contact, at least one pixel.
    pub(crate) diameter: f64,
}

impl Contact {
    /// A contact whose diameter is at least one pixel.
    pub(crate) fn new(position: [f64; 2], diameter: f64) -> Self {
        Self { position, diameter: diameter.max(1.0) }
    }
}

/// One circular footprint to composite.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Dab {
    /// Centre in document pixels.
    pub(crate) center: [f64; 2],
    /// Diameter in document pixels, at least one.
    pub(crate) diameter: f64,
}

/// The diameter a pen contact paints with: pressure scales the brush size linearly from
/// [`MINIMUM_PRESSURE_RATIO`] at no pressure to the full size at full pressure.
pub(crate) fn pressure_diameter(size: f64, pressure: f64) -> f64 {
    let pressure = pressure.clamp(0.0, 1.0);
    size * (MINIMUM_PRESSURE_RATIO + (1.0 - MINIMUM_PRESSURE_RATIO) * pressure)
}

/// The ordered dabs of one segment. `continuation` omits the start dab, which the preceding
/// segment of the same stroke already placed. A zero-length segment places one dab at its
/// start, as CuteCanvas does when only the diameter changed.
pub(crate) fn segment(start: Contact, end: Contact, continuation: bool) -> SegmentDabs {
    let delta = [end.position[0] - start.position[0], end.position[1] - start.position[1]];
    let distance = delta[0].hypot(delta[1]);
    let spacing = (start.diameter.min(end.diameter).max(1.0) * SPACING).max(0.5);
    let (steps, first) = if distance == 0.0 {
        (1, 0)
    } else {
        ((distance / spacing).ceil().max(1.0) as u64, u64::from(continuation))
    };
    let last = if distance == 0.0 { 0 } else { steps };
    SegmentDabs { start, end, delta, steps, next: first, last }
}

/// Iterator over one segment's dabs, start to end.
#[derive(Clone, Debug)]
pub(crate) struct SegmentDabs {
    start: Contact,
    end: Contact,
    delta: [f64; 2],
    steps: u64,
    next: u64,
    last: u64,
}

impl Iterator for SegmentDabs {
    type Item = Dab;

    fn next(&mut self) -> Option<Dab> {
        if self.next > self.last {
            return None;
        }
        let fraction = self.next as f64 / self.steps as f64;
        self.next += 1;
        Some(Dab {
            center: [
                self.start.position[0] + self.delta[0] * fraction,
                self.start.position[1] + self.delta[1] * fraction,
            ],
            diameter: (self.start.diameter + (self.end.diameter - self.start.diameter) * fraction)
                .max(1.0),
        })
    }
}
