// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Copy and stage the shown original for an OS drag-out through the existing execution scope.
//!
//! A storage worker reads the retained original, charges the copy to the canvas memory lease
//! and stages it as a real file, so the UI thread only hands completed results to the desktop's
//! drag source. The latest staged copies stay pinned because a drop target may read after the
//! drag ends; older ones are released on the worker of the next drag. The same worker decodes
//! the drag image, so the UI thread only fits an already small preview to its monitor.

use super::*;
use crate::native_canvas::drag_staging::{self, STAGED_DRAGS, StagedDrag};
use std::{collections::VecDeque, path::PathBuf};
use sugarsubstitute_desktop::canvas::{DRAG_PREVIEW_CEILING, DragPreview};

/// Largest original handed to an OS drag; larger outputs stay in Comfy's storage.
pub(crate) const DRAG_OUT_BYTES: u64 = 128 * 1024 * 1024;
/// Worker admission for the cache's traversal, protection and entry metadata while staging.
const DRAG_OUT_WORK_BYTES: u64 = 16 * 1024 * 1024;

/// Complete original bytes for one drag-out request and the claim that covers them.
pub(crate) struct DragOutBytes {
    /// The caller's request identity, so a superseded drag never reuses these bytes.
    pub(crate) request: u64,
    pub(crate) bytes: Vec<u8>,
    pub(crate) claim: MemoryClaim,
    /// The staged file named after the original, when staging succeeded.
    pub(crate) staged: Option<PathBuf>,
    /// The decoded drag image, when the original could be decoded within its admission.
    pub(crate) preview: Option<DragPreview>,
}

/// One worker result: the copy and its pinned staged file.
pub(super) struct Copied {
    request: u64,
    bytes: Vec<u8>,
    claim: MemoryClaim,
    staged: Option<StagedDrag>,
    preview: Option<DragPreview>,
}

/// At most one copy in flight, one completed copy awaiting the desktop, and the pins of the
/// most recent staged drags.
#[derive(Default)]
pub(super) struct DragOut {
    job: Option<OwnedJob<Copied, WorkError>>,
    ready: Option<DragOutBytes>,
    staged: VecDeque<StagedDrag>,
}

impl DragOut {
    /// Whether a copy still awaits its observed terminal outcome.
    pub(super) const fn in_flight(&self) -> bool {
        self.job.is_some()
    }
}

impl Session {
    /// Whether a primary drag on the shown content exports it instead of panning.
    pub(crate) fn allows_drag_out(&self) -> bool {
        self.document.scene.as_ref().is_some_and(|scene| scene.original.is_some())
            && self.document.tools.allows_drag_out()
    }

    /// Start copying and staging the shown original as `filename` for drag-out `request`.
    ///
    /// Returns false without submitting when closing, when no original is shown or while an
    /// earlier copy is in flight; completed bytes for an older request are discarded.
    ///
    /// # Errors
    /// Preserves identity, deadline and non-retryable admission failures.
    pub(crate) fn request_drag_out(
        &mut self,
        request: u64,
        filename: String,
    ) -> Result<bool, Box<dyn Error>> {
        self.drag_out.ready = None;
        let original = self.document.scene.as_ref().and_then(|scene| scene.original.clone());
        let (Some(original), false, false) = (original, self.closing, self.drag_out.in_flight())
        else {
            return Ok(false);
        };
        let resources = self.resources.as_ref().ok_or("Canvas storage is unavailable")?;
        let (memory, cache) = (resources.memory.clone(), resources.cache.clone());
        let spec = self.storage_spec(
            Operation::Infrastructure,
            JobClass::InfrastructureOrdinary,
            DRAG_OUT_WORK_BYTES,
            None,
        )?;
        let evicted = (self.drag_out.staged.len() >= STAGED_DRAGS)
            .then(|| self.drag_out.staged.pop_front())
            .flatten();
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        match runtime.client.submit_blocking(&runtime.scope, spec, move |context| {
            // Releasing the oldest pin lets this drag's own admission remove its file.
            drop(evicted);
            let (bytes, claim) = original
                .encoded_bytes(DRAG_OUT_BYTES, &memory, &context)
                .map_err(WorkError::new)?;
            let staged = sugarsubstitute_supervision::cache::utc_seconds()
                .and_then(|now| {
                    drag_staging::stage(cache, request, &filename, &bytes, now, &context)
                })
                .inspect_err(|cause| {
                    tracing::warn!(operation = "drag_out_staging", cause = %cause,
                        "The drag-out offers no file path; drops that need one are refused");
                })
                .ok();
            let preview = DragPreview::decode(&bytes, DRAG_PREVIEW_CEILING, &memory)
                .inspect_err(|cause| {
                    tracing::warn!(operation = "drag_out_preview", cause = %cause,
                        "The drag-out shows no preview of its output");
                })
                .ok();
            Ok(Copied { request, bytes, claim, staged, preview })
        }) {
            Ok(job) => {
                self.drag_out.job = Some(job);
                Ok(true)
            }
            Err(cause) if activity::retryable(&cause) => Ok(false),
            Err(cause) => Err(cause.into()),
        }
    }

    /// Take completed bytes for exactly `request`; bytes for any other request are discarded.
    pub(crate) fn take_drag_out(&mut self, request: u64) -> Option<DragOutBytes> {
        self.drag_out.ready.take().filter(|ready| ready.request == request)
    }

    /// Observe a completed copy, retaining its staged pin; a failed copy loses only that drag.
    ///
    /// # Errors
    /// Preserves worker retirement failures for a pin observed while closing.
    pub(super) fn consume_drag_out(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(outcome) = Self::take(&mut self.drag_out.job) else { return Ok(()) };
        match Self::terminal(outcome) {
            Ok(Copied { staged, .. }) if self.closing => {
                self.retire(Retired {
                    _drag_staging: staged.into_iter().collect(),
                    ..Default::default()
                })?;
            }
            Ok(Copied { request, bytes, claim, staged, preview }) => {
                let path = staged.as_ref().map(|staged| staged.path.clone());
                self.drag_out.staged.extend(staged);
                self.drag_out.ready =
                    Some(DragOutBytes { request, bytes, claim, staged: path, preview });
            }
            Err(cause) => {
                tracing::warn!(operation = "canvas_drag_out_copy", cause = %cause,
                    "The shown output could not be prepared for drag-out");
            }
        }
        Ok(())
    }

    /// Cancel an in-flight copy and hand staged pins to worker retirement when closing.
    ///
    /// # Errors
    /// Preserves worker retirement failures.
    pub(super) fn close_drag_out(&mut self) -> Result<(), Box<dyn Error>> {
        self.drag_out.ready = None;
        if let Some(job) = &self.drag_out.job {
            job.cancel();
        }
        if !self.drag_out.staged.is_empty() {
            let staged = self.drag_out.staged.drain(..).collect();
            self.retire(Retired { _drag_staging: staged, ..Default::default() })?;
        }
        Ok(())
    }
}
