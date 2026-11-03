// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Versioned, history-free archive of an input canvas's keyed documents.
//!
//! An archive holds the current content of every input document (extent, layer stack with
//! coverage, appearance and locks, and the selection) plus one opaque host metadata blob. Raster
//! layers keep their asset identity and extent with a host-supplied [`SourceReference`] from
//! which the host re-acquires the pixels; image pixels are never archived. Undo history is
//! never archived: a restored document starts with empty history.
//!
//! The encoding is a fixed 64-byte header followed by a raw deflate body:
//!
//! | Offset | Bytes | Field |
//! |---|---|---|
//! | 0 | 8 | magic `SSINDOC\0` |
//! | 8 | 4 | format version, little-endian |
//! | 12 | 4 | flags, zero |
//! | 16 | 8 | uncompressed payload length |
//! | 24 | 8 | compressed body length |
//! | 32 | 32 | SHA-256 of header bytes 0..32 followed by the body |
//!
//! The payload is little-endian: the metadata blob, then documents in ascending key byte
//! order. Coverage is stored as its default value and its non-default tiles in ascending
//! index order, each uniform (one value) or stored (128×128 values), so coverage, selection
//! and masks round-trip with identical values and fingerprints. Decoding validates every
//! bound, the digest and all content before returning anything; no partially decoded
//! archive is ever observable.
mod decode;
mod encode;
mod fingerprint;
mod integrity;
#[cfg(test)]
mod tests;

pub use integrity::ArchiveIntegrity;

use crate::{CanvasError, Document, DocumentSnapshot, Layer, LayerContent, LayerId};
use std::{error::Error, fmt, sync::Arc};
use sugarsubstitute_contracts::execution::{JobContext, MemoryLease};
use sugarsubstitute_image_ops::{Coverage, Extent, ImageError};

/// Format version this build writes and the only one it reads.
pub const INPUT_ARCHIVE_VERSION: u32 = 1;
/// Largest encoded archive, in bytes.
pub const MAX_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024;
/// Largest number of documents in one archive.
pub const MAX_ARCHIVED_DOCUMENTS: usize = 256;
/// Largest document key, in UTF-8 bytes.
pub const MAX_DOCUMENT_KEY_BYTES: usize = 256;
/// Largest host source reference of one raster layer, in bytes.
pub const MAX_SOURCE_REFERENCE_BYTES: usize = 32 * 1024;
/// Largest host metadata blob, in bytes.
pub const MAX_HOST_METADATA_BYTES: usize = 1024 * 1024;
/// Largest uncompressed payload, in bytes; bounds decompression independently of memory.
const MAX_PAYLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAGIC: [u8; 8] = *b"SSINDOC\0";
const HEADER_BYTES: usize = 64;

/// Why archive content was refused or archive bytes could not be decoded.
#[derive(Debug)]
pub enum ArchiveError {
    /// The bytes end before the archive they describe.
    Truncated,
    /// The archive, its payload or one of its declared lengths exceeds its bound.
    Oversized,
    /// The archive names a format version this build does not read.
    UnsupportedVersion(u32),
    /// The bytes are not an input-document archive, fail their digest or describe content
    /// that breaks a document rule.
    Corrupt(&'static str),
    /// Content offered for archiving exceeds a bound or is incomplete.
    Invalid(&'static str),
    /// Cancellation, deadline, memory admission or allocation failure, with its cause.
    Image(ImageError),
}
impl fmt::Display for ArchiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => formatter.write_str("Input-document archive is truncated"),
            Self::Oversized => formatter.write_str("Input-document archive exceeds its bound"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "Input-document archive version {version} is not supported")
            }
            Self::Corrupt(reason) => {
                write!(formatter, "Input-document archive is corrupt: {reason}")
            }
            Self::Invalid(reason) => {
                write!(formatter, "Input documents cannot be archived: {reason}")
            }
            Self::Image(cause) => write!(formatter, "Input-document archive work failed: {cause}"),
        }
    }
}
impl Error for ArchiveError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Image(cause) => Some(cause),
            _ => None,
        }
    }
}
impl From<ImageError> for ArchiveError {
    fn from(cause: ImageError) -> Self {
        Self::Image(cause)
    }
}
impl ArchiveError {
    /// Whether decoding stopped for cancellation, deadline or memory rather than the bytes.
    pub const fn interrupted(&self) -> bool {
        matches!(self, Self::Image(_))
    }
}

/// Opaque host bytes identifying where a raster layer's pixels are re-acquired, such as a
/// ComfyUI input file reference. Cloning shares the bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceReference(Arc<[u8]>);
impl SourceReference {
    /// Copy host reference bytes.
    /// # Errors
    /// Rejects references longer than [`MAX_SOURCE_REFERENCE_BYTES`].
    pub fn new(bytes: &[u8]) -> Result<Self, ArchiveError> {
        if bytes.len() > MAX_SOURCE_REFERENCE_BYTES {
            return Err(ArchiveError::Invalid("Source reference exceeds its bound"));
        }
        Ok(Self(Arc::from(bytes)))
    }
    /// The host's original bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// History-free content of one keyed input document.
#[derive(Clone, Debug, PartialEq)]
pub struct ArchivedDocument {
    key: Arc<str>,
    extent: Extent,
    layers: Vec<Layer>,
    sources: Vec<Option<SourceReference>>,
    selection: Option<Coverage>,
}
impl ArchivedDocument {
    /// Capture a snapshot's current content under `key`, sharing its coverage tiles.
    ///
    /// `source` supplies the reference of every raster and placed raster layer; other layers
    /// are never asked.
    /// # Errors
    /// Rejects an empty or oversized key and a raster layer without a source reference.
    pub fn capture(
        key: &str,
        snapshot: &DocumentSnapshot,
        mut source: impl FnMut(&Layer) -> Option<SourceReference>,
    ) -> Result<Self, ArchiveError> {
        validate_key(key).map_err(ArchiveError::Invalid)?;
        let mut sources = Vec::new();
        sources.try_reserve_exact(snapshot.layers().len()).map_err(ImageError::from)?;
        for layer in snapshot.layers() {
            sources.push(if needs_source(layer) {
                Some(source(layer).ok_or(ArchiveError::Invalid("Raster layer has no source"))?)
            } else {
                None
            });
        }
        Ok(Self {
            key: Arc::from(key),
            extent: snapshot.extent(),
            layers: snapshot.layers().to_vec(),
            sources,
            selection: snapshot.selection().cloned(),
        })
    }
    /// Host key naming this document within the archive.
    pub fn key(&self) -> &str {
        &self.key
    }
    /// Authored document pixel extent.
    pub const fn extent(&self) -> Extent {
        self.extent
    }
    /// Bottom-to-top layer stack.
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }
    /// Source reference of a raster or placed raster layer.
    pub fn source(&self, layer: LayerId) -> Option<&SourceReference> {
        let index = self.layers.iter().position(|candidate| candidate.id() == layer)?;
        self.sources.get(index)?.as_ref()
    }
    /// The archived selection, when something was selected.
    pub const fn selection(&self) -> Option<&Coverage> {
        self.selection.as_ref()
    }
    /// Rebuild an editable document with empty history under a new application identity.
    /// # Errors
    /// Preserves [`Document::restore`] failures.
    pub fn restore(&self, identity: u64, memory: &MemoryLease) -> Result<Document, CanvasError> {
        Document::restore(identity, self.extent, &self.layers, self.selection.clone(), memory)
    }
    fn coverages(&self) -> impl Iterator<Item = &Coverage> {
        self.layers
            .iter()
            .flat_map(|layer| {
                let content = match layer.content() {
                    LayerContent::Coverage { coverage, .. } => Some(coverage),
                    _ => None,
                };
                content.into_iter().chain(layer.mask())
            })
            .chain(&self.selection)
    }
}

/// Every input document of one canvas plus the host's opaque metadata. Cloning shares
/// coverage tiles.
#[derive(Clone, Debug, PartialEq)]
pub struct InputDocumentArchive {
    documents: Vec<ArchivedDocument>,
    metadata: Arc<[u8]>,
}
impl InputDocumentArchive {
    /// Collect documents, ordered by key, with host metadata such as workflow mapping and
    /// active state.
    /// # Errors
    /// Rejects more than [`MAX_ARCHIVED_DOCUMENTS`] documents, duplicate keys and metadata
    /// longer than [`MAX_HOST_METADATA_BYTES`].
    pub fn new(
        mut documents: Vec<ArchivedDocument>,
        metadata: &[u8],
    ) -> Result<Self, ArchiveError> {
        if documents.len() > MAX_ARCHIVED_DOCUMENTS {
            return Err(ArchiveError::Invalid("Too many input documents"));
        }
        if metadata.len() > MAX_HOST_METADATA_BYTES {
            return Err(ArchiveError::Invalid("Host metadata exceeds its bound"));
        }
        documents.sort_by(|left, right| left.key.as_bytes().cmp(right.key.as_bytes()));
        if documents.windows(2).any(|pair| pair[0].key == pair[1].key) {
            return Err(ArchiveError::Invalid("Duplicate input document key"));
        }
        Ok(Self { documents, metadata: Arc::from(metadata) })
    }
    /// Documents in ascending key byte order.
    pub fn documents(&self) -> &[ArchivedDocument] {
        &self.documents
    }
    /// The document archived under `key`.
    pub fn document(&self, key: &str) -> Option<&ArchivedDocument> {
        self.documents
            .binary_search_by(|document| document.key.as_bytes().cmp(key.as_bytes()))
            .ok()
            .map(|index| &self.documents[index])
    }
    /// The host's opaque metadata.
    pub fn host_metadata(&self) -> &[u8] {
        &self.metadata
    }
    /// Upper bound of the encoded size, in bytes, for admitting encoding work.
    pub fn encoded_bound(&self) -> u64 {
        let payload = self.payload_bound();
        payload.saturating_add(payload / 1024).saturating_add(HEADER_BYTES as u64 + 4096)
    }
    /// Bytes of coverage tile storage a decoded copy of this archive holds.
    pub fn stored_tile_bytes(&self) -> u64 {
        self.documents
            .iter()
            .flat_map(ArchivedDocument::coverages)
            .map(Coverage::stored_bytes)
            .fold(0, u64::saturating_add)
    }
    /// Encode the archive, checking cancellation and deadline between tiles.
    /// # Errors
    /// Rejects an encoding beyond [`MAX_ARCHIVE_BYTES`] and preserves cancellation, deadline
    /// and memory failures.
    pub fn encode(&self, context: &JobContext) -> Result<Vec<u8>, ArchiveError> {
        encode::archive(self, context)
    }
    /// Decode and fully validate archive bytes, charging restored coverage to `memory`.
    /// # Errors
    /// Rejects truncated, oversized, corrupt and unsupported-version archives, and preserves
    /// cancellation, deadline and memory failures.
    pub fn decode(
        bytes: &[u8],
        memory: &MemoryLease,
        context: &JobContext,
    ) -> Result<Self, ArchiveError> {
        decode::archive(bytes, memory, context)
    }
    /// Uncompressed payload size; stored tiles dominate, other fields are bounded small.
    fn payload_bound(&self) -> u64 {
        let documents = self.documents.iter().map(|document| {
            let layers = document.layers.iter().map(|layer| {
                let source =
                    document.source(layer.id()).map_or(0, |source| source.as_bytes().len() as u64);
                1024 + 128 + source
            });
            let coverages = document.coverages().map(|coverage| {
                let tiles = coverage.tile_values().count() as u64;
                16 + tiles * 11 + coverage.stored_bytes()
            });
            512 + layers.chain(coverages).fold(0, u64::saturating_add)
        });
        documents.fold(16 + self.metadata.len() as u64, u64::saturating_add)
    }
}

/// Raster content re-acquired by the host from a source reference.
const fn needs_source(layer: &Layer) -> bool {
    matches!(layer.content(), LayerContent::Raster { .. } | LayerContent::PlacedRaster { .. })
}

fn validate_key(key: &str) -> Result<(), &'static str> {
    if key.is_empty() || key.len() > MAX_DOCUMENT_KEY_BYTES {
        return Err("Document key is empty or exceeds its bound");
    }
    Ok(())
}

fn check(context: &JobContext) -> Result<(), ImageError> {
    if context.cancellation_requested() {
        return Err(ImageError::Cancelled);
    }
    if context.deadline().is_some_and(|deadline| context.now() >= deadline) {
        return Err(ImageError::Deadline);
    }
    Ok(())
}
