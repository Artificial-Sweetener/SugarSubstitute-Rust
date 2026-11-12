// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Change a document's structure on behalf of its host rather than through an editing gesture.
//!
//! A workflow decides which images and masks an input document holds. Those decisions are not
//! edits a person can undo: undo must never remove a mask the workflow provides or bring back a
//! base image the workflow replaced. Provisioning therefore rewrites every history state along
//! with the current content, and a wholesale reload discards history it can no longer honour.
//! Canvas geometry changes, by contrast, are ordinary undoable steps.
use super::*;
use sugarsubstitute_image_ops::ImageError;

/// One host-driven structural change, applied to the current content and every history state.
#[derive(Clone, Debug)]
pub enum Provision {
    /// Insert a layer where its identity is absent, at `index` or the top of shorter stacks.
    Insert {
        /// Destination in bottom-to-top order.
        index: usize,
        /// Complete layer value with a new identity.
        layer: Layer,
    },
    /// Remove a layer wherever it is present.
    Remove(LayerId),
    /// Rename a layer wherever it is present.
    Rename {
        /// Renamed layer.
        layer: LayerId,
        /// New display name of at most 256 characters.
        name: Arc<str>,
    },
    /// Change a mask layer's overlay appearance wherever it is present.
    Appearance {
        /// Mask layer.
        layer: LayerId,
        /// New tint and opacity.
        appearance: MaskAppearance,
    },
    /// Replace a mask layer's coverage wherever it is present, keeping each state's appearance,
    /// as when a mask's file is loaded.
    Coverage {
        /// Mask layer.
        layer: LayerId,
        /// New coverage at the document's extent.
        coverage: Coverage,
    },
    /// Replace a layer's content wherever it is present, keeping its other properties.
    Content {
        /// Changed layer.
        layer: LayerId,
        /// New content valid at the document's extent.
        content: LayerContent,
    },
    /// Move a layer to `index`, or the top of shorter stacks, wherever it is present.
    Reorder {
        /// Moved layer.
        layer: LayerId,
        /// Destination in the resulting bottom-to-top order.
        index: usize,
    },
}

impl Document {
    /// Apply host-driven changes to the current content and every undo and redo state,
    /// publishing a new revision without a history step. A history state that cannot take a
    /// change, such as one at another extent, is discarded together with every state beyond
    /// it, so each reachable state holds the provisioned structure.
    /// # Errors
    /// Rejects changes the current content cannot take, with nothing changed; preserves
    /// admission failures and exhausted revision identity.
    pub fn provision(&mut self, changes: &[Provision]) -> Result<bool, CanvasError> {
        let revision = self.next_revision()?;
        let mut current = Content::copy(&self.current.content, &self.memory)?;
        for change in changes {
            current.provision(change)?;
        }
        if current.same(&self.current.content) {
            return Ok(false);
        }
        let undo = rewrite(&self.undo, changes, &self.memory)?;
        let redo = rewrite(&self.redo, changes, &self.memory)?;
        self.undo = undo;
        self.redo = redo;
        self.current = self.published(Arc::new(current), revision);
        Ok(true)
    }

    /// Replace the whole content, including its extent, and discard history that could only
    /// restore content the host no longer provides. The selection is cleared.
    /// # Errors
    /// Rejects duplicate identities, too many layers and layers invalid at `extent`, with
    /// nothing changed; preserves admission failures and exhausted revision identity.
    pub fn reload(&mut self, extent: Extent, layers: &[Layer]) -> Result<bool, CanvasError> {
        let revision = self.next_revision()?;
        let content = validated(extent, layers, &self.memory)?;
        if content.same(&self.current.content) {
            return Ok(false);
        }
        let mut undo = Vec::new();
        let mut redo = Vec::new();
        undo.try_reserve_exact(HISTORY_LIMIT).map_err(ImageError::from)?;
        redo.try_reserve_exact(HISTORY_LIMIT).map_err(ImageError::from)?;
        self.undo = undo;
        self.redo = redo;
        self.current = self.published(Arc::new(content), revision);
        Ok(true)
    }

    /// Change the extent and replace every layer as one undoable step, as resizing or scaling
    /// a canvas does. The selection is cleared.
    /// # Errors
    /// Rejects duplicate identities, too many layers and layers invalid at `extent`, with
    /// nothing changed; preserves admission failures and exhausted revision identity.
    pub fn reframe(&mut self, extent: Extent, layers: &[Layer]) -> Result<bool, CanvasError> {
        let revision = self.next_revision()?;
        let content = validated(extent, layers, &self.memory)?;
        Ok(self.publish_content(Arc::new(content), revision))
    }
}

/// A complete content value at `extent`, validated as a sequence of insertions.
fn validated(
    extent: Extent,
    layers: &[Layer],
    memory: &MemoryLease,
) -> Result<Content, CanvasError> {
    if layers.len() > MAX_LAYERS {
        return Err(CanvasError::Invalid("Too many layers"));
    }
    let mut content = Content::new(extent, &[], None, memory)?;
    for (index, layer) in layers.iter().enumerate() {
        content.apply(DocumentEdit::Insert { index, layer: layer.clone() })?;
    }
    Ok(content)
}

/// Rewrite one history stack, whose last state is the one nearest the current content.
/// Walking outward, the first state that cannot take the changes is discarded with every
/// state beyond it.
fn rewrite(
    history: &[Arc<Content>],
    changes: &[Provision],
    memory: &MemoryLease,
) -> Result<Vec<Arc<Content>>, CanvasError> {
    let mut kept = Vec::new();
    kept.try_reserve_exact(HISTORY_LIMIT).map_err(ImageError::from)?;
    for state in history.iter().rev() {
        let mut content = Content::copy(state, memory)?;
        if changes.iter().try_for_each(|change| content.provision(change)).is_err() {
            break;
        }
        kept.push(Arc::new(content));
    }
    kept.reverse();
    Ok(kept)
}

impl Content {
    /// Apply one provisioning change with its lenient placement rules.
    fn provision(&mut self, change: &Provision) -> Result<(), CanvasError> {
        let position =
            |layers: &[Layer], id: LayerId| layers.iter().position(|layer| layer.id == id);
        match change {
            Provision::Insert { index, layer } => {
                if self.contains(layer.id) {
                    return Ok(());
                }
                let index = (*index).min(self.layers.len());
                self.apply(DocumentEdit::Insert { index, layer: layer.clone() })
            }
            Provision::Remove(id) => {
                if let Some(index) = position(&self.layers, *id) {
                    self.layers.remove(index);
                }
                Ok(())
            }
            Provision::Rename { layer, name } => {
                self.change(*layer, |current| current.with_name(name))
            }
            Provision::Appearance { layer, appearance } => self.change(*layer, |current| {
                let LayerContent::Coverage { coverage, .. } = &current.content else {
                    return Err(CanvasError::Invalid("Appearance belongs to mask layers"));
                };
                let content =
                    LayerContent::Coverage { coverage: coverage.clone(), appearance: *appearance };
                Ok(current.with_content(content))
            }),
            Provision::Coverage { layer, coverage } => self.change(*layer, |current| {
                let LayerContent::Coverage { appearance, .. } = &current.content else {
                    return Err(CanvasError::Invalid("Coverage belongs to mask layers"));
                };
                let content =
                    LayerContent::Coverage { coverage: coverage.clone(), appearance: *appearance };
                Ok(current.with_content(content))
            }),
            Provision::Content { layer, content } => {
                self.change(*layer, |current| Ok(current.with_content(content.clone())))
            }
            Provision::Reorder { layer, index } => {
                let Some(original) = position(&self.layers, *layer) else {
                    return Ok(());
                };
                let moved = self.layers.remove(original);
                let index = (*index).min(self.layers.len());
                self.layers.insert(index, moved);
                Ok(())
            }
        }
    }

    /// Replace a present layer with a validated changed copy; absent layers are left alone.
    fn change(
        &mut self,
        id: LayerId,
        change: impl FnOnce(Layer) -> Result<Layer, CanvasError>,
    ) -> Result<(), CanvasError> {
        let Some(index) = self.layers.iter().position(|layer| layer.id == id) else {
            return Ok(());
        };
        let changed = change(self.layers[index].clone())?;
        validate_layer(&changed, self.extent)?;
        self.layers[index] = changed;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
