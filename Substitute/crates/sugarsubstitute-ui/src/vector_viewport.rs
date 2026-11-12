// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve fractional device placement while bounding retained vector pixels.

use std::io;

/// Authored logical viewport, independent of the paths' visible ink bounds.
#[derive(Clone, Copy, Debug)]
pub struct VectorViewport {
    /// Logical left edge; negative placement supports clipped scrolling content.
    pub x: f32,
    /// Logical top edge.
    pub y: f32,
    /// Positive logical width.
    pub width: f32,
    /// Positive logical height.
    pub height: f32,
    /// Physical pixels per logical pixel, from one half through four.
    pub scale: f32,
}

/// Validated device geometry including the fractional phase within the first pixel.
pub(super) struct DeviceViewport {
    pub origin: [i32; 2],
    pub size: [u32; 2],
    pub phase: [f32; 2],
    pub extent: [f32; 2],
}

impl VectorViewport {
    /// Validate the bounded device viewport without allocating output pixels.
    pub(crate) fn validate(self) -> io::Result<()> {
        self.device().map(|_| ())
    }
    /// Bound both painted extent and fractional-edge storage before output admission.
    pub(super) fn device(self) -> io::Result<DeviceViewport> {
        let extent = [self.width * self.scale, self.height * self.scale];
        let location = [self.x * self.scale, self.y * self.scale];
        if ![self.x, self.y, self.width, self.height, self.scale].iter().all(|v| v.is_finite())
            || !(0.5..=4.0).contains(&self.scale)
            || extent.iter().any(|v| *v <= 0.0 || *v > 2048.0)
            || location.iter().zip(extent).any(|(v, size)| *v < -4096.0 || *v + size > 4096.0)
        {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid vector viewport"));
        }
        let origin = location.map(|v| v.floor() as i32);
        let phase = [location[0] - origin[0] as f32, location[1] - origin[1] as f32];
        let size = [(phase[0] + extent[0]).ceil() as u32, (phase[1] + extent[1]).ceil() as u32];
        Ok(DeviceViewport { origin, size, phase, extent })
    }
}
