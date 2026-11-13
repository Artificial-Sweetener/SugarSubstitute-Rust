// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Form a same-backend Comfy `/view` target from a shared, untrusted output reference.

mod animation;
mod format;
mod identity;
pub use animation::{OutputAnimationError, probe_output_animation};
pub use format::ComfyOutputFormat;
pub use identity::{OutputArtifactIdentity, OutputArtifactIdentityError};

use std::{error::Error, fmt};
use sugarsubstitute_contracts::canvas_surface::{CanvasFileReference, CanvasFileStorage};

#[cfg(windows)]
mod encoded_copy;
#[cfg(windows)]
mod retained;
#[cfg(windows)]
mod transfer;
#[cfg(windows)]
pub use retained::{RETAINED_ORIGINAL_BYTES, retain_original};
#[cfg(windows)]
pub use transfer::{ComfyOutputTransfer, OUTPUT_TRANSFER_BYTES, OutputTransferError};

/// Maximum origin-form request target accepted by the owned networking adapter, in UTF-8 bytes.
const MAX_TARGET_BYTES: usize = 8192;

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
        Self::build(reference, false)
    }

    /// Address original supported media for authenticated durable retention, independently of rendering.
    /// The caller must still supply a current independently authorized origin and peer to its transfer.
    /// # Errors
    /// Rejects unsupported media, unsafe relative references and overlong encoded targets.
    pub fn artifact(reference: &CanvasFileReference) -> Result<Self, ComfyViewError> {
        Self::build(reference, true)
    }

    /// Encode only validated backend-relative fields, preserving the native decoder's narrower admission.
    fn build(reference: &CanvasFileReference, artifact: bool) -> Result<Self, ComfyViewError> {
        let filename = relative(reference.filename(), false).ok_or(ComfyViewError::Filename)?;
        if (!artifact && filename.contains(".."))
            || filename.ends_with('/')
            || ["[input]", "[output]", "[temp]"].iter().any(|suffix| filename.ends_with(suffix))
        {
            return Err(ComfyViewError::Filename);
        }
        let format = if artifact {
            ComfyOutputFormat::from_artifact_filename(&filename)
        } else {
            ComfyOutputFormat::from_filename(&filename)
        }
        .ok_or(ComfyViewError::Format)?;
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
