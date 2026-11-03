// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The masks a shared-edge session reshapes: their original convex boundaries, the mappings
//! they went through, and the work that resamples them.
use super::{
    super::{
        ToolContext,
        session::{Checkpoints, EditWork, Previewed},
    },
    edges::Point,
    seams::Shape,
};
use crate::{CanvasError, DocumentEdit, DocumentSnapshot, LayerContent, LayerId};
use sugarsubstitute_image_ops::{CoverageAffine, CoverageMapping};

/// Coverage CuteCanvas counts as occupied: 128 of 255.
const OCCUPIED: u16 = 128 * 257;

/// The masks taking part and the mappings they went through.
#[derive(Debug)]
pub(super) struct Edges {
    pub(super) base: (u64, u64),
    pub(super) layers: Vec<LayerId>,
    pub(super) hulls: Vec<Vec<Point>>,
    pub(super) mappings: Checkpoints<Vec<CoverageMapping>>,
    pub(super) preview: Previewed,
    pub(super) participants: Option<Vec<LayerId>>,
}
impl Edges {
    pub(super) fn from(document: &DocumentSnapshot) -> Self {
        let (layers, hulls) = document
            .layers()
            .iter()
            .filter(|layer| layer.visible())
            .filter_map(|layer| match layer.content() {
                LayerContent::Coverage { coverage, .. } => {
                    Some((layer.id(), coverage.convex_boundary(OCCUPIED)))
                }
                _ => None,
            })
            .unzip::<_, _, Vec<_>, Vec<_>>();
        let identity = vec![CoverageMapping::Affine(CoverageAffine::IDENTITY); layers.len()];
        Self {
            base: (document.identity(), document.revision()),
            layers,
            hulls,
            mappings: Checkpoints::new(identity),
            preview: Previewed::default(),
            participants: None,
        }
    }
    pub(super) fn work(&self, mappings: &[CoverageMapping]) -> EditWork {
        let changes: Vec<(LayerId, CoverageMapping)> = self
            .layers
            .iter()
            .zip(mappings)
            .filter(|(_, mapping)| **mapping != CoverageMapping::Affine(CoverageAffine::IDENTITY))
            .map(|(layer, mapping)| (*layer, mapping.clone()))
            .collect();
        Box::new(move |base, job| {
            let mut edits = Vec::new();
            for (id, mapping) in &changes {
                let layer = base
                    .layers()
                    .iter()
                    .find(|layer| layer.id() == *id)
                    .ok_or(CanvasError::Invalid("A shared-edge mask is missing"))?;
                let LayerContent::Coverage { coverage, appearance } = layer.content() else {
                    return Err(CanvasError::Invalid("A shared-edge layer is not a mask"));
                };
                let coverage = coverage.remap(mapping, job)?;
                let content = LayerContent::Coverage { coverage, appearance: *appearance };
                edits.push(DocumentEdit::Replace(layer.clone().with_content(content)));
            }
            Ok(DocumentEdit::Group(edits))
        })
    }
    /// Preview `mappings`; at the base nothing has moved.
    pub(super) fn show(
        &mut self,
        mappings: &[CoverageMapping],
        context: &mut ToolContext<'_>,
    ) -> Result<(), CanvasError> {
        if mappings == self.mappings.base().as_slice() {
            self.preview.discard(context);
            return Ok(());
        }
        let work = self.work(mappings);
        self.preview.preview(context, work)
    }
    /// The current checkpoint with a gesture's changed participants.
    pub(super) fn with(&self, changes: &[(LayerId, CoverageMapping)]) -> Vec<CoverageMapping> {
        let mut mappings = self.mappings.current().clone();
        for (layer, mapping) in changes {
            if let Some(index) = self.layers.iter().position(|candidate| candidate == layer) {
                mappings[index] = mapping.clone();
            }
        }
        mappings
    }
}

/// Every visible layer as discovery sees it: masks by their mapped convex boundary, other layers
/// by their bounds.
pub(super) fn shapes(
    document: &DocumentSnapshot,
    edges: &Edges,
    mappings: &[CoverageMapping],
) -> Vec<Shape> {
    let identity = CoverageMapping::Affine(CoverageAffine::IDENTITY);
    document
        .layers()
        .iter()
        .filter(|layer| layer.visible())
        .filter_map(|layer| {
            let Some(index) = edges.layers.iter().position(|candidate| *candidate == layer.id())
            else {
                let extent = document.extent();
                let [left, top, right, bottom] = match layer.content() {
                    LayerContent::Raster { .. } => {
                        [0.0, 0.0, f64::from(extent.width()), f64::from(extent.height())]
                    }
                    LayerContent::PlacedRaster { bounds: [x, y, width, height], .. } => {
                        [*x, *y, x + width, y + height]
                    }
                    LayerContent::VectorRectangle(rectangle) => rectangle.bounds(),
                    LayerContent::Exposure { .. } | LayerContent::Coverage { .. } => return None,
                };
                let boundary = vec![[left, top], [right, top], [right, bottom], [left, bottom]];
                return Some(Shape {
                    layer: layer.id(),
                    eligible: false,
                    mapping: identity.clone(),
                    boundary,
                });
            };
            let mapping = mappings[index].clone();
            let boundary = match (&mapping, mapping.target_boundary()) {
                (_, Some(target)) => target.to_vec(),
                (CoverageMapping::Affine(affine), None) => {
                    edges.hulls[index].iter().map(|point| affine.map(*point)).collect()
                }
                _ => Vec::new(),
            };
            Some(Shape { layer: layer.id(), eligible: true, mapping, boundary })
        })
        .collect()
}
