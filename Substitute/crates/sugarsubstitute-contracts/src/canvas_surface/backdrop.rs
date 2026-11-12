// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Describe bounded presentation-only frosted regions without granting native authority.

/// One schema-validated bar in CSS pixels relative to its canvas viewport's origin.
/// Native placement independently verifies density and clips it to the actual drawable extent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasSurfaceBackdrop {
    pub(super) bounds: [f64; 4],
    pub(super) corner_radius: f64,
    pub(super) sigma: f64,
}
impl CanvasSurfaceBackdrop {
    /// Viewport-relative `[left, top, width, height]` in CSS pixels.
    pub const fn bounds(self) -> [f64; 4] {
        self.bounds
    }
    /// Uniform outer corner radius in CSS pixels; presentation clamps it to the bar dimensions.
    pub const fn corner_radius(self) -> f64 {
        self.corner_radius
    }
    /// Gaussian standard deviation in CSS pixels, from zero through 64 inclusive.
    pub const fn sigma(self) -> f64 {
        self.sigma
    }
}
