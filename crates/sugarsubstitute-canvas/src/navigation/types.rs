// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Toolkit-independent navigation observations and semantic feedback.

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
    /// center is the document point at the viewport center, normalized to the inclusive range
    /// 0 to 1 of the content on each axis. span is the normalized content distance per
    /// physical viewport pixel on each axis, greater than 0 and at most 1e9, so the camera is
    /// independent of viewport extent. The scale is contained across differing aspect ratios,
    /// bounded like zoom while keeping Fit reachable, and an exact Fit or one-to-one scale
    /// becomes that semantic mode; pan bounds apply as for every destination.
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
/// Native cursor appearance requested by the interaction owner.
/// This changes appearance only: physical pointer position remains owned by the operating system.
/// Hosts publish this feedback after input without waiting for image work or presentation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CursorIntent {
    /// Navigation cannot capture a pan gesture.
    Arrow,
    /// Navigation can capture a pan gesture.
    OpenHand,
    /// Navigation currently owns a pan gesture.
    ClosedHand,
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
