// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exchange the native input region and routed gestures between the canvas and desktop owners.
//!
//! The canvas owner publishes where native navigation may receive pointer input; the desktop
//! adapter routes Win32 mouse messages against that region on the UI thread and returns only
//! the gestures it assigned to the canvas. Neither direction waits on the other.

use super::placement::{CanvasPlacement, PhysicalRect, WebViewAllocation};

/// Maximum occlusion rectangles in one published input region, matching the shared contract.
pub const MAX_INPUT_OCCLUSIONS: usize = 64;

/// Where the committed final image accepts native gestures, in physical client pixels.
///
/// The desktop routes into `placement.clip` minus every occlusion, and only while the native
/// tree still shows pixels at `placement` for `identity` and the current WebView allocation
/// equals `allocation`. Any mismatch withdraws the region, so stale state routes to WebView2.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MainCanvasInputRegion<'a> {
    /// Native main lifetime whose canvas published this region.
    pub identity: crate::MainIdentity,
    /// Acknowledged native placement of the shown final image.
    pub placement: CanvasPlacement,
    /// WebView allocation that qualified the placement and occlusion projection.
    pub allocation: WebViewAllocation,
    /// Page-published web content above the viewport, rounded outward; at most
    /// [`MAX_INPUT_OCCLUSIONS`] rectangles.
    pub occlusions: &'a [PhysicalRect],
}

/// Pointer button that may drive native navigation; every other button stays with WebView2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MainCanvasButton {
    /// The primary (usually left) button.
    Primary,
    /// The middle button or wheel press.
    Middle,
}

/// One routed observation, in the order the desktop received its Win32 message.
///
/// Positions are physical client pixels of the main window. The canvas owner converts them to
/// its viewport and applies them to the navigation tool.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MainCanvasGesture {
    /// The pointer moved inside the region or during a captured native gesture.
    Pointer([f64; 2]),
    /// A routed button changed state at the most recent pointer position.
    Button {
        /// The changed button.
        button: MainCanvasButton,
        /// Whether the button is now held.
        pressed: bool,
    },
    /// Signed hardware wheel notches at the most recent pointer position; positive zooms in.
    Wheel(f64),
    /// The OS double-click rule matched two primary presses at this position.
    DoubleClick([f64; 2]),
    /// Capture or activation was lost, or the region was withdrawn; end any native gesture.
    Cancel,
}

/// A routed gesture for the exact main lifetime whose region admitted it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MainCanvasInput {
    /// Native main lifetime; the canvas owner rejects any other identity.
    pub identity: crate::MainIdentity,
    /// Routed observation in physical client pixels.
    pub gesture: MainCanvasGesture,
}
