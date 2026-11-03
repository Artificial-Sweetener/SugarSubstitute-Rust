// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reconstruct canonical image regions from bounded independently checked source chunks.
mod batch;
mod samples;
use crate::{ImportError, Packing, RasterDescriptor, buffer::Buffer};
use sha2::{Digest, Sha256};
use std::io::{Read, Seek, SeekFrom};
use sugarsubstitute_contracts::execution::{JobContext, MemoryLease};
use sugarsubstitute_image_ops::{AssetId, ImageError, Pixel, RasterSource, Region, TILE_EDGE};

const MAX_ENCODED_BYTES: usize = 1024 * 1024 - 128;

/// Actual source work, excluding decoded tiles retained by the image-ops evaluator.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ArchiveStats {
    /// Packed sample and integrity bytes requested from the read boundary.
    pub read_bytes: u64,
    /// Independently hashed chunks used to supply canonical source regions.
    pub chunks: u64,
    /// Successful bounded encoded-data range reads, excluding archive-open metadata I/O.
    pub range_reads: u64,
    /// Encoded-data cursor repositioning operations, excluding archive-open metadata seeks.
    pub range_seeks: u64,
}
impl ArchiveStats {
    /// Return saturating work completed after an earlier cumulative observation.
    pub const fn since(self, earlier: Self) -> Self {
        Self {
            read_bytes: self.read_bytes.saturating_sub(earlier.read_bytes),
            chunks: self.chunks.saturating_sub(earlier.chunks),
            range_reads: self.range_reads.saturating_sub(earlier.range_reads),
            range_seeks: self.range_seeks.saturating_sub(earlier.range_seeks),
        }
    }
}
/// One immutable stream with bounded charged conversion tables and chunk workspace.
/// Cache adapters retain their entry pin inside `R`; the DAG owns decoded tile reuse.
pub struct RasterArchive<R> {
    reader: R,
    descriptor: RasterDescriptor,
    encoded: Buffer<u8>,
    linear: Buffer<f32>,
    stats: ArchiveStats,
    position: Option<u64>,
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
        let length = reader.seek(SeekFrom::End(0))?;
        if descriptor != expected || length != descriptor.layout().length() {
            return Err(ImportError::Corrupt(Box::new(ImportError::Invalid(
                "Reconstructed source differs from its original content identity",
            ))));
        }
        let layout = descriptor.layout();
        let maximum_chunk = usize::try_from(
            layout.region(0, 0)?.pixels() * u64::from(layout.packing().bytes()) + 32,
        )
        .map_err(|_| ImportError::Invalid("Image source chunk exceeds address space"))?;
        if maximum_chunk > MAX_ENCODED_BYTES {
            return Err(ImportError::Invalid("Image source chunk exceeds bounded workspace"));
        }
        let stripe = layout
            .stripe_bytes()
            .checked_add(
                usize::try_from(layout.columns())
                    .map_err(|_| ImportError::Invalid("Image source columns exceed address space"))?
                    .checked_mul(32)
                    .ok_or(ImportError::Invalid("Image source stripe size overflow"))?,
            )
            .ok_or(ImportError::Invalid("Image source stripe size overflow"))?;
        let encoded = Buffer::new(maximum_chunk.max(stripe.min(MAX_ENCODED_BYTES)), 0, memory)?;
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
        Ok(Self {
            reader,
            descriptor,
            encoded,
            linear,
            stats: ArchiveStats::default(),
            position: Some(length),
        })
    }
    /// Exact original-content and source-interpretation contract.
    pub const fn descriptor(&self) -> RasterDescriptor {
        self.descriptor
    }
    /// Cumulative requested bytes and checked chunks.
    pub const fn stats(&self) -> ArchiveStats {
        self.stats
    }
    fn read_chunk(
        &mut self,
        column: u32,
        stripe: u32,
        context: &JobContext,
    ) -> Result<Region, ImportError> {
        let layout = self.descriptor.layout();
        let stride = layout.packing().bytes() as usize;
        crate::check(context)?;
        let chunk = layout.region(column, stripe)?;
        let length = chunk.pixels() as usize * stride;
        self.read_encoded(layout.offset(column, stripe)?, length + 32)?;
        let expected: [u8; 32] = self.encoded.data[..32]
            .try_into()
            .map_err(|_| ImportError::Invalid("Source chunk checksum width changed"))?;
        self.stats.read_bytes = self.stats.read_bytes.saturating_add(length as u64 + 32);
        self.stats.chunks = self.stats.chunks.saturating_add(1);
        let actual: [u8; 32] = Sha256::digest(&self.payload()[..length]).into();
        if actual != expected {
            return Err(ImportError::Corrupt(Box::new(ImportError::Invalid(
                "Reconstructed source chunk failed integrity verification",
            ))));
        }
        Ok(chunk)
    }
    /// Load one bounded contiguous archive span, preserving sequential cursor position.
    fn read_encoded(&mut self, offset: u64, length: usize) -> Result<(), ImportError> {
        if length > self.encoded.data.len() {
            return Err(ImportError::Invalid("Image source read exceeds bounded workspace"));
        }
        if self.position != Some(offset) {
            self.position = None;
            let position = self.reader.seek(SeekFrom::Start(offset))?;
            if position != offset {
                return Err(ImportError::Invalid("Image source seek reached another offset"));
            }
            self.position = Some(position);
            self.stats.range_seeks = self.stats.range_seeks.saturating_add(1);
        }
        let end = offset
            .checked_add(length as u64)
            .ok_or(ImportError::Invalid("Image source read offset overflow"))?;
        self.position = None;
        cache_read(self.reader.read_exact(&mut self.encoded.data[..length]))?;
        self.position = Some(end);
        self.stats.range_reads = self.stats.range_reads.saturating_add(1);
        Ok(())
    }
    fn payload(&self) -> &[u8] {
        &self.encoded.data[32..]
    }
    fn decode_pixel(&self, encoded: &[u8]) -> Result<Pixel, ImportError> {
        let indices: [usize; 4] = match self.descriptor.layout().packing() {
            Packing::Rgba8 => std::array::from_fn(|channel| encoded[channel] as usize),
            Packing::Rgba16 => std::array::from_fn(|channel| {
                u16::from_be_bytes([encoded[channel * 2], encoded[channel * 2 + 1]]) as usize
            }),
        };
        Ok(Pixel::linear(
            self.linear.data[indices[0]],
            self.linear.data[indices[1]],
            self.linear.data[indices[2]],
            indices[3] as f32 / (self.linear.data.len() - 1) as f32,
        )?)
    }
    fn read_region(
        &mut self,
        region: Region,
        output: &mut [Pixel],
        context: &JobContext,
    ) -> Result<(), ImportError> {
        if output.len() as u64 != region.pixels() {
            return Err(ImportError::Invalid("Invalid bounded source region"));
        }
        let stride = self.descriptor.layout().packing().bytes() as usize;
        self.visit_rows(region, context, |archive, target, encoded| {
            for (pixel, sample) in output[target..].iter_mut().zip(encoded.chunks_exact(stride)) {
                *pixel = archive.decode_pixel(sample)?;
            }
            Ok(())
        })
    }
    /// Copy a region's original straight RGBA samples, as a file decoder would hand them out,
    /// without transfer decoding or premultiplication. Sixteen-bit samples keep their most
    /// significant byte. The region spans at most one 128-pixel tile on each axis.
    /// # Errors
    /// Rejects regions outside the image or a wrong output length; preserves integrity, native
    /// and control failures.
    pub fn read_samples8(
        &mut self,
        region: Region,
        output: &mut [[u8; 4]],
        context: &JobContext,
    ) -> Result<(), ImportError> {
        if output.len() as u64 != region.pixels() {
            return Err(ImportError::Invalid("Invalid bounded source region"));
        }
        let packing = self.descriptor.layout().packing();
        let stride = packing.bytes() as usize;
        self.visit_rows(region, context, |_, target, encoded| {
            for (sample, encoded) in output[target..].iter_mut().zip(encoded.chunks_exact(stride)) {
                *sample = match packing {
                    Packing::Rgba8 => [encoded[0], encoded[1], encoded[2], encoded[3]],
                    Packing::Rgba16 => [encoded[0], encoded[2], encoded[4], encoded[6]],
                };
            }
            Ok(())
        })
    }
    /// Hand each row of a bounded region's encoded samples within one chunk to `visit`, with
    /// the row-major output index of its first sample.
    fn visit_rows(
        &mut self,
        region: Region,
        context: &JobContext,
        mut visit: impl FnMut(&Self, usize, &[u8]) -> Result<(), ImportError>,
    ) -> Result<(), ImportError> {
        let layout = self.descriptor.layout();
        if region.width() > TILE_EDGE
            || region.height() > TILE_EDGE
            || region.intersection(layout.extent().region()) != Some(region)
        {
            return Err(ImportError::Invalid("Invalid bounded source region"));
        }
        let stride = layout.packing().bytes() as usize;
        for stripe in region.y() / layout.rows()..=(region.bottom() - 1) / layout.rows() {
            for column in region.x() / TILE_EDGE..=(region.right() - 1) / TILE_EDGE {
                let chunk = self.read_chunk(column, stripe, context)?;
                let overlap = region
                    .intersection(chunk)
                    .ok_or(ImportError::Invalid("Missing source chunk overlap"))?;
                for y in overlap.y()..overlap.bottom() {
                    crate::check(context)?;
                    let source = ((y - chunk.y()) as usize * chunk.width() as usize
                        + (overlap.x() - chunk.x()) as usize)
                        * stride;
                    let encoded =
                        &self.payload()[source..source + overlap.width() as usize * stride];
                    let target = (y - region.y()) as usize * region.width() as usize
                        + (overlap.x() - region.x()) as usize;
                    visit(self, target, encoded)?;
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
