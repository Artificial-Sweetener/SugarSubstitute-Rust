// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Expose narrow admitted byte custody and storage retirement to the native transport adapter.

use super::{preview::*, *};

impl Session {
    /// Read the selected source generation for correlating an accepted offer with later frames.
    pub(crate) fn source_intent_revision(&self) -> u64 {
        self.selection
    }

    /// Keep accepted handoff disposal intact until the existing retirement queue has capacity.
    pub(in crate::native_canvas::session) fn advance_preview_retirement(
        &mut self,
    ) -> Result<(), Box<dyn Error>> {
        if self.deferred_retirement.is_some() {
            return Ok(());
        }
        if let Some(value) = self.preview.retired.take() {
            self.retire(Retired { _preview: Some(value), ..Default::default() })?;
        }
        Ok(())
    }

    /// Preview candidates are prepared and presented as CPU frames, including in a composition
    /// GPU session; standalone GPU windows refuse them at discovery and admission instead of
    /// holding transfers until their lease expires.
    pub(crate) fn presents_previews(&self) -> bool {
        self.presentation.gpu_window().is_none()
    }

    /// Report whether accepted preview pixels still own the active document during restoration.
    pub(crate) fn preview_active(&self) -> bool {
        self.preview_shown()
    }

    /// Prove that withdrawal restored an empty application attachment, ready for native hide.
    /// Native visibility remains owned by the desktop adapter and requires its actual acknowledgement.
    pub(crate) fn preview_restored_empty(&self) -> bool {
        !self.closing
            && !self.preview_pending()
            && !self.preview.restoring
            && self.pending_open.is_none()
            && self.loading.is_none()
            && self.document.scene.as_ref().is_none_or(|scene| {
                scene.source.is_none()
                    && scene.original.is_none()
                    && scene.document.snapshot().layers().is_empty()
            })
    }

    /// Borrow the resident session admission for bounded native preview assembly claims.
    pub(crate) fn preview_resident(&self) -> Option<MemoryLease> {
        if self.closing {
            return None;
        }
        Some(self.resources.as_ref()?.memory.clone())
    }

    /// Read the execution clock epoch used by preview and shutdown deadlines.
    pub(crate) fn preview_now(&self) -> Moment {
        self.clock.now()
    }

    /// Retire an aborted admitted byte assembly on the existing storage worker.
    /// Refusal preserves the exact vector and its original claim in the caller's slot.
    pub(crate) fn retire_preview_bytes(
        &mut self,
        input: &mut Option<(Vec<u8>, MemoryClaim)>,
    ) -> Result<bool, Box<dyn Error>> {
        if self.deferred_retirement.is_some() || self.shutdown_started {
            return Ok(false);
        }
        if input.is_none() {
            return Ok(true);
        }
        let cutoff = self.disposal_cutoff();
        let value = Retired {
            _preview: Some(PreviewRetired { _bytes: input.take(), ..Default::default() }),
            ..Default::default()
        };
        self.preview.poll_requested = true;
        self.retire(value)?;
        if self.retirement_deadline.is_none() {
            self.retirement_deadline = cutoff;
        }
        Ok(true)
    }

    /// Retire an unaccepted original on the existing storage worker with caller-retained refusal.
    pub(crate) fn retire_preview_original(
        &mut self,
        input: &mut Option<ImageSource>,
    ) -> Result<bool, Box<dyn Error>> {
        if self.deferred_retirement.is_some() || self.shutdown_started {
            return Ok(false);
        }
        if input.is_none() {
            return Ok(true);
        }
        self.preview.poll_requested = true;
        self.retire(Retired {
            _preview: Some(PreviewRetired { _original: input.take(), ..Default::default() }),
            ..Default::default()
        })?;
        Ok(true)
    }
}
