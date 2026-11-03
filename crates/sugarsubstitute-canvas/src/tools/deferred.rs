// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coverage work a tool hands to the execution owner instead of running on the input thread.
//!
//! Rasterizing a large selection, or expanding, contracting and feathering a whole mask, can take
//! long enough to stall input. A tool describes such work as a [`DeferredEdit`]; the host runs it
//! as an owned job against the snapshot it was requested for and hands the result back. The result
//! applies only while the document still shows that revision; anything else is stale and dropped.
use crate::{CanvasError, Document, DocumentEdit, DocumentSnapshot};
use sugarsubstitute_contracts::execution::JobContext;

/// What a finished deferred edit does to the document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Disposition {
    /// Publish as one undo step.
    Commit,
    /// Stage in the tool's edit session for a live preview, replacing any earlier preview; the
    /// tool commits or cancels it later.
    Preview,
}

pub(super) type Work =
    Box<dyn FnOnce(&DocumentSnapshot, &JobContext) -> Result<DocumentEdit, CanvasError> + Send>;

/// Pending work and the exact revision it was requested against.
pub struct DeferredEdit {
    base: DocumentSnapshot,
    disposition: Disposition,
    request: u64,
    work: Work,
}

/// A computed edit waiting to be applied on the input thread.
pub struct PreparedEdit {
    base: (u64, u64),
    disposition: Disposition,
    request: u64,
    edit: DocumentEdit,
}

impl DeferredEdit {
    pub(super) fn new(
        base: DocumentSnapshot,
        disposition: Disposition,
        request: u64,
        work: Work,
    ) -> Self {
        Self { base, disposition, request, work }
    }
    /// The revision this work reads.
    pub const fn base(&self) -> &DocumentSnapshot {
        &self.base
    }
    /// Run the work inside an owned execution job.
    /// # Errors
    /// Preserves the work's cancellation, deadline, memory and validation failures.
    pub fn run(self, context: &JobContext) -> Result<PreparedEdit, CanvasError> {
        let edit = (self.work)(&self.base, context)?;
        Ok(PreparedEdit {
            base: (self.base.identity(), self.base.revision()),
            disposition: self.disposition,
            request: self.request,
            edit,
        })
    }
}

impl PreparedEdit {
    /// Whether the document still shows the revision this edit was computed from.
    pub fn current(&self, document: &Document) -> bool {
        let snapshot = document.snapshot();
        self.base == (snapshot.identity(), snapshot.revision())
    }
    pub(super) const fn disposition(&self) -> Disposition {
        self.disposition
    }
    /// The manager-assigned order of the request that produced this edit.
    pub(super) const fn request(&self) -> u64 {
        self.request
    }
    pub(super) fn into_edit(self) -> DocumentEdit {
        self.edit
    }
}

impl std::fmt::Debug for DeferredEdit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeferredEdit")
            .field("base", &(self.base.identity(), self.base.revision()))
            .field("disposition", &self.disposition)
            .finish_non_exhaustive()
    }
}

/// Package work against the loaded document's current revision under the next request order.
pub(super) fn request(
    document: Option<&Document>,
    disposition: Disposition,
    next_request: &mut u64,
    work: Work,
) -> Result<DeferredEdit, CanvasError> {
    let base =
        document.map(Document::snapshot).ok_or(CanvasError::Invalid("No editable document"))?;
    let request = next_request
        .checked_add(1)
        .ok_or(CanvasError::Invalid("Deferred request order exhausted"))?;
    *next_request = request;
    Ok(DeferredEdit::new(base, disposition, request, work))
}
