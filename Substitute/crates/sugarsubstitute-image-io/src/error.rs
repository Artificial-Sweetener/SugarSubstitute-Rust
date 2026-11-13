// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve original decoder, storage, execution-admission and image-semantic failures.
use std::{error::Error, fmt, io};
use sugarsubstitute_image_ops::ImageError;

/// An image import or authenticated region request did not finish successfully.
#[derive(Debug)]
pub enum ImportError {
    /// Unsupported or malformed source/derived-format semantics.
    Invalid(&'static str),
    /// Another admitted import currently owns this retained encoded stream.
    Busy,
    /// Reconstructible cache bytes failed integrity/compatibility checks; originals are intact.
    Corrupt(Box<ImportError>),
    /// Original storage/stream error.
    Io(io::Error),
    /// Original PNG decoding or integrity error.
    Png(png::DecodingError),
    /// Original JPEG decoding or integrity error.
    Jpeg(zune_jpeg::errors::DecodeErrors),
    /// Original image, allocation or execution-admission error.
    Image(ImageError),
}
impl fmt::Display for ImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
            Self::Busy => formatter.write_str("Encoded image source is already in use"),
            Self::Corrupt(cause) => {
                write!(formatter, "Reconstructed image cache is invalid: {cause}")
            }
            Self::Io(cause) => write!(formatter, "Image input storage: {cause}"),
            Self::Png(cause) => write!(formatter, "PNG decoding: {cause}"),
            Self::Jpeg(cause) => write!(formatter, "JPEG decoding: {cause}"),
            Self::Image(cause) => write!(formatter, "Image import: {cause}"),
        }
    }
}
impl Error for ImportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Invalid(_) | Self::Busy => None,
            Self::Corrupt(cause) => Some(cause),
            Self::Io(cause) => Some(cause),
            Self::Png(cause) => Some(cause),
            Self::Jpeg(cause) => Some(cause),
            Self::Image(cause) => Some(cause),
        }
    }
}
impl From<io::Error> for ImportError {
    fn from(cause: io::Error) -> Self {
        Self::Io(cause)
    }
}
impl From<png::DecodingError> for ImportError {
    fn from(cause: png::DecodingError) -> Self {
        Self::Png(cause)
    }
}
impl From<zune_jpeg::errors::DecodeErrors> for ImportError {
    fn from(cause: zune_jpeg::errors::DecodeErrors) -> Self {
        Self::Jpeg(cause)
    }
}
impl From<ImageError> for ImportError {
    fn from(cause: ImageError) -> Self {
        Self::Image(cause)
    }
}
impl From<sugarsubstitute_contracts::execution::MemoryClaimError> for ImportError {
    fn from(cause: sugarsubstitute_contracts::execution::MemoryClaimError) -> Self {
        Self::Image(ImageError::from(cause))
    }
}
impl From<std::collections::TryReserveError> for ImportError {
    fn from(cause: std::collections::TryReserveError) -> Self {
        Self::Image(ImageError::from(cause))
    }
}
