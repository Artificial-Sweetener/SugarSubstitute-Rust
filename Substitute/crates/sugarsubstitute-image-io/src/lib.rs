// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Streaming image decoding and reconstructible raster tile formats independent of windowing.
mod archive;
mod buffer;
#[cfg(windows)]
mod cached;
#[cfg(windows)]
mod cached_input;
mod descriptor;
mod error;
#[cfg(windows)]
mod image_input;
mod jpeg_input;
mod layout;
mod png_input;
mod shared;
#[cfg(windows)]
mod source;
mod write;
pub use archive::{ArchiveStats, RasterArchive};
#[cfg(windows)]
pub use cached::{CacheReuse, CachedRaster, import_cached_image, import_image};
pub use descriptor::{RasterDescriptor, Transfer};
pub use error::ImportError;
pub use jpeg_input::JpegImport;
pub use layout::{ChunkLayout, Packing};
pub use png_input::PngImport;
pub use shared::SharedRaster;
#[cfg(windows)]
pub use source::ImageSource;
pub use write::write_raster;

/// Largest immutable encoded image original accepted by the import owner, in bytes.
pub const MAX_ENCODED_SOURCE_BYTES: u64 = 32 * 1024 * 1024 * 1024;

/// Observe original worker controls without retaining its execution context.
fn check(context: &sugarsubstitute_contracts::execution::JobContext) -> Result<(), ImportError> {
    if context.cancellation_requested() {
        return Err(sugarsubstitute_image_ops::ImageError::Cancelled.into());
    }
    if context.deadline().is_some_and(|deadline| context.now() >= deadline) {
        return Err(sugarsubstitute_image_ops::ImageError::Deadline.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
