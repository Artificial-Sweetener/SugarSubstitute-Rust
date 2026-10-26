// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepared canvas presentation independent of native window handles.
mod cpu;
mod gpu;
mod gpu_surface;
mod projection;
mod surface;
mod update;
pub use cpu::{CpuPreparation, CpuRenderer};
pub use gpu::{GpuRenderer, PreparedGpuUpdate};
pub use gpu_surface::{GpuFrame, GpuSurface};
pub use projection::{CpuProjection, ProjectionBuffer, ProjectionSamples};
pub use surface::{CpuSurface, PreparedUpdate, SurfaceWork};

/// Reject invalid prepared presentation without changing retained surface contents.
#[derive(Debug)]
pub enum RenderError {
    /// Invalid dimensions, identity, patch coverage or prepared pixel representation.
    Invalid(&'static str),
    /// A prepared update depends on a frame that is no longer presented.
    StaleBase,
    /// A newer or equal generation already owns the surface.
    StaleTarget,
    /// Image/memory/allocator error preserving its original cause.
    Image(sugarsubstitute_image_ops::ImageError),
    /// Document or camera compilation error preserving original image causes.
    Canvas(sugarsubstitute_canvas::CanvasError),
    /// Native GPU evaluation failure preserving driver, image and execution causes.
    Gpu(sugarsubstitute_image_gpu::GpuError),
}
impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message) => f.write_str(message),
            Self::StaleBase => f.write_str("Canvas repair base is no longer presented"),
            Self::StaleTarget => f.write_str("Canvas frame was superseded"),
            Self::Image(cause) => write!(f, "Canvas preparation: {cause}"),
            Self::Gpu(cause) => write!(f, "Canvas GPU preparation: {cause}"),
            Self::Canvas(cause) => write!(f, "Canvas document: {cause}"),
        }
    }
}
impl std::error::Error for RenderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Image(cause) => Some(cause),
            Self::Canvas(cause) => Some(cause),
            Self::Gpu(cause) => Some(cause),
            _ => None,
        }
    }
}
impl From<sugarsubstitute_image_ops::ImageError> for RenderError {
    fn from(cause: sugarsubstitute_image_ops::ImageError) -> Self {
        Self::Image(cause)
    }
}
impl From<sugarsubstitute_canvas::CanvasError> for RenderError {
    fn from(cause: sugarsubstitute_canvas::CanvasError) -> Self {
        Self::Canvas(cause)
    }
}

impl From<sugarsubstitute_image_gpu::GpuError> for RenderError {
    fn from(cause: sugarsubstitute_image_gpu::GpuError) -> Self {
        Self::Gpu(cause)
    }
}

#[cfg(test)]
mod tests;
