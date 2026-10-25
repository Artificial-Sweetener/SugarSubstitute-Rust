// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reconstruct canonical image regions from bounded independently checked source chunks.
use crate::{ImportError, Packing, RasterDescriptor, buffer::Buffer};
use sha2::{Digest, Sha256};
use std::io::{Read, Seek, SeekFrom};
use sugarsubstitute_contracts::execution::{JobContext, MemoryLease};
use sugarsubstitute_image_ops::{AssetId, ImageError, Pixel, RasterSource, Region, TILE_EDGE};

/// Actual source work, excluding decoded tiles retained by the image-ops evaluator.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ArchiveStats {
    /// Packed sample and integrity bytes requested from the read boundary.
    pub read_bytes: u64,
    /// Independently hashed chunks used to supply canonical source regions.
    pub chunks: u64,
}
/// One immutable stream with bounded charged conversion tables and chunk workspace.
/// Cache adapters retain their entry pin inside `R`; the DAG owns decoded tile reuse.
pub struct RasterArchive<R> {
    reader: R,
    descriptor: RasterDescriptor,
    samples: Buffer<u8>,
    linear: Buffer<f32>,
    stats: ArchiveStats,
}
impl<R: Read + Seek> RasterArchive<R> {
    /// Verify expected source semantics and complete archive length before region access.
    /// Original retained memory is separate from the current worker's controls.
    /// # Errors
    /// Preserves native, integrity, format, allocation, admission and control failures.
    pub fn open(
        mut reader: R,
        expected: RasterDescriptor,
        memory: &MemoryLease,
        context: &JobContext,
    ) -> Result<Self, ImportError> {
        crate::check(context)?;
        reader.rewind()?;
        let mut bytes = [0; 128];
        cache_read(reader.read_exact(&mut bytes))?;
        let descriptor = RasterDescriptor::from_header(&bytes)
            .map_err(|cause| ImportError::Corrupt(Box::new(cause)))?;
        if descriptor != expected || reader.seek(SeekFrom::End(0))? != descriptor.layout().length()
        {
            return Err(ImportError::Corrupt(Box::new(ImportError::Invalid(
                "Reconstructed source differs from its original content identity",
            ))));
        }
        let layout = descriptor.layout();
        let maximum =
            (layout.region(0, 0)?.pixels() * u64::from(layout.packing().bytes())) as usize;
        let samples = Buffer::new(maximum, 0, memory)?;
        let entries = match layout.packing() {
            Packing::Rgba8 => 256,
            Packing::Rgba16 => 65536,
        };
        let mut linear = Buffer::new(entries, 0.0, memory)?;
        for (index, value) in linear.data.iter_mut().enumerate() {
            if index % 256 == 0 {
                crate::check(context)?;
            }
            *value = descriptor.transfer().linear(index as f64 / (entries - 1) as f64);
        }
        crate::check(context)?;
        Ok(Self { reader, descriptor, samples, linear, stats: ArchiveStats::default() })
    }
    /// Exact original-content and source-interpretation contract.
    pub const fn descriptor(&self) -> RasterDescriptor {
        self.descriptor
    }
    /// Cumulative requested bytes and checked chunks.
    pub const fn stats(&self) -> ArchiveStats {
        self.stats
    }
    fn read_region(
        &mut self,
        region: Region,
        output: &mut [Pixel],
        context: &JobContext,
    ) -> Result<(), ImportError> {
        let layout = self.descriptor.layout();
        if region.width() > TILE_EDGE
            || region.height() > TILE_EDGE
            || region.intersection(layout.extent().region()) != Some(region)
            || output.len() as u64 != region.pixels()
        {
            return Err(ImportError::Invalid("Invalid bounded source region"));
        }
        let stride = layout.packing().bytes() as usize;
        for stripe in region.y() / layout.rows()..=(region.bottom() - 1) / layout.rows() {
            for column in region.x() / TILE_EDGE..=(region.right() - 1) / TILE_EDGE {
                crate::check(context)?;
                let chunk = layout.region(column, stripe)?;
                let length = chunk.pixels() as usize * stride;
                self.reader.seek(SeekFrom::Start(layout.offset(column, stripe)?))?;
                let mut expected = [0; 32];
                cache_read(self.reader.read_exact(&mut expected))?;
                cache_read(self.reader.read_exact(&mut self.samples.data[..length]))?;
                self.stats.read_bytes = self.stats.read_bytes.saturating_add(length as u64 + 32);
                self.stats.chunks = self.stats.chunks.saturating_add(1);
                let actual: [u8; 32] = Sha256::digest(&self.samples.data[..length]).into();
                if actual != expected {
                    return Err(ImportError::Corrupt(Box::new(ImportError::Invalid(
                        "Reconstructed source chunk failed integrity verification",
                    ))));
                }
                let overlap = region
                    .intersection(chunk)
                    .ok_or(ImportError::Invalid("Missing source chunk overlap"))?;
                for y in overlap.y()..overlap.bottom() {
                    crate::check(context)?;
                    for x in overlap.x()..overlap.right() {
                        let source = ((y - chunk.y()) as usize * chunk.width() as usize
                            + (x - chunk.x()) as usize)
                            * stride;
                        let encoded = &self.samples.data[source..source + stride];
                        let indices: [usize; 4] = match layout.packing() {
                            Packing::Rgba8 => {
                                std::array::from_fn(|channel| encoded[channel] as usize)
                            }
                            Packing::Rgba16 => std::array::from_fn(|channel| {
                                u16::from_be_bytes([encoded[channel * 2], encoded[channel * 2 + 1]])
                                    as usize
                            }),
                        };
                        let target = (y - region.y()) as usize * region.width() as usize
                            + (x - region.x()) as usize;
                        output[target] = Pixel::linear(
                            self.linear.data[indices[0]],
                            self.linear.data[indices[1]],
                            self.linear.data[indices[2]],
                            indices[3] as f32 / (self.linear.data.len() - 1) as f32,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// Truncated derived files are reconstructible misses; unrelated native errors keep their cause.
fn cache_read<T>(result: std::io::Result<T>) -> Result<T, ImportError> {
    result.map_err(|cause| {
        if cause.kind() == std::io::ErrorKind::UnexpectedEof {
            ImportError::Corrupt(Box::new(ImportError::Io(cause)))
        } else {
            ImportError::Io(cause)
        }
    })
}
impl<R: Read + Seek> RasterSource for RasterArchive<R> {
    fn read(
        &mut self,
        asset: AssetId,
        region: Region,
        output: &mut [Pixel],
        context: &JobContext,
    ) -> Result<(), ImageError> {
        if asset != self.descriptor.asset() {
            return Err(ImageError::Invalid("Raster source asset identity mismatch"));
        }
        self.read_region(region, output, context)
            .map_err(|cause| ImageError::Source(Box::new(cause)))
    }
}
