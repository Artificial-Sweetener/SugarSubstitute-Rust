// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Stage gesture edits through the document's own validation and publish one history transition.
use super::*;

/// Positive application-assigned identity for one transient edit gesture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EditSessionId(u64);
impl EditSessionId {
    /// Validate a gesture identity; the application must not reuse it within a document lifetime.
    /// # Errors
    /// Rejects zero, which never identifies an active gesture.
    pub fn new(value: u64) -> Result<Self, CanvasError> {
        if value == 0 {
            return Err(CanvasError::Invalid("Zero edit session identity"));
        }
        Ok(Self(value))
    }
    /// Original application-assigned gesture identity.
    pub const fn get(self) -> u64 {
        self.0
    }
}
/// Borrowed transient geometry with explicit base revision and gesture identity.
/// It is not an authored document snapshot and must not be published as that base's exact frame.
pub struct EditPreview<'a> {
    session: EditSessionId,
    base: &'a DocumentSnapshot,
    layers: &'a [Layer],
}
impl EditPreview<'_> {
    /// Gesture whose transient state produced this preview.
    pub const fn session(&self) -> EditSessionId {
        self.session
    }
    /// Immutable authored revision against which this gesture was admitted.
    pub const fn base(&self) -> &DocumentSnapshot {
        self.base
    }
    /// Staged bottom-to-top layer metadata; original pixels remain shared immutable assets.
    pub const fn layers(&self) -> &[Layer] {
        self.layers
    }
}
/// One document-bound, execution-admitted transient layer edit.
/// Stage validates atomically; dropping cancels with no authored or history change.
/// Commit consumes custody and creates at most one undo entry, regardless of staged updates.
pub struct EditSession {
    id: EditSessionId,
    base: DocumentSnapshot,
    staged: Option<Content>,
    memory: MemoryLease,
}
impl EditSession {
    /// Stable identity of the gesture.
    pub const fn id(&self) -> EditSessionId {
        self.id
    }
    /// Observe staged metadata with its separate preview identity.
    pub fn preview(&self) -> EditPreview<'_> {
        EditPreview {
            session: self.id,
            base: &self.base,
            layers: self.staged.as_ref().map_or(self.base.layers(), |content| &content.layers),
        }
    }
    /// Apply one complete layer command to the transient branch using authoritative validation.
    /// # Errors
    /// Preserves the preceding preview on invalid commands, allocation or admission failures.
    pub fn stage(&mut self, edit: DocumentEdit) -> Result<(), CanvasError> {
        let mut content = Content::copy(self.base.extent(), self.preview().layers(), &self.memory)?;
        content.apply(edit)?;
        self.staged = Some(content);
        Ok(())
    }
}
impl Document {
    /// Begin a gesture against the exact current immutable revision, sharing original admission.
    pub fn begin_edit(&self, id: EditSessionId) -> EditSession {
        EditSession { id, base: self.snapshot(), staged: None, memory: self.memory.clone() }
    }
    /// Publish a gesture only while its original document and revision remain current.
    /// # Errors
    /// Rejects stale/foreign gestures and revision exhaustion before changing state or history.
    pub fn commit_edit(&mut self, session: EditSession) -> Result<bool, CanvasError> {
        if session.base.identity != self.current.identity
            || session.base.revision != self.current.revision
            || !Arc::ptr_eq(&session.base.content, &self.current.content)
        {
            return Err(CanvasError::Invalid("Edit session no longer matches its document"));
        }
        let Some(content) = session.staged else {
            return Ok(false);
        };
        if content.layers == self.current.content.layers {
            return Ok(false);
        }
        let revision = self.next_revision()?;
        Ok(self.publish_content(content, revision))
    }
}
impl Content {
    /// One validation owner for immediate commands and transient gesture branches.
    pub(super) fn apply(&mut self, edit: DocumentEdit) -> Result<(), CanvasError> {
        match edit {
            DocumentEdit::Insert { index, layer } => {
                if self.layers.len() == MAX_LAYERS
                    || index > self.layers.len()
                    || self.layers.iter().any(|old| old.id == layer.id)
                {
                    return Err(CanvasError::Invalid("Invalid layer insertion"));
                }
                validate_layer(layer, self.extent)?;
                self.layers.insert(index, layer);
            }
            DocumentEdit::Remove(id) => {
                let index = find(&self.layers, id)?;
                self.layers.remove(index);
            }
            DocumentEdit::Replace(layer) => {
                let index = find(&self.layers, layer.id)?;
                validate_layer(layer, self.extent)?;
                self.layers[index] = layer;
            }
            DocumentEdit::Reorder { layer, index } => {
                if index >= self.layers.len() {
                    return Err(CanvasError::Invalid("Invalid layer destination"));
                }
                let original = find(&self.layers, layer)?;
                let layer = self.layers.remove(original);
                self.layers.insert(index, layer);
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests;
