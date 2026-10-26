// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Toolkit-independent navigation observations and semantic feedback.

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
