// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve the original worker failure through standalone viewer composition.
use std::{error::Error, fmt};

/// Actual worker failure without passing thread-affine desktop objects between workers.
#[derive(Debug)]
pub(super) struct WorkError(Box<dyn Error + Send + Sync>);
impl WorkError {
    /// Retain an original typed worker cause.
    pub(super) fn new(cause: impl Error + Send + Sync + 'static) -> Self {
        Self(Box::new(cause))
    }
    /// Reject invalid application work state without a runtime panic.
    pub(super) fn invalid(message: &'static str) -> Self {
        Self::new(std::io::Error::other(message))
    }
}
impl fmt::Display for WorkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}
impl Error for WorkError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.0.as_ref())
    }
}
macro_rules! causes {
    ($($cause:ty),+ $(,)?) => {$ (
        impl From<$cause> for WorkError {
            fn from(cause: $cause) -> Self { Self::new(cause) }
        }
    )+ };
}
causes!(
    std::io::Error,
    sugarsubstitute_image_ops::ImageError,
    sugarsubstitute_canvas::CanvasError,
    sugarsubstitute_canvas_render::RenderError,
    sugarsubstitute_image_io::ImportError,
    sugarsubstitute_contracts::execution::MemoryClaimError,
    sugarsubstitute_contracts::execution::IdentityError,
);
