// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Preserve the source splash's front-edge-front card geometry independently of rendering.

/// Describe a logical or physical rectangle in the caller's coordinate system.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    /// Left edge in pixels or logical units.
    pub x: f64,
    /// Top edge in pixels or logical units.
    pub y: f64,
    /// Horizontal extent in the same units as the origin.
    pub width: f64,
    /// Vertical extent in the same units as the origin.
    pub height: f64,
}

impl Bounds {
    /// Reject empty or nonfinite geometry before allocating or painting a card.
    pub fn is_valid(self) -> bool {
        [self.x, self.y, self.width, self.height].iter().all(|value| value.is_finite())
            && self.width > 0.0
            && self.height > 0.0
    }
}

/// Identify one destination corner in the caller's coordinate system.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    /// Horizontal position.
    pub x: f64,
    /// Vertical position.
    pub y: f64,
}

/// Project the card with the Python splash's production perspective, bob, and settle values.
///
/// Return corners clockwise from the upper left. Progress is clamped to zero through one;
/// nonfinite progress means the initial front-facing frame. Nonnegative directions turn
/// right and negative directions turn left. The source formula produces a parallelogram,
/// so its Qt quadrilateral transform is an affine scale and shear.
pub fn projected_quad(bounds: Bounds, progress: f64, direction: i8) -> [Point; 4] {
    let progress = if progress.is_finite() { progress.clamp(0.0, 1.0) } else { 0.0 };
    let turn = (progress * std::f64::consts::PI).sin();
    let fraction = (progress * std::f64::consts::PI).cos().abs().max(0.055);
    let half_width = (bounds.width * fraction).max(2.0) / 2.0;
    let half_height = bounds.height / 2.0;
    let center_x = bounds.x + bounds.width / 2.0;
    let mut center_y = bounds.y + half_height - bounds.height * 0.035 * turn;
    if progress >= 0.82 {
        let local = (progress - 0.82) / 0.18;
        center_y +=
            bounds.height * 0.018 * (local * std::f64::consts::PI * 2.0).sin() * (1.0 - local);
    }
    let sign = if direction >= 0 { 1.0 } else { -1.0 };
    let slant = half_height * turn * 0.25 * 0.18 * sign;
    [
        Point { x: center_x - half_width, y: center_y - half_height + slant },
        Point { x: center_x + half_width, y: center_y - half_height - slant },
        Point { x: center_x + half_width, y: center_y + half_height - slant },
        Point { x: center_x - half_width, y: center_y + half_height + slant },
    ]
}

#[cfg(test)]
mod tests {
    use super::{Bounds, projected_quad};

    /// Exercise the Python projection's reference rectangle without renderer dependencies.
    fn bounds() -> Bounds {
        Bounds { x: 83.0, y: 7.0, width: 387.0, height: 386.0 }
    }

    /// Preserve the original front-facing card without a pixel-center translation.
    #[test]
    fn initial_card_matches_available_bounds() {
        let quad = projected_quad(bounds(), 0.0, 1);
        assert_eq!(quad[0].x, 83.0);
        assert_eq!(quad[0].y, 7.0);
        assert_eq!(quad[2].x, 470.0);
        assert_eq!(quad[2].y, 393.0);
    }

    /// Keep an edge-on card transformable and retain the exact source bob and slant.
    #[test]
    fn midpoint_keeps_minimum_width_and_changes_turn_direction() {
        let quad = projected_quad(bounds(), 0.5, 1);
        let opposite = projected_quad(bounds(), 0.5, -1);
        assert!((quad[1].x - quad[0].x - 21.285).abs() < 1e-9);
        assert!((quad[0].y - 2.175).abs() < 1e-9);
        assert!((quad[1].y + 15.195).abs() < 1e-9);
        assert_eq!(quad[0].y, opposite[1].y);
        assert_eq!(quad[1].y, opposite[0].y);
    }

    /// Match the source's damped landing instead of extending its airborne bob.
    #[test]
    fn landing_settle_reaches_source_displacement() {
        let quad = projected_quad(bounds(), 0.865, 1);
        let midpoint_y = (quad[0].y + quad[2].y) / 2.0;
        let turn = (0.865 * std::f64::consts::PI).sin();
        let expected = 200.0 - 386.0 * 0.035 * turn + 386.0 * 0.018 * 0.75;
        assert!((midpoint_y - expected).abs() < 1e-9);
    }

    /// Bound animation input and prevent malformed geometry from reaching the renderer.
    #[test]
    fn progress_and_geometry_have_explicit_boundaries() {
        assert_eq!(projected_quad(bounds(), -1.0, 1), projected_quad(bounds(), 0.0, 1));
        assert_eq!(projected_quad(bounds(), 2.0, 1), projected_quad(bounds(), 1.0, 1));
        assert_eq!(projected_quad(bounds(), f64::NAN, 1), projected_quad(bounds(), 0.0, 1));
        assert!(bounds().is_valid());
        assert!(!Bounds { width: 0.0, ..bounds() }.is_valid());
        assert!(!Bounds { x: f64::INFINITY, ..bounds() }.is_valid());
    }
}
