// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Share immutable source identity while keeping mutable stream cursors on admitted workers.
use crate::{ArchiveStats, ImportError, RasterArchive, RasterDescriptor};
use std::{
    io::{Read, Seek},
    sync::{Arc, Mutex},
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim, MemoryLease};
use sugarsubstitute_image_ops::{
    AssetId, ImageError, Pixel, RasterSampleRequest, RasterSource, Region,
};

struct Reader<R> {
    archive: Mutex<RasterArchive<R>>,
    _memory: MemoryClaim,
}
/// Cloneable immutable source identity for document snapshots and independent evaluation workers.
/// Native/UI callers inspect the descriptor without locking. Only admitted region reads lock the
/// mutable stream cursor; the lock is never a scheduling queue or an image-cache admission owner.
pub struct SharedRaster<R> {
    descriptor: RasterDescriptor,
    reader: Arc<Reader<R>>,
}
impl<R: Read + Seek> SharedRaster<R> {
    /// Retain an existing archive and admit the shared reader's complete fixed metadata.
    /// # Errors
    /// Preserves original retained-memory admission failures.
    pub fn new(archive: RasterArchive<R>, memory: &MemoryLease) -> Result<Self, ImportError> {
        let claim = memory.claim(size_of::<Reader<R>>() as u64 + 128)?;
        let descriptor = archive.descriptor();
        Ok(Self {
            descriptor,
            reader: Arc::new(Reader { archive: Mutex::new(archive), _memory: claim }),
        })
    }
    /// Original content identity and geometry, available without touching worker-owned I/O.
    pub const fn descriptor(&self) -> RasterDescriptor {
        self.descriptor
    }
    /// Cumulative authenticated source work completed by the shared archive reader.
    /// Call only from an admitted worker: observation waits for any active read and must not block
    /// a native UI thread.
    /// # Errors
    /// Reports failure to recover the shared reader lock without replacing its original state.
    pub fn stats(&self) -> Result<ArchiveStats, ImageError> {
        let source = self
            .reader
            .archive
            .lock()
            .map_err(|_| ImageError::Invalid("Shared raster reader failed"))?;
        Ok(source.stats())
    }
}
impl<R> Clone for SharedRaster<R> {
    fn clone(&self) -> Self {
        Self { descriptor: self.descriptor, reader: Arc::clone(&self.reader) }
    }
}
impl<R: Read + Seek> RasterSource for SharedRaster<R> {
    fn read_sample_batch(
        &mut self,
        asset: AssetId,
        requests: &mut [RasterSampleRequest<'_>],
        context: &JobContext,
    ) -> Result<(), ImageError> {
        if context.cancellation_requested() {
            return Err(ImageError::Cancelled);
        }
        let mut source = self
            .reader
            .archive
            .lock()
            .map_err(|_| ImageError::Invalid("Shared raster reader failed"))?;
        source.read_sample_batch(asset, requests, context)
    }

    fn read_samples(
        &mut self,
        asset: AssetId,
        samples: &sugarsubstitute_image_ops::RasterSamples,
        output: &mut [Pixel],
        context: &JobContext,
    ) -> Result<(), ImageError> {
        if context.cancellation_requested() {
            return Err(ImageError::Cancelled);
        }
        let mut source = self
            .reader
            .archive
            .lock()
            .map_err(|_| ImageError::Invalid("Shared raster reader failed"))?;
        source.read_samples(asset, samples, output, context)
    }

    fn read(
        &mut self,
        asset: AssetId,
        region: Region,
        output: &mut [Pixel],
        context: &JobContext,
    ) -> Result<(), ImageError> {
        if context.cancellation_requested() {
            return Err(ImageError::Cancelled);
        }
        let mut source = self
            .reader
            .archive
            .lock()
            .map_err(|_| ImageError::Invalid("Shared raster reader failed"))?;
        source.read(asset, region, output, context)
    }
}
