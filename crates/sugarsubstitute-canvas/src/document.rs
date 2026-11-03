// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Revisioned document metadata and reversible commands sharing original image assets.
mod damage;
mod edit;
mod highlight;
mod provision;
use crate::CanvasError;
pub use edit::{EditPreview, EditSession, EditSessionId};
pub use provision::Provision;
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};
use sugarsubstitute_image_ops::{AssetId, Coverage, Extent, VectorRectangle};

pub(crate) const MAX_LAYERS: usize = 32;
const HISTORY_LIMIT: usize = 32;
/// Longest layer name, in characters, matching what the canvas protocol carries.
const MAX_NAME_CHARACTERS: usize = 256;

/// Stable positive layer identity, preserved across reordering and history traversal.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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

/// How a mask layer is drawn over the image: a tint at partial opacity. Appearance never
/// changes the mask's coverage or what a workflow receives from it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaskAppearance {
    color: [u8; 3],
    opacity: f32,
}
impl MaskAppearance {
    /// Tint as sRGB bytes and its overlay opacity.
    /// # Errors
    /// Rejects an opacity that is nonfinite or outside `[0, 1]`.
    pub fn new(color: [u8; 3], opacity: f32) -> Result<Self, CanvasError> {
        if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
            return Err(CanvasError::Invalid("Invalid mask overlay opacity"));
        }
        Ok(Self { color, opacity })
    }
    /// Tint as sRGB bytes.
    pub const fn color(self) -> [u8; 3] {
        self.color
    }
    /// Overlay opacity in `[0, 1]`.
    pub const fn opacity(self) -> f32 {
        self.opacity
    }
}

/// Authoritative layer content; pixels, coverage and vector geometry remain independently
/// editable. Cloning shares coverage tiles.
#[derive(Clone, Debug, PartialEq)]
pub enum LayerContent {
    /// Immutable original image asset at the document's pixel extent.
    Raster {
        /// Content-bound asset identity.
        asset: AssetId,
        /// Original pixel dimensions.
        extent: Extent,
    },
    /// Immutable original image stretched into a document-space rectangle, such as one cell of
    /// a composed overview; the asset keeps its own pixel extent.
    PlacedRaster {
        /// Content-bound asset identity.
        asset: AssetId,
        /// Original pixel dimensions.
        extent: Extent,
        /// Document-space `[left, top, width, height]`; width and height are positive.
        bounds: [f64; 4],
    },
    /// Continuous document-space geometry, never replaced by a rasterized cache entry.
    VectorRectangle(VectorRectangle),
    /// Non-destructive linear-light exposure of the composite beneath this layer.
    Exposure {
        /// Exposure in EV, finite and between -32 and 32.
        stops: f32,
    },
    /// A mask: coverage at the document extent, shown as a tinted overlay and never part of
    /// the composed image.
    Coverage {
        /// The mask's coverage.
        coverage: Coverage,
        /// How the overlay is drawn.
        appearance: MaskAppearance,
    },
}

/// Immutable layer description. Changes are published only through document commands.
/// Cloning shares mask coverage tiles.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    id: LayerId,
    name: Arc<str>,
    content: LayerContent,
    visible: bool,
    locked: bool,
    opacity: f32,
    mask: Option<Coverage>,
}
impl Layer {
    /// Create an unnamed, unlocked, opaque visible layer with explicit source content.
    /// # Errors
    /// Rejects nonfinite or unsupported operation parameters.
    pub fn new(id: LayerId, content: LayerContent) -> Result<Self, CanvasError> {
        if let LayerContent::Exposure { stops } = content
            && (!stops.is_finite() || !(-32.0..=32.0).contains(&stops))
        {
            return Err(CanvasError::Invalid("Invalid layer exposure"));
        }
        Ok(Self {
            id,
            name: Arc::from(""),
            content,
            visible: true,
            locked: false,
            opacity: 1.0,
            mask: None,
        })
    }
    /// Stable layer identity.
    pub const fn id(&self) -> LayerId {
        self.id
    }
    /// Display name shown to people choosing layers.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Original source geometry, image identity, coverage or adjustment parameters.
    pub const fn content(&self) -> &LayerContent {
        &self.content
    }
    /// Whether tools leave this layer alone: it cannot be selected, moved, painted, reordered
    /// or removed by editing gestures, as with an input canvas's base image.
    pub const fn locked(&self) -> bool {
        self.locked
    }
    /// Whether this layer contributes to the document's rendered result.
    pub const fn visible(&self) -> bool {
        self.visible
    }
    /// Finite layer coverage in `[0, 1]`.
    pub const fn opacity(&self) -> f32 {
        self.opacity
    }
    /// Layer mask coverage at the document extent, when the layer has one.
    pub const fn mask(&self) -> Option<&Coverage> {
        self.mask.as_ref()
    }
    /// Rename the layer.
    /// # Errors
    /// Rejects names longer than 256 characters.
    pub fn with_name(mut self, name: &str) -> Result<Self, CanvasError> {
        if name.chars().count() > MAX_NAME_CHARACTERS {
            return Err(CanvasError::Invalid("Layer name is too long"));
        }
        self.name = Arc::from(name);
        Ok(self)
    }
    /// Replace the layer's content, keeping its identity and other properties.
    pub fn with_content(mut self, content: LayerContent) -> Self {
        self.content = content;
        self
    }
    /// Lock or unlock the layer against editing gestures.
    pub fn with_locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        self
    }
    /// Change visibility on a value submitted through a document command.
    pub fn with_visibility(mut self, visible: bool) -> Self {
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
    /// Attach or remove a layer mask.
    pub fn with_mask(mut self, mask: Option<Coverage>) -> Self {
        self.mask = mask;
        self
    }
}

/// One atomic edit; rejected edits leave document revision and both history branches intact.
#[derive(Clone, Debug)]
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
    /// Replace the document's selection; `None` deselects.
    Select(Option<Coverage>),
    /// Apply edits in order as one atomic step, as floating pixels land on their mask while
    /// the selection moves with them; any rejected edit rejects the whole group.
    Group(Vec<DocumentEdit>),
}

struct Content {
    extent: Extent,
    layers: Vec<Layer>,
    selection: Option<Coverage>,
    _memory: MemoryClaim,
}
impl Content {
    fn copy(source: &Self, memory: &MemoryLease) -> Result<Self, CanvasError> {
        Self::new(source.extent, &source.layers, source.selection.clone(), memory)
    }
    fn new(
        extent: Extent,
        layers: &[Layer],
        selection: Option<Coverage>,
        memory: &MemoryLease,
    ) -> Result<Self, CanvasError> {
        let claim = memory
            .claim((MAX_LAYERS * size_of::<Layer>() + 128) as u64)
            .map_err(sugarsubstitute_image_ops::ImageError::from)?;
        let mut owned = Vec::new();
        owned.try_reserve_exact(MAX_LAYERS).map_err(sugarsubstitute_image_ops::ImageError::from)?;
        owned.extend_from_slice(layers);
        Ok(Self { extent, layers: owned, selection, _memory: claim })
    }
    /// Whether two contents hold the same extent, layers and selection.
    fn same(&self, other: &Self) -> bool {
        self.extent == other.extent
            && self.layers == other.layers
            && self.selection == other.selection
    }
    /// Whether a layer with this identity is present.
    fn contains(&self, id: LayerId) -> bool {
        self.layers.iter().any(|layer| layer.id == id)
    }
}

/// Immutable document content used by rendering and independent comparison views.
#[derive(Clone)]
pub struct DocumentSnapshot {
    identity: u64,
    revision: u64,
    stage: u64,
    content: Arc<Content>,
    highlight: Option<LayerId>,
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
    /// Zero for published content; a gesture's staged previews count up from one on top of
    /// their base revision, so each staged step renders as distinct content.
    pub const fn stage(&self) -> u64 {
        self.stage
    }
    /// Authored document pixel extent, independent of viewport bounds.
    pub fn extent(&self) -> Extent {
        self.content.extent
    }
    /// Immutable bottom-to-top layer stack, preserving raster sources and vector geometry.
    pub fn layers(&self) -> &[Layer] {
        &self.content.layers
    }
    /// The document's selection, when something is selected.
    pub fn selection(&self) -> Option<&Coverage> {
        self.content.selection.as_ref()
    }
    /// The mask layer whose overlay is drawn emphasized, as while a person hovers its row.
    /// Emphasis is presentation only: it is never part of history or of the authored content.
    pub const fn highlight(&self) -> Option<LayerId> {
        self.highlight
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
        let content = Arc::new(Content::new(extent, &[], None, memory)?);
        Ok(Self {
            current: DocumentSnapshot { identity, revision: 1, stage: 0, content, highlight: None },
            undo,
            redo,
            memory: memory.clone(),
            _history_memory: claim,
        })
    }
    /// Rebuild persisted content as the document's first revision, with empty history.
    /// # Errors
    /// Rejects zero identity, more than 32 layers, duplicate identities, layers or a selection
    /// that do not match `extent`, and insufficient memory/allocation capacity.
    pub fn restore(
        identity: u64,
        extent: Extent,
        layers: &[Layer],
        selection: Option<Coverage>,
        memory: &MemoryLease,
    ) -> Result<Self, CanvasError> {
        validate_stack(extent, layers, selection.as_ref())?;
        let mut document = Self::new(identity, extent, memory)?;
        document.current.content = Arc::new(Content::new(extent, layers, selection, memory)?);
        Ok(document)
    }
    /// Share the current immutable revision without copying layers or pixels.
    pub fn snapshot(&self) -> DocumentSnapshot {
        self.current.clone()
    }
    /// The execution admission that edits of this document charge.
    pub(crate) const fn memory(&self) -> &MemoryLease {
        &self.memory
    }
    /// Apply one validated edit and publish a new revision; no-op edits retain revision/history.
    /// # Errors
    /// Rejects missing/duplicate IDs, invalid indices, extent mismatches, limits and memory pressure.
    pub fn apply(&mut self, edit: DocumentEdit) -> Result<bool, CanvasError> {
        let revision = self.next_revision()?;
        let mut content = Content::copy(&self.current.content, &self.memory)?;
        content.apply(edit)?;
        Ok(self.publish_content(Arc::new(content), revision))
    }
    /// Publish one validated metadata branch through the sole history owner.
    fn publish_content(&mut self, content: Arc<Content>, revision: u64) -> bool {
        if content.same(&self.current.content) {
            return false;
        }
        push_history(&mut self.undo, Arc::clone(&self.current.content));
        self.redo.clear();
        self.current = self.published(content, revision);
        true
    }
    /// The next current snapshot: `content` at `revision`, keeping the highlight while its
    /// layer remains.
    fn published(&self, content: Arc<Content>, revision: u64) -> DocumentSnapshot {
        let highlight = self.current.highlight.filter(|layer| content.contains(*layer));
        DocumentSnapshot { identity: self.current.identity, revision, stage: 0, content, highlight }
    }
    /// Whether undo and redo each have a step to restore, as the editing UI reports them.
    pub fn history(&self) -> [bool; 2] {
        [!self.undo.is_empty(), !self.redo.is_empty()]
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
        self.current = self.published(content, revision);
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
        self.current = self.published(content, revision);
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
/// Check a complete bottom-to-top stack and selection against the rules every edit keeps.
pub(crate) fn validate_stack(
    extent: Extent,
    layers: &[Layer],
    selection: Option<&Coverage>,
) -> Result<(), CanvasError> {
    let duplicate =
        |(index, layer): (usize, &Layer)| layers[..index].iter().any(|old| old.id == layer.id);
    if layers.len() > MAX_LAYERS || layers.iter().enumerate().any(duplicate) {
        return Err(CanvasError::Invalid("Too many layers or a duplicate layer identity"));
    }
    layers.iter().try_for_each(|layer| validate_layer(layer, extent))?;
    validate_selection(selection, extent)
}
/// A selection lies on the document: it spans its extent and keeps nothing beyond it.
fn validate_selection(selection: Option<&Coverage>, extent: Extent) -> Result<(), CanvasError> {
    if selection.is_some_and(|selection| selection.extent() != extent) {
        return Err(CanvasError::Invalid("Selection extent differs from document"));
    }
    if selection.is_some_and(|selection| selection.outside().is_some()) {
        return Err(CanvasError::Invalid("Selection keeps content beyond the document"));
    }
    Ok(())
}
fn validate_layer(layer: &Layer, extent: Extent) -> Result<(), CanvasError> {
    if let LayerContent::Raster { extent: source, .. } = &layer.content
        && *source != extent
    {
        return Err(CanvasError::Invalid("Raster layer extent differs from document extent"));
    }
    if let LayerContent::PlacedRaster { bounds: [left, top, width, height], .. } = layer.content
        && !([left, top, width, height].iter().all(|value| value.is_finite())
            && width > 0.0
            && height > 0.0)
    {
        return Err(CanvasError::Invalid("Placed raster bounds must be finite and nonempty"));
    }
    if layer.mask.as_ref().is_some_and(|mask| mask.extent() != extent) {
        return Err(CanvasError::Invalid("Layer mask extent differs from document extent"));
    }
    if let LayerContent::Coverage { coverage, .. } = &layer.content
        && coverage.extent() != extent
    {
        return Err(CanvasError::Invalid("Mask layer extent differs from document extent"));
    }
    Ok(())
}
fn push_history(history: &mut Vec<Arc<Content>>, content: Arc<Content>) {
    if history.len() == HISTORY_LIMIT {
        history.remove(0);
    }
    history.push(content);
}
