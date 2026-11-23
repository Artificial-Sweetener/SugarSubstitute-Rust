// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The reader behind a canvas source, and loading of live preview frames outside the image cache.
use super::*;
use sugarsubstitute_image_io::MemoryRaster;

/// Where a canvas source's decoded archive is read from: the image cache for authored and final
/// images, or memory for a transient frame shown once and replaced.
pub(in crate::native_canvas) enum SourceReader {
    Cache(CacheReader<'static>),
    /// Only live previews, a document composition feature, read from memory.
    #[cfg_attr(not(feature = "native-composition"), allow(dead_code))]
    Memory(MemoryRaster),
}

impl std::io::Read for SourceReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Cache(reader) => reader.read(buffer),
            Self::Memory(reader) => reader.read(buffer),
        }
    }
}

impl std::io::Seek for SourceReader {
    fn seek(&mut self, position: std::io::SeekFrom) -> std::io::Result<u64> {
        match self {
            Self::Cache(reader) => reader.seek(position),
            Self::Memory(reader) => reader.seek(position),
        }
    }
}

/// Load a live preview frame: decoded straight into memory, never written to or read from the
/// image cache, since it is shown once and replaced.
#[cfg(feature = "native-composition")]
pub(in crate::native_canvas) fn load_transient(
    original: ImageSource,
    revision: u64,
    resources: &Resources,
    context: &JobContext,
) -> Result<Scene, WorkError> {
    let raster = original.import_transient(&resources.memory, context)?;
    let source = SharedRaster::new(raster.map_reader(SourceReader::Memory), &resources.memory)?;
    scene(source, original, None, revision, resources)
}

/// One document showing `source` as its single raster layer, at `revision`.
pub(super) fn scene(
    source: Source,
    original: ImageSource,
    compared: Option<Box<Companion>>,
    revision: u64,
    resources: &Resources,
) -> Result<Scene, WorkError> {
    let descriptor = source.descriptor();
    let extent = descriptor.layout().extent();
    let mut document = Document::new(revision, extent, &resources.memory)?;
    let layer =
        Layer::new(LayerId::new(1)?, LayerContent::Raster { asset: descriptor.asset(), extent })?;
    document.apply(DocumentEdit::Insert { index: 0, layer })?;
    Ok(Scene {
        document,
        source: Some(source),
        source_revision: revision,
        original: Some(original),
        compared,
        placed: Vec::new(),
    })
}
