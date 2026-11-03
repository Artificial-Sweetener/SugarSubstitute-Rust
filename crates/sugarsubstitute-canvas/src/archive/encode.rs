// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Write the canonical archive encoding: deterministic payload, deflate body, digest header.
use super::{
    ArchiveError, ArchivedDocument, HEADER_BYTES, INPUT_ARCHIVE_VERSION, InputDocumentArchive,
    MAGIC, MAX_ARCHIVE_BYTES, MAX_PAYLOAD_BYTES, check,
};
use crate::{Layer, LayerContent};
use flate2::{Compression, write::DeflateEncoder};
use sha2::{Digest, Sha256};
use std::io::{self, Write};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
use sugarsubstitute_image_ops::{Coverage, CoverageTileValues, ImageError};

pub(super) fn archive(
    archive: &InputDocumentArchive,
    context: &JobContext,
) -> Result<Vec<u8>, ArchiveError> {
    check(context)?;
    let limit = archive.encoded_bound().min(MAX_ARCHIVE_BYTES);
    let claim = context.memory().claim(limit).map_err(ImageError::from)?;
    let mut sink = Sink { bytes: Vec::new(), limit: limit as usize, _claim: claim };
    sink.write_all(&[0; HEADER_BYTES]).map_err(io_error)?;
    let encoder = DeflateEncoder::new(sink, Compression::default());
    let mut payload = Payload { output: encoder, length: 0, context };
    payload.bytes32(archive.host_metadata())?;
    payload.u32(archive.documents().len() as u32)?;
    for document in archive.documents() {
        payload.document(document)?;
    }
    let length = payload.length;
    let mut sink = payload.output.finish().map_err(io_error)?;
    let body = (sink.bytes.len() - HEADER_BYTES) as u64;
    let header = &mut sink.bytes[..HEADER_BYTES];
    header[..8].copy_from_slice(&MAGIC);
    header[8..12].copy_from_slice(&INPUT_ARCHIVE_VERSION.to_le_bytes());
    header[16..24].copy_from_slice(&length.to_le_bytes());
    header[24..32].copy_from_slice(&body.to_le_bytes());
    let digest = Sha256::new()
        .chain_update(&sink.bytes[..32])
        .chain_update(&sink.bytes[HEADER_BYTES..])
        .finalize();
    sink.bytes[32..HEADER_BYTES].copy_from_slice(&digest);
    check(context)?;
    Ok(sink.bytes)
}

/// Map writer failures: exceeding the claimed bound is an oversized archive; other compressor
/// or allocation failures keep their cause.
fn io_error(cause: io::Error) -> ArchiveError {
    match cause.kind() {
        io::ErrorKind::FileTooLarge => ArchiveError::Oversized,
        _ => ArchiveError::Image(ImageError::Source(Box::new(cause))),
    }
}

/// Encoded bytes within one memory claim; growth past the claim is refused.
struct Sink {
    bytes: Vec<u8>,
    limit: usize,
    _claim: MemoryClaim,
}
impl Write for Sink {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let needed = self.bytes.len().saturating_add(data.len());
        if needed > self.limit {
            return Err(io::Error::new(io::ErrorKind::FileTooLarge, "Archive exceeds its bound"));
        }
        if needed > self.bytes.capacity() {
            let target = needed.max(self.bytes.capacity().saturating_mul(2)).min(self.limit);
            self.bytes.try_reserve_exact(target - self.bytes.len()).map_err(io::Error::other)?;
        }
        self.bytes.extend_from_slice(data);
        Ok(data.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Little-endian payload fields written through the compressor.
struct Payload<'a> {
    output: DeflateEncoder<Sink>,
    length: u64,
    context: &'a JobContext,
}
impl Payload<'_> {
    fn raw(&mut self, data: &[u8]) -> Result<(), ArchiveError> {
        self.length += data.len() as u64;
        if self.length > MAX_PAYLOAD_BYTES {
            return Err(ArchiveError::Oversized);
        }
        self.output.write_all(data).map_err(io_error)
    }
    fn u8(&mut self, value: u8) -> Result<(), ArchiveError> {
        self.raw(&[value])
    }
    fn u16(&mut self, value: u16) -> Result<(), ArchiveError> {
        self.raw(&value.to_le_bytes())
    }
    fn u32(&mut self, value: u32) -> Result<(), ArchiveError> {
        self.raw(&value.to_le_bytes())
    }
    fn u64(&mut self, value: u64) -> Result<(), ArchiveError> {
        self.raw(&value.to_le_bytes())
    }
    fn f32(&mut self, value: f32) -> Result<(), ArchiveError> {
        self.u32(value.to_bits())
    }
    fn f64s(&mut self, values: [f64; 4]) -> Result<(), ArchiveError> {
        values.into_iter().try_for_each(|value| self.u64(value.to_bits()))
    }
    fn bytes16(&mut self, data: &[u8]) -> Result<(), ArchiveError> {
        self.u16(data.len() as u16)?;
        self.raw(data)
    }
    fn bytes32(&mut self, data: &[u8]) -> Result<(), ArchiveError> {
        self.u32(data.len() as u32)?;
        self.raw(data)
    }

    fn document(&mut self, document: &ArchivedDocument) -> Result<(), ArchiveError> {
        check(self.context)?;
        self.bytes16(document.key.as_bytes())?;
        self.u32(document.extent.width())?;
        self.u32(document.extent.height())?;
        self.u8(document.layers.len() as u8)?;
        for (layer, source) in document.layers.iter().zip(&document.sources) {
            self.layer(layer, source.as_ref().map(super::SourceReference::as_bytes))?;
        }
        self.optional_coverage(document.selection.as_ref())
    }

    fn layer(&mut self, layer: &Layer, source: Option<&[u8]>) -> Result<(), ArchiveError> {
        self.u64(layer.id().get())?;
        self.bytes16(layer.name().as_bytes())?;
        self.u8(u8::from(layer.visible()) | (u8::from(layer.locked()) << 1))?;
        self.f32(layer.opacity())?;
        self.optional_coverage(layer.mask())?;
        let missing = ArchiveError::Invalid("Raster layer has no source");
        match layer.content() {
            LayerContent::Raster { asset, extent } => {
                self.u8(0)?;
                self.raw(&asset.0)?;
                self.u32(extent.width())?;
                self.u32(extent.height())?;
                self.bytes32(source.ok_or(missing)?)
            }
            LayerContent::PlacedRaster { asset, extent, bounds } => {
                self.u8(1)?;
                self.raw(&asset.0)?;
                self.u32(extent.width())?;
                self.u32(extent.height())?;
                self.f64s(*bounds)?;
                self.bytes32(source.ok_or(missing)?)
            }
            LayerContent::VectorRectangle(rectangle) => {
                self.u8(2)?;
                self.f64s(rectangle.bounds())?;
                rectangle.color().channels().into_iter().try_for_each(|value| self.f32(value))
            }
            LayerContent::Exposure { stops } => {
                self.u8(3)?;
                self.f32(*stops)
            }
            LayerContent::Coverage { coverage, appearance } => {
                self.u8(4)?;
                self.raw(&appearance.color())?;
                self.f32(appearance.opacity())?;
                self.coverage(coverage)
            }
        }
    }

    fn optional_coverage(&mut self, coverage: Option<&Coverage>) -> Result<(), ArchiveError> {
        match coverage {
            None => self.u8(0),
            Some(coverage) => {
                self.u8(1)?;
                self.coverage(coverage)
            }
        }
    }

    fn coverage(&mut self, coverage: &Coverage) -> Result<(), ArchiveError> {
        self.u16(coverage.default_value())?;
        self.u64(coverage.tile_values().count() as u64)?;
        for (index, values) in coverage.tile_values() {
            check(self.context)?;
            self.u64(index)?;
            match values {
                CoverageTileValues::Uniform(value) => {
                    self.u8(0)?;
                    self.u16(value)?;
                }
                CoverageTileValues::Stored(values) => {
                    self.u8(1)?;
                    let mut bytes = [0; 256];
                    for row in values.chunks_exact(128) {
                        for (target, value) in bytes.chunks_exact_mut(2).zip(row) {
                            target.copy_from_slice(&value.to_le_bytes());
                        }
                        self.raw(&bytes)?;
                    }
                }
            }
        }
        Ok(())
    }
}
