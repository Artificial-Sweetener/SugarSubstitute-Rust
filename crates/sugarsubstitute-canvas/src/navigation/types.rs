// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Toolkit-independent navigation observations and camera commands.

use crate::Viewport;

/// Semantic camera operations independent of physical pointer and wheel gestures.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NavigationCommand {
    /// Center the complete image without cropping.
    Fit,
    /// Center the image at one document pixel per physical viewport pixel.
    OneToOne,
    /// Multiply the requested scale by a factor in the inclusive range 0.01 to 100.
    /// Preserve the document point at the viewport center and apply navigation scale bounds.
    Zoom(f64),
    /// Move content by fractions of the physical viewport width and height.
    /// Each component is in the inclusive range -10 to 10; positive moves right/down.
    Pan([f64; 2]),
    /// Show an image-relative camera immediately, without interpolation.
    ///
    /// center is the finite document point at the viewport center relative to the content
    /// extent; it may lie outside the image on either axis. span is normalized content distance per
    /// physical viewport pixel on each axis, greater than 0 and at most 1e9, so the camera is
    /// independent of viewport extent. The scale is contained across differing aspect ratios
    /// and validated by Viewport's numerical safety range, without interactive pan/zoom bounds.
    /// An actual centered fitted camera is Fit; another camera at that scale remains Custom.
    /// Nonfinite or unrepresentable transforms fail atomically, retaining the preceding camera.
    Frame {
        /// Normalized content point at the viewport center.
        center: [f64; 2],
        /// Normalized content distance per physical viewport pixel.
        span: [f64; 2],
    },
}

/// Admission of a semantic operation by the existing navigation authority.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NavigationCommandOutcome {
    /// The command owns this destination, possibly already displayed without further motion.
    Applied {
        /// Final camera after any bounded interpolation finishes.
        target: Viewport,
    },
    /// Current policy or an active physical gesture retains camera ownership.
    Denied,
    /// No image geometry is available for navigation.
    NoContent,
}

/// Meaning of the camera scale, independent of its current animated numerical value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ZoomMode {
    /// Refit and recenter when the drawable viewport changes.
    Fit,
    /// One document pixel per physical display pixel.
    OneToOne,
    /// User-selected scale with pointer-anchored reconstruction.
    Custom,
}
/// Platform gesture lifetime, distinct from individual wheel deltas.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScrollPhase {
    /// The device provides no lifetime; finish after 120 ms of inactivity.
    Unphased,
    /// Start an explicitly delimited gesture.
    Begin,
    /// Continue an explicitly delimited gesture.
    Update,
    /// Finish the gesture even when the final delta is zero.
    End,
}
/// Normalized navigation observations. Coordinates and pan deltas are physical device pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NavigationInput {
    /// Move the pointer; a captured drag moves the content by the same displacement.
    Pointer([f64; 2]),
    /// Begin/end direct panning at the last observed pointer location.
    Pan(bool),
    /// Hardware wheel notches, including fractional notches; these are not OS scroll lines.
    Wheel {
        /// Positive zooms in; negative zooms out.
        steps: f64,
        /// Device-provided gesture boundary.
        phase: ScrollPhase,
    },
    /// Toggle semantic Fit/OneToOne around this physical point.
    DoubleClick([f64; 2]),
    /// Transition to centered Fit.
    Fit,
    /// Transition to native scale around the last pointer position.
    OneToOne,
    /// End all transient motion and capture while preserving the current camera.
    Cancel,
}
