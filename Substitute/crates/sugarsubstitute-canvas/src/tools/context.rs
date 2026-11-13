// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Give tools scoped navigation and transactional commands without exporting the document owner.
use super::*;
use sugarsubstitute_contracts::execution::JobContext;

/// Borrowed capability for one synchronous tool observation; stores no window or scheduler handle.
pub struct ToolContext<'a> {
    pub(super) navigation: &'a mut Navigation,
    pub(super) document: Option<&'a mut Document>,
    pub(super) edit: &'a mut Option<EditSession>,
    pub(super) next_edit: &'a mut u64,
    pub(super) capture: &'a mut bool,
    pub(super) target: Option<LayerId>,
    pub(super) deferred: &'a mut Option<DeferredEdit>,
    pub(super) next_request: &'a mut u64,
    pub(super) previews: &'a mut PreviewOrder,
    pub(super) settled: &'a mut bool,
    pub(super) now: Duration,
}
/// Which deferred previews may still be staged: only requests newer than `floor`, and the
/// request whose result is staged now.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct PreviewOrder {
    pub(super) floor: u64,
    pub(super) staged: Option<u64>,
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
        self.previews.floor = *self.next_request;
        self.previews.staged = None;
        result
    }
    /// Cancel the active edit without changing document content, revision or history. Previews
    /// requested before now are obsolete and never staged; requested commits still apply.
    pub fn cancel_edit(&mut self) {
        drop(self.edit.take());
        *self.capture = false;
        self.previews.floor = *self.next_request;
        self.previews.staged = None;
        if self.deferred.as_ref().is_some_and(|work| work.disposition() == Disposition::Preview) {
            *self.deferred = None;
        }
    }
    /// The order of the latest deferred request, such as the preview [`Self::defer`] just made.
    pub const fn latest_request(&self) -> u64 {
        *self.next_request
    }
    /// The request whose preview is staged now against the document's current revision, when
    /// one is; a preview of an older revision cannot be published as it stands.
    pub fn staged_request(&self) -> Option<u64> {
        let (Some(edit), Some(document)) = (self.edit.as_ref(), self.document.as_deref()) else {
            return None;
        };
        let (preview, current) = (edit.preview(), document.snapshot());
        let base = preview.base();
        ((base.identity(), base.revision()) == (current.identity(), current.revision()))
            .then_some(self.previews.staged)
            .flatten()
    }
    /// Ask for the previously selected tool once this observation or command settles, as
    /// the Transform toolbar's Apply and Cancel return to the tool it was opened from.
    pub fn settle(&mut self) {
        *self.settled = true;
    }
    /// The layer editing tools act on, such as the active mask.
    pub const fn target(&self) -> Option<LayerId> {
        self.target
    }
    /// The current authored revision, when a document is loaded.
    pub fn document(&self) -> Option<DocumentSnapshot> {
        self.document.as_deref().map(Document::snapshot)
    }
    /// The execution admission that edits of this document charge, for bounded coverage edits
    /// such as brush dabs.
    /// # Errors
    /// Rejects a missing document.
    pub fn memory(&self) -> Result<&MemoryLease, CanvasError> {
        self.document
            .as_deref()
            .map(Document::memory)
            .ok_or(CanvasError::Invalid("No editable document"))
    }
    /// Request coverage work too large for the input thread. The host runs it as an owned job
    /// against the current revision; a newer request replaces one not yet taken. The work may
    /// run again against a newer revision when the document changes while it runs.
    /// # Errors
    /// Rejects a missing document.
    pub fn defer(
        &mut self,
        disposition: Disposition,
        work: impl Fn(&DocumentSnapshot, &JobContext) -> Result<DocumentEdit, CanvasError>
        + Send
        + 'static,
    ) -> Result<(), CanvasError> {
        *self.deferred = Some(deferred::request(
            self.document.as_deref(),
            disposition,
            self.next_request,
            Box::new(work),
        )?);
        Ok(())
    }
    /// Observe transient geometry with its gesture and base-revision identities.
    pub fn preview(&self) -> Option<EditPreview<'_>> {
        self.edit.as_ref().map(EditSession::preview)
    }
}
