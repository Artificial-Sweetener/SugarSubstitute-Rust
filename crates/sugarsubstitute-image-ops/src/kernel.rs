// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Share normalized reconstruction coefficients and global sample phase across evaluators.
use crate::{Filter, ImageError, Sampling};

/// Coordinate axis in the image sampling transform.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SamplingAxis {
    /// Source/output x coordinates.
    Horizontal,
    /// Source/output y coordinates.
    Vertical,
}
/// Maximum axis support for the bounded twofold Lanczos-3 pyramid reduction.
pub const MAX_SAMPLING_TAPS: usize = 13;
/// One normalized reconstruction weight at a clamped global source-pixel index.
#[derive(Clone, Copy, Debug, Default)]
pub struct SampleTap {
    pub(crate) coordinate: u32,
    pub(crate) weight: f32,
}
/// Bounded axis reconstruction in the source image's global pixel coordinates.
#[derive(Clone, Copy, Debug)]
pub struct AxisKernel {
    pub(crate) taps: [SampleTap; MAX_SAMPLING_TAPS],
    pub(crate) length: usize,
}
impl AxisKernel {
    pub(crate) fn new(
        output: i64,
        source: u32,
        ratio: f64,
        offset: f64,
    ) -> Result<Self, ImageError> {
        let scale = ratio.max(1.0);
        let center = pixel_center(output, ratio, offset) - 0.5;
        if source == 1
            || center + 3.0 * scale < 0.0
            || center - 3.0 * scale > f64::from(source - 1)
            || (scale == 1.0 && center.fract() == 0.0)
        {
            let mut taps = [SampleTap::default(); MAX_SAMPLING_TAPS];
            taps[0] = SampleTap {
                coordinate: center.clamp(0.0, f64::from(source - 1)) as u32,
                weight: 1.0,
            };
            return Ok(Self { taps, length: 1 });
        }
        let first = (center - 3.0 * scale).ceil() as i64;
        let last = (center + 3.0 * scale).floor() as i64;
        let length = (last - first + 1) as usize;
        if length > MAX_SAMPLING_TAPS {
            return Err(ImageError::Invalid("Resampling exceeds bounded Lanczos support"));
        }
        let mut taps = [SampleTap::default(); MAX_SAMPLING_TAPS];
        let mut total = 0.0;
        for (i, tap) in taps[..length].iter_mut().enumerate() {
            let position = first + i as i64;
            let weight = lanczos((position as f64 - center) / scale);
            *tap = SampleTap {
                coordinate: position.clamp(0, i64::from(source) - 1) as u32,
                weight: weight as f32,
            };
            total += weight;
        }
        if !total.is_finite() || total.abs() < f64::EPSILON {
            return Err(ImageError::Invalid("Invalid Lanczos normalization"));
        }
        for tap in &mut taps[..length] {
            tap.weight = (f64::from(tap.weight) / total) as f32;
        }
        Ok(Self { taps, length })
    }
}

fn lanczos(distance: f64) -> f64 {
    if distance.abs() < 1e-12 {
        return 1.0;
    }
    if distance.abs() >= 3.0 {
        return 0.0;
    }
    let angle = std::f64::consts::PI * distance;
    (angle.sin() / angle) * ((angle / 3.0).sin() / (angle / 3.0))
}

impl SampleTap {
    /// Zero-based global source index, including clamped edge duplicates.
    pub const fn coordinate(self) -> u32 {
        self.coordinate
    }
    /// Normalized linear reconstruction coefficient; Lanczos can have negative weights.
    pub const fn weight(self) -> f32 {
        self.weight
    }
}
impl AxisKernel {
    /// Ordered coefficients shared by CPU convolution and GPU upload.
    /// Keep the order and duplicate edge samples when accumulating floating-point values.
    pub fn taps(&self) -> &[SampleTap] {
        &self.taps[..self.length]
    }
}
impl Sampling {
    /// Prepare coefficients at one global output pixel center for either evaluator.
    /// Source length is in pixels; reconstruction clamps all taps to `[0, source_length)`.
    /// # Errors
    /// Rejects an empty source, overflowing sample coordinates or a reduction requiring pyramids.
    pub fn axis_kernel(
        self,
        axis: SamplingAxis,
        output: u32,
        source_length: u32,
    ) -> Result<AxisKernel, ImageError> {
        let axis = match axis {
            SamplingAxis::Horizontal => 0,
            SamplingAxis::Vertical => 1,
        };
        let ratio = self.scale()[axis];
        let offset = self.offset()[axis];
        if source_length == 0 || (source_length > 1 && ratio > 2.0) {
            return Err(ImageError::Invalid("Invalid bounded sampling source or reduction"));
        }
        if !self.center(axis, output).is_finite() {
            return Err(ImageError::Invalid("Image sampling coordinate overflow"));
        }
        match self.filter() {
            Filter::Bilinear => {
                let center =
                    (self.center(axis, output) - 0.5).clamp(0.0, f64::from(source_length - 1));
                let first = center.floor() as u32;
                let fraction = (center - f64::from(first)) as f32;
                let mut taps = [SampleTap::default(); MAX_SAMPLING_TAPS];
                taps[0] = SampleTap { coordinate: first, weight: 1.0 - fraction };
                let length = if fraction > 0.0 && first < source_length - 1 {
                    taps[1] = SampleTap { coordinate: first + 1, weight: fraction };
                    2
                } else {
                    1
                };
                Ok(AxisKernel { taps, length })
            }
            Filter::Lanczos3 => {
                AxisKernel::new(self.unshifted_index(axis, output), source_length, ratio, offset)
            }
            Filter::Nearest => {
                let mut taps = [SampleTap::default(); MAX_SAMPLING_TAPS];
                taps[0] = SampleTap {
                    coordinate: nearest_coordinate(
                        self.unshifted_index(axis, output),
                        source_length,
                        ratio,
                        offset,
                    ),
                    weight: 1.0,
                };
                Ok(AxisKernel { taps, length: 1 })
            }
        }
    }
}
pub(crate) fn pixel_center(output: i64, ratio: f64, offset: f64) -> f64 {
    (output as f64 + 0.5) * ratio + offset
}
pub(crate) fn nearest_coordinate(output: i64, source: u32, ratio: f64, offset: f64) -> u32 {
    pixel_center(output, ratio, offset).floor().clamp(0.0, f64::from(source - 1)) as u32
}
