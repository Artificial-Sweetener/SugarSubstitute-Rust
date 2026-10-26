// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Give tools scoped navigation and transactional commands without exporting the document owner.
use super::*;

/// Borrowed capability for one synchronous tool observation; stores no window or scheduler handle.
pub struct ToolContext<'a> {
    pub(super) navigation: &'a mut Navigation,
    pub(super) document: Option<&'a mut Document>,
    pub(super) edit: &'a mut Option<EditSession>,
    pub(super) next_edit: &'a mut u64,
    pub(super) capture: &'a mut bool,
    pub(super) now: Duration,
}
impl ToolContext<'_> {
    /// Request/release pointer capture for this tool's gesture; only the desktop owns native effects.
    pub fn capture_pointer(&mut self, active: bool) {
        *self.capture = active;
    }
    /// Observe the current shared camera and navigation policy.
    pub const fn navigation(&self) -> &Navigation {
        self.navigation
    }
    /// Submit normalized navigation through its authoritative rules at this observation's time.
    /// # Errors
    /// Preserves invalid-input, geometry and monotonic-time failures.
    pub fn navigate(&mut self, input: NavigationInput) -> Result<(), CanvasError> {
        self.navigation.input(input, self.now)
    }
    /// Start one owned edit against the current authored revision.
    /// # Errors
    /// Rejects missing documents, overlapping gestures or exhausted identity before changing state.
    pub fn begin_edit(&mut self) -> Result<EditSessionId, CanvasError> {
        if self.edit.is_some() {
            return Err(CanvasError::Invalid("Tool already owns an edit session"));
        }
        let doc = self.document.as_deref().ok_or(CanvasError::Invalid("No editable document"))?;
        let next =
            self.next_edit.checked_add(1).ok_or(CanvasError::Invalid("Edit identity exhausted"))?;
        let id = EditSessionId::new(next)?;
        *self.edit = Some(doc.begin_edit(id));
        *self.next_edit = next;
        Ok(id)
    }
    /// Stage a validated layer command without changing authored history.
    /// # Errors
    /// Rejects absent gestures and preserves validation/admission causes and the preceding preview.
    pub fn stage(&mut self, edit: DocumentEdit) -> Result<(), CanvasError> {
        self.edit.as_mut().ok_or(CanvasError::Invalid("Tool has no edit session"))?.stage(edit)
    }
    /// Publish the active gesture as one document/history transition.
    /// # Errors
    /// Rejects missing, foreign or stale custody without changing authored content.
    pub fn commit_edit(&mut self) -> Result<bool, CanvasError> {
        let doc =
            self.document.as_deref_mut().ok_or(CanvasError::Invalid("No editable document"))?;
        let edit = self.edit.take().ok_or(CanvasError::Invalid("Tool has no edit session"))?;
        let result = doc.commit_edit(edit);
        *self.capture = false;
        result
    }
    /// Cancel the active edit without changing document content, revision or history.
    pub fn cancel_edit(&mut self) {
        drop(self.edit.take());
        *self.capture = false;
    }
    /// Observe transient geometry with its gesture and base-revision identities.
    pub fn preview(&self) -> Option<EditPreview<'_>> {
        self.edit.as_ref().map(EditSession::preview)
    }
}
