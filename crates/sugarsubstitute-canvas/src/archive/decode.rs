// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate archive bytes completely before any decoded document is returned.
use super::{
    ArchiveError, ArchiveIntegrity, ArchivedDocument, HEADER_BYTES, InputDocumentArchive,
    MAX_ARCHIVE_BYTES, MAX_ARCHIVED_DOCUMENTS, MAX_HOST_METADATA_BYTES, MAX_SOURCE_REFERENCE_BYTES,
    SourceReference, check, validate_key,
};
use crate::{
    CanvasError, Layer, LayerContent, LayerId, MaskAppearance, document::MAX_LAYERS,
    document::validate_stack,
};
use flate2::read::DeflateDecoder;
use std::{io::Read, sync::Arc};
use sugarsubstitute_contracts::execution::{JobContext, MemoryLease};
use sugarsubstitute_image_ops::{
    AssetId, Coverage, CoverageBuilder, Extent, ImageError, Pixel, TILE_EDGE, VectorRectangle,
};

pub(super) fn archive(
    bytes: &[u8],
    memory: &MemoryLease,
    context: &JobContext,
) -> Result<InputDocumentArchive, ArchiveError> {
    check(context)?;
    if bytes.len() as u64 > MAX_ARCHIVE_BYTES {
        return Err(ArchiveError::Oversized);
    }
    let payload = ArchiveIntegrity::check(bytes)?;
    let mut reader = Reader {
        input: DeflateDecoder::new(&bytes[HEADER_BYTES..]),
        remaining: payload,
        memory,
        context,
    };
    let metadata = reader.bytes32(MAX_HOST_METADATA_BYTES)?;
    let count = reader.u32()? as usize;
    if count > MAX_ARCHIVED_DOCUMENTS {
        return Err(ArchiveError::Corrupt("Too many input documents"));
    }
    let mut documents = Vec::new();
    documents.try_reserve_exact(count).map_err(ImageError::from)?;
    for _ in 0..count {
        let document = reader.document()?;
        if documents
            .last()
            .is_some_and(|last: &ArchivedDocument| last.key.as_bytes() >= document.key.as_bytes())
        {
            return Err(ArchiveError::Corrupt("Document keys are not strictly ascending"));
        }
        documents.push(document);
    }
    reader.finish()?;
    Ok(InputDocumentArchive { documents, metadata: Arc::from(metadata) })
}

/// Treat a content rule violation in archived bytes as corruption.
fn corrupt(cause: CanvasError) -> ArchiveError {
    match cause {
        CanvasError::Invalid(reason) | CanvasError::Image(ImageError::Invalid(reason)) => {
            ArchiveError::Corrupt(reason)
        }
        CanvasError::Image(cause) => ArchiveError::Image(cause),
    }
}

fn image(cause: ImageError) -> ArchiveError {
    corrupt(CanvasError::Image(cause))
}

/// Decompressed payload reads limited to the declared payload length.
struct Reader<'a> {
    input: DeflateDecoder<&'a [u8]>,
    remaining: u64,
    memory: &'a MemoryLease,
    context: &'a JobContext,
}
impl Reader<'_> {
    fn raw(&mut self, output: &mut [u8]) -> Result<(), ArchiveError> {
        if (output.len() as u64) > self.remaining {
            return Err(ArchiveError::Corrupt("Content exceeds the declared payload"));
        }
        self.remaining -= output.len() as u64;
        self.input
            .read_exact(output)
            .map_err(|_| ArchiveError::Corrupt("Archive body does not decompress to its payload"))
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], ArchiveError> {
        let mut value = [0; N];
        self.raw(&mut value)?;
        Ok(value)
    }
    fn u8(&mut self) -> Result<u8, ArchiveError> {
        Ok(self.array::<1>()?[0])
    }
    fn u16(&mut self) -> Result<u16, ArchiveError> {
        self.array().map(u16::from_le_bytes)
    }
    fn u32(&mut self) -> Result<u32, ArchiveError> {
        self.array().map(u32::from_le_bytes)
    }
    fn u64(&mut self) -> Result<u64, ArchiveError> {
        self.array().map(u64::from_le_bytes)
    }
    fn f32(&mut self) -> Result<f32, ArchiveError> {
        self.u32().map(f32::from_bits)
    }
    fn f64s(&mut self) -> Result<[f64; 4], ArchiveError> {
        let mut values = [0.0; 4];
        for value in &mut values {
            *value = f64::from_bits(self.u64()?);
        }
        Ok(values)
    }
    fn bytes(&mut self, length: usize) -> Result<Vec<u8>, ArchiveError> {
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length).map_err(ImageError::from)?;
        bytes.resize(length, 0);
        self.raw(&mut bytes)?;
        Ok(bytes)
    }
    fn bytes16(&mut self, maximum: usize) -> Result<Vec<u8>, ArchiveError> {
        let length = usize::from(self.u16()?);
        if length > maximum {
            return Err(ArchiveError::Corrupt("Field exceeds its bound"));
        }
        self.bytes(length)
    }
    fn bytes32(&mut self, maximum: usize) -> Result<Vec<u8>, ArchiveError> {
        let length = self.u32()? as usize;
        if length > maximum {
            return Err(ArchiveError::Corrupt("Field exceeds its bound"));
        }
        self.bytes(length)
    }
    fn text(&mut self, maximum: usize) -> Result<String, ArchiveError> {
        String::from_utf8(self.bytes16(maximum)?)
            .map_err(|_| ArchiveError::Corrupt("Text is not UTF-8"))
    }
    fn extent(&mut self) -> Result<Extent, ArchiveError> {
        Extent::new(self.u32()?, self.u32()?).map_err(image)
    }
    /// Require the declared payload to end exactly where the compressed stream ends.
    fn finish(mut self) -> Result<(), ArchiveError> {
        if self.remaining != 0 {
            return Err(ArchiveError::Corrupt("Payload ends before its declared length"));
        }
        let mut probe = [0; 1];
        match self.input.read(&mut probe) {
            Ok(0) => Ok(()),
            Ok(_) => Err(ArchiveError::Corrupt("Payload continues past its declared length")),
            Err(_) => Err(ArchiveError::Corrupt("Archive body does not decompress")),
        }
    }

    fn document(&mut self) -> Result<ArchivedDocument, ArchiveError> {
        check(self.context)?;
        let key = self.text(super::MAX_DOCUMENT_KEY_BYTES)?;
        validate_key(&key).map_err(ArchiveError::Corrupt)?;
        let extent = self.extent()?;
        let count = usize::from(self.u8()?);
        if count > MAX_LAYERS {
            return Err(ArchiveError::Corrupt("Too many layers"));
        }
        let mut layers = Vec::new();
        let mut sources = Vec::new();
        layers.try_reserve_exact(count).map_err(ImageError::from)?;
        sources.try_reserve_exact(count).map_err(ImageError::from)?;
        for _ in 0..count {
            let (layer, source) = self.layer(extent)?;
            layers.push(layer);
            sources.push(source);
        }
        let selection = self.optional_coverage(extent)?;
        validate_stack(extent, &layers, selection.as_ref()).map_err(corrupt)?;
        Ok(ArchivedDocument { key: Arc::from(key), extent, layers, sources, selection })
    }

    fn layer(&mut self, extent: Extent) -> Result<(Layer, Option<SourceReference>), ArchiveError> {
        let id = LayerId::new(self.u64()?).map_err(corrupt)?;
        let name = self.text(4 * 256)?;
        let flags = self.u8()?;
        if flags & !0b11 != 0 {
            return Err(ArchiveError::Corrupt("Unknown layer flags"));
        }
        let opacity = self.f32()?;
        let mask = self.optional_coverage(extent)?;
        let (content, source) = match self.u8()? {
            0 => {
                let asset = AssetId(self.array()?);
                let extent = self.extent()?;
                (LayerContent::Raster { asset, extent }, Some(self.source()?))
            }
            1 => {
                let asset = AssetId(self.array()?);
                let extent = self.extent()?;
                let bounds = self.f64s()?;
                (LayerContent::PlacedRaster { asset, extent, bounds }, Some(self.source()?))
            }
            2 => {
                let bounds = self.f64s()?;
                let color = [self.f32()?, self.f32()?, self.f32()?, self.f32()?];
                let color = Pixel::premultiplied(color).map_err(image)?;
                (
                    LayerContent::VectorRectangle(
                        VectorRectangle::new(bounds, color).map_err(image)?,
                    ),
                    None,
                )
            }
            3 => (LayerContent::Exposure { stops: self.f32()? }, None),
            4 => {
                let color = self.array()?;
                let appearance = MaskAppearance::new(color, self.f32()?).map_err(corrupt)?;
                let coverage = self.coverage(extent)?;
                (LayerContent::Coverage { coverage, appearance }, None)
            }
            _ => return Err(ArchiveError::Corrupt("Unknown layer content")),
        };
        let layer = Layer::new(id, content)
            .and_then(|layer| layer.with_name(&name))
            .and_then(|layer| layer.with_opacity(opacity))
            .map_err(corrupt)?
            .with_visibility(flags & 1 != 0)
            .with_locked(flags & 2 != 0)
            .with_mask(mask);
        Ok((layer, source))
    }

    fn source(&mut self) -> Result<SourceReference, ArchiveError> {
        let bytes = self.bytes32(MAX_SOURCE_REFERENCE_BYTES)?;
        SourceReference::new(&bytes).map_err(|_| ArchiveError::Corrupt("Source reference bound"))
    }

    fn optional_coverage(&mut self, extent: Extent) -> Result<Option<Coverage>, ArchiveError> {
        match self.u8()? {
            0 => Ok(None),
            1 => self.coverage(extent).map(Some),
            _ => Err(ArchiveError::Corrupt("Unknown optional coverage tag")),
        }
    }

    fn coverage(&mut self, extent: Extent) -> Result<Coverage, ArchiveError> {
        let mut builder = CoverageBuilder::new(extent, self.u16()?);
        let count = self.u64()?;
        let tiles = u64::from(extent.width().div_ceil(TILE_EDGE))
            * u64::from(extent.height().div_ceil(TILE_EDGE));
        if count > tiles {
            return Err(ArchiveError::Corrupt("More coverage tiles than the grid holds"));
        }
        let mut row = [0; 256];
        for _ in 0..count {
            check(self.context)?;
            let index = self.u64()?;
            match self.u8()? {
                0 => builder.uniform(index, self.u16()?).map_err(image)?,
                1 => {
                    let length = CoverageBuilder::stored_tile_values();
                    let claim = self.memory.claim((length * 2) as u64).map_err(ImageError::from)?;
                    let mut values = Vec::new();
                    values.try_reserve_exact(length).map_err(ImageError::from)?;
                    for _ in 0..length / 128 {
                        self.raw(&mut row)?;
                        values.extend(
                            row.chunks_exact(2).map(|pair| u16::from_le_bytes([pair[0], pair[1]])),
                        );
                    }
                    builder.stored(index, values, self.memory).map_err(image)?;
                    drop(claim);
                }
                _ => return Err(ArchiveError::Corrupt("Unknown coverage tile kind")),
            }
        }
        builder.finish(self.memory).map_err(image)
    }
}
