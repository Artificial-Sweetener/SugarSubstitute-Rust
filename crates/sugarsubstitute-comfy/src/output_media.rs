// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Form a same-backend Comfy `/view` target from a shared, untrusted output reference.

use std::{error::Error, fmt};
use sugarsubstitute_contracts::canvas_surface::{CanvasFileReference, CanvasFileStorage};

#[cfg(windows)]
mod transfer;
#[cfg(windows)]
pub use transfer::{ComfyOutputTransfer, OUTPUT_TRANSFER_BYTES, OutputTransferError};

/// Maximum origin-form request target accepted by the owned networking adapter, in UTF-8 bytes.
const MAX_TARGET_BYTES: usize = 8192;

/// Encoded still-image format of one Comfy output, named by its filename extension.
///
/// The transfer enforces the matching content type, minimum length and signature; the image
/// decoder still validates the complete stream and rejects unsupported encodings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComfyOutputFormat {
    /// `.png` output served as `image/png`.
    Png,
    /// `.jpg` or `.jpeg` output served as `image/jpeg`.
    Jpeg,
}

impl ComfyOutputFormat {
    /// Classify a filename by its case-insensitive `.png`, `.jpg` or `.jpeg` extension.
    /// A leaf without a stem, such as `.png`, names no output format.
    pub fn from_filename(filename: &str) -> Option<Self> {
        let leaf = filename.rsplit(['/', '\\']).next()?;
        let (stem, extension) = leaf.rsplit_once('.')?;
        if stem.is_empty() {
            return None;
        }
        if extension.eq_ignore_ascii_case("png") {
            Some(Self::Png)
        } else if extension.eq_ignore_ascii_case("jpg") || extension.eq_ignore_ascii_case("jpeg") {
            Some(Self::Jpeg)
        } else {
            None
        }
    }

    /// The exact HTTP media type Comfy serves for this format.
    pub fn content_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
        }
    }

    /// Smallest encoded length that can hold a decodable image, in bytes.
    ///
    /// PNG needs its signature and complete `IHDR` chunk. JPEG needs `SOI`, the smallest
    /// single-component frame and scan headers, and `EOI`.
    pub fn minimum_bytes(self) -> u64 {
        match self {
            Self::Png => 33,
            Self::Jpeg => 27,
        }
    }

    /// Leading bytes every encoded stream of this format starts with.
    pub fn signature(self) -> &'static [u8] {
        match self {
            Self::Png => b"\x89PNG\r\n\x1a\n",
            Self::Jpeg => &[0xff, 0xd8, 0xff],
        }
    }
}

/// A relative Comfy `/view` target; it does not identify or authorize a server.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComfyViewTarget {
    target: String,
    format: ComfyOutputFormat,
}

impl ComfyViewTarget {
    /// Validate a shared reference and encode only the three supported Comfy query values.
    ///
    /// The caller must use its configured, independently qualified Comfy connection and
    /// recheck backend identity through the transfer. This value grants no filesystem access.
    /// # Errors
    /// Rejects unsafe or ambiguous paths, filenames without a supported image extension and
    /// targets exceeding the transport byte limit.
    pub fn new(reference: &CanvasFileReference) -> Result<Self, ComfyViewError> {
        let filename = relative(reference.filename(), false).ok_or(ComfyViewError::Filename)?;
        if filename.contains("..")
            || filename.ends_with('/')
            || ["[input]", "[output]", "[temp]"].iter().any(|suffix| filename.ends_with(suffix))
        {
            return Err(ComfyViewError::Filename);
        }
        let format = ComfyOutputFormat::from_filename(&filename).ok_or(ComfyViewError::Format)?;
        let subfolder = relative(reference.subfolder(), true).ok_or(ComfyViewError::Subfolder)?;
        let storage = match reference.storage() {
            CanvasFileStorage::Input => "input",
            CanvasFileStorage::Output => "output",
            CanvasFileStorage::Temporary => "temp",
        };
        let prefix = String::from("/view?");
        let prefix_length = prefix.len();
        let mut query = url::form_urlencoded::Serializer::for_suffix(prefix, prefix_length);
        query.append_pair("filename", &filename);
        query.append_pair("subfolder", &subfolder);
        query.append_pair("type", storage);
        let target = query.finish();
        if target.len() > MAX_TARGET_BYTES {
            return Err(ComfyViewError::TooLarge);
        }
        Ok(Self { target, format })
    }

    /// Borrow only the encoded origin-form target for the selected Comfy backend.
    pub fn as_str(&self) -> &str {
        &self.target
    }

    /// The encoded format the referenced filename names.
    pub fn format(&self) -> ComfyOutputFormat {
        self.format
    }
}

/// Explain why a reference cannot safely identify a Comfy media request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComfyViewError {
    /// Filename is empty, absolute, traversal-bearing, or carries a Comfy root annotation.
    Filename,
    /// Filename does not end in a supported `.png`, `.jpg` or `.jpeg` extension.
    Format,
    /// Subfolder is absolute, traversal-bearing, or contains unsupported characters.
    Subfolder,
    /// Percent encoding exceeds the native request-target limit.
    TooLarge,
}

impl fmt::Display for ComfyViewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Filename => formatter.write_str("Comfy output filename is invalid"),
            Self::Format => formatter.write_str("Comfy output format is unsupported"),
            Self::Subfolder => formatter.write_str("Comfy output subfolder is invalid"),
            Self::TooLarge => formatter.write_str("Comfy output request exceeds its byte bound"),
        }
    }
}

impl Error for ComfyViewError {}

/// Match SugarCubes' relative-path rules before encoding; Comfy still enforces its own boundary.
fn relative(value: &str, allow_empty: bool) -> Option<String> {
    if value.contains(':') || value.chars().any(|character| character < ' ') {
        return None;
    }
    let path = value.replace('\\', "/");
    if path.starts_with('/')
        || (!allow_empty && path.is_empty())
        || path.split('/').any(|part| matches!(part, "." | ".."))
    {
        return None;
    }
    Some(path)
}

#[cfg(test)]
mod tests;
