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
mod layout;
mod png_input;
mod shared;
#[cfg(windows)]
mod source;
mod write;
pub use archive::{ArchiveStats, RasterArchive};
#[cfg(windows)]
pub use cached::{
    CacheReuse, CachedRaster, MAX_ENCODED_SOURCE_BYTES, import_cached_png, import_png,
};
pub use descriptor::{RasterDescriptor, Transfer};
pub use error::ImportError;
pub use layout::{ChunkLayout, Packing};
pub use png_input::PngImport;
pub use shared::SharedRaster;
#[cfg(windows)]
pub use source::ImageSource;
pub use write::write_raster;

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
