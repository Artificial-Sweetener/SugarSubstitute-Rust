// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve canvas-domain errors and original image engine causes.
use std::{error::Error, fmt};
use sugarsubstitute_image_ops::ImageError;

/// Reject invalid document/view changes while preserving the preceding usable state.
#[derive(Debug)]
pub enum CanvasError {
    /// Invalid canvas input or an exhausted finite domain limit.
    Invalid(&'static str),
    /// Image operation failure with original memory, cancellation or source causes.
    Image(ImageError),
}
impl fmt::Display for CanvasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => f.write_str(message),
            Self::Image(cause) => write!(f, "Canvas image: {cause}"),
        }
    }
}
impl Error for CanvasError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Image(cause) => Some(cause),
            _ => None,
        }
    }
}
impl From<ImageError> for CanvasError {
    fn from(cause: ImageError) -> Self {
        Self::Image(cause)
    }
}
