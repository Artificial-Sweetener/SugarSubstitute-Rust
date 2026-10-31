// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Carry viewport intent and bounded image presentation state without native authority.

use super::{CanvasSurfaceMessageError, wire};

/// A schema-validated viewport intent decoded from an addressed surface request.
///
/// Zoom applies a relative multiplier. Pan offsets are fractions of the current physical
/// viewport extent, with positive x/y moving the image right/down. Native owners validate
/// attachment identity and revision before applying any intent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CanvasViewportCommand {
    /// Read the current presentation without changing it.
    Read,
    /// Fit the image inside the available viewport.
    Fit,
    /// Present one image pixel per physical display pixel.
    Native,
    /// Multiply the current physical scale by a factor from 0.01 through 100.
    Zoom {
        /// Relative scale multiplier, independent of CSS density.
        factor: f64,
    },
    /// Translate the image by viewport fractions from -10 through 10 on each axis.
    Pan {
        /// Horizontal fraction of the current physical viewport width.
        x: f64,
        /// Vertical fraction of the current physical viewport height.
        y: f64,
    },
    /// Move the comparison divider without changing the camera.
    Wipe {
        /// Axis the divider runs along.
        orientation: CanvasWipeOrientation,
        /// Leading viewport fraction, from 0 through 1, that reveals the comparison image.
        split: f64,
    },
}

/// Axis of a comparison wipe divider.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanvasWipeOrientation {
    /// A vertical divider; the comparison image is revealed on the left.
    Vertical,
    /// A horizontal divider; the comparison image is revealed on top.
    Horizontal,
}

/// The native owner's current viewport presentation policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanvasViewportMode {
    /// The image remains fitted to the viewport.
    Fit,
    /// The image uses one physical display pixel per image pixel.
    Native,
    /// Explicit magnification controls the image presentation.
    Custom,
}

/// A bounded native presentation snapshot for the currently displayed image.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasViewportState {
    mode: CanvasViewportMode,
    physical_scale: f64,
    width: u32,
    height: u32,
    secondary: Option<(u32, u32)>,
}

impl CanvasViewportCommand {
    /// Adopt command data only after the enclosing generated request passed validation.
    pub(super) fn from_wire(value: wire::CanvasSurfaceViewportCommand) -> Self {
        match value {
            wire::CanvasSurfaceViewportCommand::Read {} => Self::Read,
            wire::CanvasSurfaceViewportCommand::Fit {} => Self::Fit,
            wire::CanvasSurfaceViewportCommand::Native {} => Self::Native,
            wire::CanvasSurfaceViewportCommand::Zoom { factor } => Self::Zoom { factor },
            wire::CanvasSurfaceViewportCommand::Pan { x, y, .. } => Self::Pan { x, y },
            wire::CanvasSurfaceViewportCommand::Wipe { orientation, split } => Self::Wipe {
                orientation: if orientation == "horizontal" {
                    CanvasWipeOrientation::Horizontal
                } else {
                    CanvasWipeOrientation::Vertical
                },
                split,
            },
        }
    }
}

impl CanvasViewportState {
    /// Validate a snapshot before exposing it to the shared response encoder.
    ///
    /// `physical_scale` is physical display pixels per source image pixel, from 1e-9
    /// through 16384. Dimensions are source image pixels, from 1 through 2147483647.
    /// # Errors
    /// Rejects nonfinite or out-of-range scale and zero or out-of-range dimensions.
    pub fn new(
        mode: CanvasViewportMode,
        physical_scale: f64,
        width: u32,
        height: u32,
    ) -> Result<Self, CanvasSurfaceMessageError> {
        let value = Self { mode, physical_scale, width, height, secondary: None };
        if !value.to_wire().valid() {
            return Err(CanvasSurfaceMessageError::Invalid);
        }
        Ok(value)
    }

    /// Return the native owner's current presentation policy.
    pub const fn mode(&self) -> CanvasViewportMode {
        self.mode
    }

    /// Return physical display pixels per source image pixel.
    pub const fn physical_scale(&self) -> f64 {
        self.physical_scale
    }

    /// Return source image width and height in pixels.
    pub const fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Report a shown comparison image's own width and height in pixels.
    /// # Errors
    /// Rejects zero or out-of-range dimensions.
    pub fn with_secondary_dimensions(
        self,
        width: u32,
        height: u32,
    ) -> Result<Self, CanvasSurfaceMessageError> {
        let value = Self { secondary: Some((width, height)), ..self };
        if !value.to_wire().valid() {
            return Err(CanvasSurfaceMessageError::Invalid);
        }
        Ok(value)
    }

    /// Return the shown comparison image's width and height, if any.
    pub const fn secondary_dimensions(&self) -> Option<(u32, u32)> {
        self.secondary
    }

    /// Translate validated owned state to the exact shared response shape.
    pub(super) fn to_wire(self) -> wire::CanvasSurfaceViewportState {
        wire::CanvasSurfaceViewportState {
            mode: match self.mode {
                CanvasViewportMode::Fit => "fit",
                CanvasViewportMode::Native => "native",
                CanvasViewportMode::Custom => "custom",
            }
            .into(),
            physical_scale: self.physical_scale,
            dimensions: wire::CanvasSurfaceImageDimensions {
                width: u64::from(self.width),
                height: u64::from(self.height),
            },
            secondary_dimensions: self.secondary.map(|(width, height)| {
                wire::CanvasSurfaceImageDimensions {
                    width: u64::from(width),
                    height: u64::from(height),
                }
            }),
        }
    }
}

#[cfg(test)]
mod tests;
