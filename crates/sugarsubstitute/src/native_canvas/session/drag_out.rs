// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Copy the shown original for an OS drag-out through the existing execution scope.
//!
//! The retained original's bytes are read on a storage worker and charged to the canvas memory
//! lease, so the UI thread only hands completed bytes to the desktop's drag source.

use super::*;

/// Largest original handed to an OS drag; larger outputs stay in Comfy's storage.
pub(crate) const DRAG_OUT_BYTES: u64 = 128 * 1024 * 1024;

/// Complete original bytes for one drag-out request and the claim that covers them.
pub(crate) struct DragOutBytes {
    /// The caller's request identity, so a superseded drag never reuses these bytes.
    pub(crate) request: u64,
    pub(crate) bytes: Vec<u8>,
    pub(crate) claim: MemoryClaim,
}

/// At most one copy in flight and one completed copy awaiting the desktop.
#[derive(Default)]
pub(super) struct DragOut {
    job: Option<OwnedJob<DragOutBytes, WorkError>>,
    ready: Option<DragOutBytes>,
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

    /// Start copying the shown original for drag-out `request`.
    ///
    /// Returns false without submitting when closing, when no original is shown or while an
    /// earlier copy is in flight; completed bytes for an older request are discarded.
    ///
    /// # Errors
    /// Preserves identity, deadline and non-retryable admission failures.
    pub(crate) fn request_drag_out(&mut self, request: u64) -> Result<bool, Box<dyn Error>> {
        self.drag_out.ready = None;
        let original = self.document.scene.as_ref().and_then(|scene| scene.original.clone());
        let (Some(original), false, false) = (original, self.closing, self.drag_out.in_flight())
        else {
            return Ok(false);
        };
        let memory = self.resources.as_ref().ok_or("Canvas storage is unavailable")?.memory.clone();
        let spec = self.spec(
            Operation::Infrastructure,
            JobClass::InfrastructureOrdinary,
            64 * 1024,
            Duration::from_secs(10),
            None,
        )?;
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        match runtime.client.submit_blocking(&runtime.scope, spec, move |context| {
            let (bytes, claim) = original
                .encoded_bytes(DRAG_OUT_BYTES, &memory, &context)
                .map_err(WorkError::new)?;
            Ok(DragOutBytes { request, bytes, claim })
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

    /// Observe a completed copy; a failed copy loses only that drag-out.
    pub(super) fn consume_drag_out(&mut self) {
        let Some(outcome) = Self::take(&mut self.drag_out.job) else { return };
        match Self::terminal(outcome) {
            Ok(bytes) if !self.closing => self.drag_out.ready = Some(bytes),
            Ok(_) => {}
            Err(cause) => {
                tracing::warn!(operation = "canvas_drag_out_copy", cause = %cause,
                    "The shown output could not be prepared for drag-out");
            }
        }
    }

    /// Cancel an in-flight copy and release completed bytes when the session closes.
    pub(super) fn close_drag_out(&mut self) {
        self.drag_out.ready = None;
        if let Some(job) = &self.drag_out.job {
            job.cancel();
        }
    }
}
