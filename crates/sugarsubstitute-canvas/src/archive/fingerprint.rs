// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Identify archive content cheaply, so unchanged documents are never saved again.
use super::{ArchivedDocument, InputDocumentArchive};
use crate::LayerContent;
use sha2::{Digest, Sha256};
use sugarsubstitute_image_ops::Coverage;

impl InputDocumentArchive {
    /// Content identity covering metadata and every archived field. Coverage contributes its
    /// precomputed fingerprint, so the cost follows layer count rather than pixel count.
    pub fn fingerprint(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"sugarsubstitute-input-archive-v1\0");
        bytes(&mut hash, self.host_metadata());
        for document in self.documents() {
            document_fields(&mut hash, document);
        }
        hash.finalize().into()
    }
}

fn document_fields(hash: &mut Sha256, document: &ArchivedDocument) {
    bytes(hash, document.key().as_bytes());
    hash.update(document.extent().width().to_le_bytes());
    hash.update(document.extent().height().to_le_bytes());
    hash.update((document.layers().len() as u64).to_le_bytes());
    for layer in document.layers() {
        hash.update(layer.id().get().to_le_bytes());
        bytes(hash, layer.name().as_bytes());
        hash.update([u8::from(layer.visible()), u8::from(layer.locked())]);
        hash.update(layer.opacity().to_bits().to_le_bytes());
        coverage(hash, layer.mask());
        match layer.content() {
            LayerContent::Raster { asset, extent } => {
                hash.update([0]);
                hash.update(asset.0);
                hash.update(extent.width().to_le_bytes());
                hash.update(extent.height().to_le_bytes());
            }
            LayerContent::PlacedRaster { asset, extent, bounds } => {
                hash.update([1]);
                hash.update(asset.0);
                hash.update(extent.width().to_le_bytes());
                hash.update(extent.height().to_le_bytes());
                bounds.iter().for_each(|value| hash.update(value.to_bits().to_le_bytes()));
            }
            LayerContent::VectorRectangle(rectangle) => {
                hash.update([2]);
                rectangle
                    .bounds()
                    .iter()
                    .for_each(|value| hash.update(value.to_bits().to_le_bytes()));
                let color = rectangle.color().channels();
                color.iter().for_each(|value| hash.update(value.to_bits().to_le_bytes()));
            }
            LayerContent::Exposure { stops } => {
                hash.update([3]);
                hash.update(stops.to_bits().to_le_bytes());
            }
            LayerContent::Coverage { coverage: content, appearance } => {
                hash.update([4]);
                hash.update(appearance.color());
                hash.update(appearance.opacity().to_bits().to_le_bytes());
                coverage(hash, Some(content));
            }
        }
        match document.source(layer.id()) {
            None => hash.update([0]),
            Some(source) => {
                hash.update([1]);
                bytes(hash, source.as_bytes());
            }
        }
    }
    coverage(hash, document.selection());
}

fn coverage(hash: &mut Sha256, coverage: Option<&Coverage>) {
    match coverage {
        None => hash.update([0]),
        Some(coverage) => {
            hash.update([1]);
            hash.update(coverage.fingerprint());
            match coverage.outside() {
                None => hash.update([0]),
                Some((frame, origin)) => {
                    hash.update([1]);
                    origin.iter().for_each(|value| hash.update(value.to_le_bytes()));
                    hash.update(frame.extent().width().to_le_bytes());
                    hash.update(frame.extent().height().to_le_bytes());
                    hash.update(frame.fingerprint());
                }
            }
        }
    }
}

fn bytes(hash: &mut Sha256, value: &[u8]) {
    hash.update((value.len() as u64).to_le_bytes());
    hash.update(value);
}
