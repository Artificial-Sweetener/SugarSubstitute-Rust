// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Revisioned document metadata and reversible commands sharing original image assets.
use crate::CanvasError;
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};
use sugarsubstitute_image_ops::{AssetId, Extent, VectorRectangle};

const MAX_LAYERS: usize = 32;
const HISTORY_LIMIT: usize = 32;

/// Stable positive layer identity, preserved across reordering and history traversal.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct LayerId(u64);
impl LayerId {
    /// Validate application-assigned layer identity.
    /// # Errors
    /// Rejects zero, which is never an authored layer.
    pub fn new(value: u64) -> Result<Self, CanvasError> {
        if value == 0 {
            return Err(CanvasError::Invalid("Zero layer identity"));
        }
        Ok(Self(value))
    }
    /// Original application-assigned identity.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Authoritative layer content; pixels and vector geometry remain independently editable.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LayerContent {
    /// Immutable original image asset at the document's pixel extent.
    Raster {
        /// Content-bound asset identity.
        asset: AssetId,
        /// Original pixel dimensions.
        extent: Extent,
    },
    /// Continuous document-space geometry, never replaced by a rasterized cache entry.
    VectorRectangle(VectorRectangle),
    /// Non-destructive linear-light exposure of the composite beneath this layer.
    Exposure {
        /// Exposure in EV, finite and between -32 and 32.
        stops: f32,
    },
}

/// Coverage source retained as authored content; evaluation uses its alpha channel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LayerMask {
    /// Raster coverage at the document's pixel extent.
    Raster {
        /// Original coverage asset.
        asset: AssetId,
        /// Coverage pixel dimensions.
        extent: Extent,
    },
    /// Vector coverage preserving editable geometry.
    VectorRectangle(VectorRectangle),
}

/// Immutable layer description. Changes are published only through document commands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layer {
    id: LayerId,
    content: LayerContent,
    visible: bool,
    opacity: f32,
    mask: Option<LayerMask>,
}
impl Layer {
    /// Create an opaque visible layer with explicit source content.
    /// # Errors
    /// Rejects nonfinite or unsupported operation parameters.
    pub fn new(id: LayerId, content: LayerContent) -> Result<Self, CanvasError> {
        if let LayerContent::Exposure { stops } = content
            && (!stops.is_finite() || !(-32.0..=32.0).contains(&stops))
        {
            return Err(CanvasError::Invalid("Invalid layer exposure"));
        }
        Ok(Self { id, content, visible: true, opacity: 1.0, mask: None })
    }
    /// Stable layer identity.
    pub const fn id(self) -> LayerId {
        self.id
    }
    /// Original source geometry, image identity or adjustment parameters.
    pub const fn content(self) -> LayerContent {
        self.content
    }
    /// Whether this layer contributes to the document's rendered result.
    pub const fn visible(self) -> bool {
        self.visible
    }
    /// Finite layer coverage in `[0, 1]`.
    pub const fn opacity(self) -> f32 {
        self.opacity
    }
    /// Original authored coverage source.
    pub const fn mask(self) -> Option<LayerMask> {
        self.mask
    }
    /// Change visibility on a value submitted through a document command.
    pub const fn with_visibility(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }
    /// Change layer coverage without modifying its source content.
    /// # Errors
    /// Rejects nonfinite coverage or values outside `[0, 1]`.
    pub fn with_opacity(mut self, opacity: f32) -> Result<Self, CanvasError> {
        if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
            return Err(CanvasError::Invalid("Invalid layer opacity"));
        }
        self.opacity = opacity;
        Ok(self)
    }
    /// Attach or remove a mask while preserving its authored representation.
    pub const fn with_mask(mut self, mask: Option<LayerMask>) -> Self {
        self.mask = mask;
        self
    }
}

/// One atomic edit; rejected edits leave document revision and both history branches intact.
#[derive(Clone, Copy, Debug)]
pub enum DocumentEdit {
    /// Insert at an index in bottom-to-top compositing order.
    Insert {
        /// Destination index, including the current length.
        index: usize,
        /// Complete layer value with a unique identity.
        layer: Layer,
    },
    /// Remove the layer with this stable identity.
    Remove(LayerId),
    /// Replace the layer with the same identity, retaining its stacking position.
    Replace(Layer),
    /// Move an existing layer to an index in the resulting bottom-to-top stack.
    Reorder {
        /// Existing layer.
        layer: LayerId,
        /// Destination index after removing the original item.
        index: usize,
    },
}

struct Content {
    extent: Extent,
    layers: Vec<Layer>,
    _memory: MemoryClaim,
}
impl Content {
    fn copy(extent: Extent, layers: &[Layer], memory: &MemoryLease) -> Result<Self, CanvasError> {
        let claim = memory
            .claim((MAX_LAYERS * size_of::<Layer>() + 128) as u64)
            .map_err(sugarsubstitute_image_ops::ImageError::from)?;
        let mut owned = Vec::new();
        owned.try_reserve_exact(MAX_LAYERS).map_err(sugarsubstitute_image_ops::ImageError::from)?;
        owned.extend_from_slice(layers);
        Ok(Self { extent, layers: owned, _memory: claim })
    }
}

/// Immutable document content used by rendering and independent comparison views.
#[derive(Clone)]
pub struct DocumentSnapshot {
    identity: u64,
    revision: u64,
    content: Arc<Content>,
}
impl DocumentSnapshot {
    /// Stable document identity supplied by its application owner.
    pub const fn identity(&self) -> u64 {
        self.identity
    }
    /// Monotonic publication revision, advancing even when undo restores older content.
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    /// Authored document pixel extent, independent of viewport bounds.
    pub fn extent(&self) -> Extent {
        self.content.extent
    }
    /// Immutable bottom-to-top layer stack, preserving raster sources and vector geometry.
    pub fn layers(&self) -> &[Layer] {
        &self.content.layers
    }
}

/// Authoritative document metadata and bounded undo/redo history; image pixels remain shared assets.
pub struct Document {
    current: DocumentSnapshot,
    undo: Vec<Arc<Content>>,
    redo: Vec<Arc<Content>>,
    memory: MemoryLease,
    _history_memory: MemoryClaim,
}
impl Document {
    /// Create an empty document under its application's retained execution admission.
    /// # Errors
    /// Rejects zero identity and insufficient memory/allocation capacity.
    pub fn new(identity: u64, extent: Extent, memory: &MemoryLease) -> Result<Self, CanvasError> {
        if identity == 0 {
            return Err(CanvasError::Invalid("Zero document identity"));
        }
        let claim = memory
            .claim((2 * HISTORY_LIMIT * size_of::<Arc<Content>>() + 128) as u64)
            .map_err(sugarsubstitute_image_ops::ImageError::from)?;
        let mut undo = Vec::new();
        let mut redo = Vec::new();
        undo.try_reserve_exact(HISTORY_LIMIT)
            .map_err(sugarsubstitute_image_ops::ImageError::from)?;
        redo.try_reserve_exact(HISTORY_LIMIT)
            .map_err(sugarsubstitute_image_ops::ImageError::from)?;
        let content = Arc::new(Content::copy(extent, &[], memory)?);
        Ok(Self {
            current: DocumentSnapshot { identity, revision: 1, content },
            undo,
            redo,
            memory: memory.clone(),
            _history_memory: claim,
        })
    }
    /// Share the current immutable revision without copying layers or pixels.
    pub fn snapshot(&self) -> DocumentSnapshot {
        self.current.clone()
    }
    /// Apply one validated edit and publish a new revision; no-op edits retain revision/history.
    /// # Errors
    /// Rejects missing/duplicate IDs, invalid indices, extent mismatches, limits and memory pressure.
    pub fn apply(&mut self, edit: DocumentEdit) -> Result<bool, CanvasError> {
        let revision = self.next_revision()?;
        let mut content =
            Content::copy(self.current.extent(), self.current.layers(), &self.memory)?;
        match edit {
            DocumentEdit::Insert { index, layer } => {
                if content.layers.len() == MAX_LAYERS
                    || index > content.layers.len()
                    || content.layers.iter().any(|old| old.id == layer.id)
                {
                    return Err(CanvasError::Invalid("Invalid layer insertion"));
                }
                validate_layer(layer, content.extent)?;
                content.layers.insert(index, layer);
            }
            DocumentEdit::Remove(id) => {
                let index = find(&content.layers, id)?;
                content.layers.remove(index);
            }
            DocumentEdit::Replace(layer) => {
                let index = find(&content.layers, layer.id)?;
                validate_layer(layer, content.extent)?;
                content.layers[index] = layer;
            }
            DocumentEdit::Reorder { layer, index } => {
                if index >= content.layers.len() {
                    return Err(CanvasError::Invalid("Invalid layer destination"));
                }
                let original = find(&content.layers, layer)?;
                let layer = content.layers.remove(original);
                content.layers.insert(index, layer);
            }
        }
        if content.layers == self.current.content.layers {
            return Ok(false);
        }
        push_history(&mut self.undo, Arc::clone(&self.current.content));
        self.redo.clear();
        self.current = DocumentSnapshot {
            identity: self.current.identity,
            revision,
            content: Arc::new(content),
        };
        Ok(true)
    }
    /// Restore preceding authored content and publish a fresh revision, without raster copies.
    /// # Errors
    /// Rejects exhausted revision identity without changing history.
    pub fn undo(&mut self) -> Result<bool, CanvasError> {
        if self.undo.is_empty() {
            return Ok(false);
        }
        let revision = self.next_revision()?;
        let content = self.undo.pop().ok_or(CanvasError::Invalid("Missing undo state"))?;
        push_history(&mut self.redo, Arc::clone(&self.current.content));
        self.current = DocumentSnapshot { identity: self.current.identity, revision, content };
        Ok(true)
    }
    /// Restore a previously undone edit, publishing a fresh revision.
    /// # Errors
    /// Rejects exhausted revision identity without changing history.
    pub fn redo(&mut self) -> Result<bool, CanvasError> {
        if self.redo.is_empty() {
            return Ok(false);
        }
        let revision = self.next_revision()?;
        let content = self.redo.pop().ok_or(CanvasError::Invalid("Missing redo state"))?;
        push_history(&mut self.undo, Arc::clone(&self.current.content));
        self.current = DocumentSnapshot { identity: self.current.identity, revision, content };
        Ok(true)
    }
    fn next_revision(&self) -> Result<u64, CanvasError> {
        self.current
            .revision
            .checked_add(1)
            .ok_or(CanvasError::Invalid("Document revision exhausted"))
    }
}
fn find(layers: &[Layer], id: LayerId) -> Result<usize, CanvasError> {
    layers
        .iter()
        .position(|layer| layer.id == id)
        .ok_or(CanvasError::Invalid("Unknown layer identity"))
}
fn validate_layer(layer: Layer, extent: Extent) -> Result<(), CanvasError> {
    if let LayerContent::Raster { extent: source, .. } = layer.content
        && source != extent
    {
        return Err(CanvasError::Invalid("Raster layer extent differs from document extent"));
    }
    if let Some(LayerMask::Raster { extent: source, .. }) = layer.mask
        && source != extent
    {
        return Err(CanvasError::Invalid("Raster mask extent differs from document extent"));
    }
    Ok(())
}
fn push_history(history: &mut Vec<Arc<Content>>, content: Arc<Content>) {
    if history.len() == HISTORY_LIMIT {
        history.remove(0);
    }
    history.push(content);
}
