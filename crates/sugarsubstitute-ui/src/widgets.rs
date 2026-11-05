// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fluent form decoration shared by native surfaces. Surface owners supply placement
//! and observed interaction state; this owner supplies every color and border rule.

use crate::accent::AccentPalette;
use crate::bitmap::{Bitmap, CoverageMask};
use crate::shapes::{AllocationError, CoverageRaster, blend_pixel, rounded};

/// Logical rectangle inside a surface's client area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    /// Logical left coordinate.
    pub x: f32,
    /// Logical top coordinate.
    pub y: f32,
    /// Positive logical width.
    pub width: f32,
    /// Positive logical height.
    pub height: f32,
}

impl Bounds {
    /// Validate finite positive logical dimensions and their bounded physical representation.
    /// # Errors
    /// Rejects invalid scale, nonfinite coordinates and dimensions exceeding the renderer limit.
    pub fn validate(self, scale: f32) -> Result<(), AllocationError> {
        if ![self.x, self.y, self.width, self.height, scale].iter().all(|v| v.is_finite())
            || self.width <= 0.0
            || self.height <= 0.0
            || !(0.5..=4.0).contains(&scale)
            || self.x * scale < -4096.0
            || self.y * scale < -4096.0
            || (self.x + self.width) * scale > 4096.0
            || (self.y + self.height) * scale > 4096.0
            || self.width * scale > 4096.0
            || self.height * scale > 4096.0
        {
            return Err(AllocationError::InvalidDimensions);
        }
        Ok(())
    }
}

/// Pointer/availability state of a form control.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ControlState {
    /// Enabled, with no active pointer gesture.
    #[default]
    Rest,
    /// The pointer is over an enabled control.
    Hover,
    /// An enabled control owns the press gesture.
    Pressed,
    /// The control is visible but cannot accept input.
    Disabled,
}

/// Supported native form treatments. Colors remain owned by this module; accent roles come
/// from the surface's [`AccentPalette`].
#[derive(Clone, Copy, Debug)]
pub enum Decoration {
    /// Ordinary or emphasized pushbutton.
    Button {
        /// Primary action uses the product accent.
        primary: bool,
        /// Actual input state.
        state: ControlState,
    },
    /// Text entry or a closed selection control.
    Field {
        /// Selection controls use a uniform low-contrast border.
        selection: bool,
        /// Text entry focus emphasizes its bottom edge.
        focused: bool,
        /// Actual input state.
        state: ControlState,
    },
    /// A grouped set of form fields.
    Section,
    /// Installation progress group with its original smaller corner radius.
    ProgressSection,
    /// Rounded icon background in a page heading or choice.
    IconBadge,
    /// A selectable card's hover treatment.
    Choice {
        /// Actual input state.
        state: ControlState,
    },
    /// The installation summary panel.
    Summary,
    /// Background around the native output control.
    Output,
    /// Installer body material wash.
    Body,
    /// Radio outline and selected accent ring; the native peer owns its group semantics.
    Radio {
        /// Actual application selection.
        checked: bool,
    },
    /// Check-box background; the bundled Accept icon supplies its selected mark.
    Check {
        /// Actual application selection.
        checked: bool,
    },
    /// Window close hover/press background, with its icon supplied by the caption owner.
    Close {
        /// Actual pointer and availability state.
        state: ControlState,
    },
}

/// Shared dark Fluent form foreground roles, independent of any page's text or layout.
#[derive(Clone, Copy, Debug)]
pub enum TextTone {
    /// Primary text and input values.
    Primary,
    /// Supporting descriptions.
    Secondary,
    /// Journey and subdued context.
    Tertiary,
    /// Actionable error details.
    Error,
    /// Button foreground resolved with its actual availability and pointer state.
    Action {
        /// Accent primary button.
        primary: bool,
        /// Actual state.
        state: ControlState,
    },
}
impl TextTone {
    /// Unpremultiplied foreground; raster coverage remains with the shared text renderer.
    pub fn color(self) -> [u8; 4] {
        match self {
            Self::Primary => [255; 4],
            Self::Secondary => [255, 255, 255, 189],
            Self::Tertiary => [255, 255, 255, 150],
            Self::Error => [255, 173, 191, 255],
            Self::Action { primary, state } => match (primary, state) {
                (true, ControlState::Disabled) => [255, 255, 255, 110],
                (false, ControlState::Disabled) => [255, 255, 255, 92],
                (_, ControlState::Pressed) => [255, 255, 255, 200],
                (true, _) => [0, 0, 0, 255],
                (false, _) => [255; 4],
            },
        }
    }
}

struct Palette {
    radius: f32,
    fill: [u8; 4],
    border: [u8; 4],
    top: [u8; 4],
    bottom: [u8; 4],
    insets: [f32; 4],
}

#[path = "widgets_palette.rs"]
mod palette;

enum Stage {
    Outer(CoverageRaster),
    Inner(CoverageRaster),
    Paint,
    Done,
}

/// One resumable form-decoration paint, with at most two target-sized coverage planes.
/// The caller accounts these planes in its preparation job's scratch reservation.
pub struct DecorationCursor {
    bounds: Bounds,
    palette: Palette,
    scale: f32,
    width: u32,
    height: u32,
    stage: Stage,
    outer: Option<CoverageMask>,
    inner: Option<CoverageMask>,
    position: usize,
}

impl DecorationCursor {
    /// Capture validated form geometry without initializing the coverage planes.
    /// The surface's accent supplies every emphasized role of the decoration.
    /// # Errors
    /// Rejects unsupported geometry and preserves allocation failures.
    pub fn new(
        width: u32,
        height: u32,
        bounds: Bounds,
        decoration: Decoration,
        accent: AccentPalette,
        scale: f32,
    ) -> Result<Self, AllocationError> {
        bounds.validate(scale)?;
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Err(AllocationError::InvalidDimensions);
        }
        let palette = decoration.palette(accent);
        let path = rounded(bounds.x, bounds.y, bounds.width, bounds.height, palette.radius, scale)
            .ok_or(AllocationError::InvalidDimensions)?;
        Ok(Self {
            bounds,
            palette,
            scale,
            width,
            height,
            stage: Stage::Outer(CoverageRaster::new(&path, width, height, true)?),
            outer: None,
            inner: None,
            position: 0,
        })
    }

    /// Advance at most 32,768 coverage or composition pixels on an admitted worker.
    /// # Errors
    /// Preserves allocation failures and rejects changed target dimensions before mutation.
    pub fn step(&mut self, target: &mut Bitmap) -> Result<bool, AllocationError> {
        if target.width() != self.width || target.height() != self.height {
            return Err(AllocationError::InvalidDimensions);
        }
        match &mut self.stage {
            Stage::Outer(raster) => {
                if !raster.step(32768) {
                    return Ok(false);
                }
                let Stage::Outer(raster) = std::mem::replace(&mut self.stage, Stage::Paint) else {
                    return Err(AllocationError::InvalidDimensions);
                };
                self.outer = Some(raster.finish()?);
                let [left, top, right, bottom] = self.palette.insets;
                if left + top + right + bottom > 0.0 {
                    let b = self.bounds;
                    if let Some(path) = rounded(
                        b.x + left,
                        b.y + top,
                        b.width - left - right,
                        b.height - top - bottom,
                        (self.palette.radius - left.max(top)).max(0.0),
                        self.scale,
                    ) {
                        self.stage = Stage::Inner(CoverageRaster::new(
                            &path,
                            self.width,
                            self.height,
                            true,
                        )?);
                    }
                }
            }
            Stage::Inner(raster) => {
                if !raster.step(32768) {
                    return Ok(false);
                }
                let Stage::Inner(raster) = std::mem::replace(&mut self.stage, Stage::Paint) else {
                    return Err(AllocationError::InvalidDimensions);
                };
                self.inner = Some(raster.finish()?);
            }
            Stage::Paint => self.paint(target)?,
            Stage::Done => return Ok(true),
        }
        Ok(false)
    }

    /// Composite each pixel once, retaining independently covered background and border.
    fn paint(&mut self, target: &mut Bitmap) -> Result<(), AllocationError> {
        let mask = self.outer.as_ref().ok_or(AllocationError::InvalidDimensions)?;
        let end = (self.position + 32768).min(mask.data().len());
        for index in self.position..end {
            let outer = mask.data()[index];
            if outer == 0 {
                continue;
            }
            let x = (index % self.width as usize) as i64;
            let y = (index / self.width as usize) as i64;
            blend_pixel(target, x, y, self.palette.fill, outer);
            if self.palette.insets.iter().all(|v| *v == 0.0) {
                continue;
            }
            let inner = self.inner.as_ref().map_or(0, |mask| mask.data()[index]);
            let border = outer.saturating_sub(inner);
            let lx = (x as f32 + 0.5) / self.scale - self.bounds.x;
            let ly = (y as f32 + 0.5) / self.scale - self.bounds.y;
            let side = lx.min(self.bounds.width - lx);
            let color = if ly <= side && ly <= self.bounds.height - ly {
                self.palette.top
            } else if self.bounds.height - ly <= side {
                self.palette.bottom
            } else {
                self.palette.border
            };
            blend_pixel(target, x, y, color, border);
        }
        self.position = end;
        if end == mask.data().len() {
            self.stage = Stage::Done;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "widgets_tests.rs"]
mod tests;
