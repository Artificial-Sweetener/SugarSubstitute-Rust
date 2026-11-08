// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Present GPU frames through shared textures copied by the main-window composition.
//!
//! The composition target in the session resources retains every texture a frame here names,
//! so replacing or abandoning a frame on the UI thread never releases the last texture
//! reference; close retires frames and resources together on the retirement worker.

use super::*;
use sugarsubstitute_desktop::canvas::{
    CanvasContent, CanvasGraphics, Lifetime, SharedFrame,
    composition_adapter::SharedTextureFallback,
};

/// One completed shared-texture frame with its exact application identity.
pub(super) struct CompositionDisplay {
    pub(super) target: FrameStamp,
    pub(super) frame: work::CompositionFrame,
}

impl Session {
    /// Adopt the composition device prepared for the selected GPU, or fall back to CPU frames.
    /// Runs before any display work, so every later renderer decision sees the final mode.
    pub(super) fn adopt_composition_graphics(
        &mut self,
        graphics: &CanvasGraphics,
    ) -> Result<(), Box<dyn Error>> {
        let Some(device) = self.composition_device.take() else {
            return Ok(());
        };
        if self.closing {
            return self.retire(Retired { _device: Some(device), ..Default::default() });
        }
        let adapter = device.descriptor().name.clone();
        let factory = graphics
            .shared_texture_factory()
            .filter(|factory| device.descriptor().luid == Some(factory.luid()));
        match factory {
            Some(factory) => {
                let target = work::CompositionTarget::new(device.clone(), factory)?;
                let resources = self.resources.take().ok_or("Canvas resources are unavailable")?;
                self.gpu_descriptor = Some(device.descriptor().clone());
                self.gpu_diagnostics = Some(device.diagnostics());
                self.resources = Some(resources.with_composition(device, target));
                tracing::info!(
                    operation = "canvas_gpu_composition",
                    adapter = %adapter,
                    "Embedded canvas presents GPU frames through shared composition textures"
                );
            }
            None => {
                let reason = graphics
                    .shared_texture_fallback()
                    .map_or("composition_adapter_mismatch", SharedTextureFallback::name);
                self.fall_back_to_cpu(reason, &adapter);
                self.retire(Retired { _device: Some(device), ..Default::default() })?;
            }
        }
        self.gpu_ready = true;
        Ok(())
    }

    /// Borrow the frame the main-window composition shows: an offered CPU preview, the current
    /// shared GPU frame, or the current CPU frame of a CPU-only document.
    pub(crate) fn selected_frame(&self) -> Option<(CanvasContent<'_>, &MemoryLease)> {
        if let Some((frame, memory)) = self.preview_cpu_frame() {
            return Some((CanvasContent::Cpu(frame), memory));
        }
        if !self.gpu_document() {
            return self
                .selected_cpu_frame()
                .map(|(frame, memory)| (CanvasContent::Cpu(frame), memory));
        }
        if self.closing
            || self.failed.is_some()
            || self.pending_open.is_some()
            || self.loading.is_some()
        {
            return None;
        }
        if !self.draws_current_selection() {
            return None;
        }
        let scene = self.document.scene.as_ref()?;
        let (display, pending) = match (&self.composition_pending, &self.composition_shown) {
            (Some(display), _) => (display, true),
            (None, Some(display)) => (display, false),
            (None, None) => return None,
        };
        let evidence = if pending {
            self.document.pending_gpu_evidence?
        } else {
            self.document.display_evidence?
        };
        let surface = self.document.tools.navigation().viewport().surface();
        // As for CPU frames, a frame of an earlier revision or gesture step stays presentable.
        if evidence.target != display.target
            || evidence.source_revision != self.selection
            || evidence.document.0 != scene.document.snapshot().identity()
            || evidence.view.surface() != surface
            || evidence.backdrop_generation != self.backdrops.generation
            || display.frame.texture().extent() != surface
        {
            return None;
        }
        let content = CanvasContent::Shared(SharedFrame {
            pending,
            target: display.target,
            texture: display.frame.texture(),
        });
        Some((content, &self.resources.as_ref()?.presentation))
    }

    /// Acknowledge the exact selected frame after the composition committed it.
    /// # Errors
    /// Rejects foreign, stale and duplicate acknowledgements.
    pub(crate) fn did_present_selected(
        &mut self,
        target: FrameStamp,
    ) -> Result<(), Box<dyn Error>> {
        if self.composition_pending.as_ref().is_some_and(|display| display.target == target) {
            return self.did_present_composition(target);
        }
        self.did_present_cpu(target)
    }

    /// Commit shared-frame evidence once the composition copied and committed its texture.
    /// # Errors
    /// Rejects foreign, stale and duplicate acknowledgements.
    pub(crate) fn did_present_composition(
        &mut self,
        target: FrameStamp,
    ) -> Result<(), Box<dyn Error>> {
        if self.composition_pending.as_ref().is_none_or(|display| {
            display.target != target
                || display.frame.backdrop_generation() != self.backdrops.generation
        }) || self.document.pending_gpu_evidence.is_none_or(|evidence| evidence.target != target)
        {
            return Err("Canvas received a foreign shared-frame acknowledgement".into());
        }
        let evidence = self
            .document
            .pending_gpu_evidence
            .take()
            .ok_or("Canvas shared-frame evidence is unavailable")?;
        self.pacing.native_accepted(target, self.navigation_time())?;
        self.composition_shown = self.composition_pending.take();
        self.accept_display(evidence, true)?;
        self.document.native_presented = Some(target);
        Ok(())
    }

    /// Withdraw an unacknowledged shared frame before its source, extent or attachment changes.
    pub(super) fn abandon_composition_frame(
        &mut self,
        now: Duration,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(display) = self.composition_pending.take() {
            self.pacing.abandoned(display.target, now)?;
            self.document.pending_gpu_evidence = None;
        }
        Ok(())
    }

    /// Accept a completed shared frame as the sole pending native presentation.
    pub(super) fn offer_composition_frame(
        &mut self,
        target: FrameStamp,
        frame: work::CompositionFrame,
        evidence: DisplayEvidence,
        now: Duration,
    ) -> Result<(), Box<dyn Error>> {
        if frame.backdrop_generation() != evidence.backdrop_generation
            || evidence.backdrop_generation != self.backdrops.generation
        {
            return Err("Composition pixels have a stale backdrop generation".into());
        }
        self.pacing.prepared(target, now)?;
        self.document.pending_gpu_evidence = Some(evidence);
        self.composition_pending = Some(CompositionDisplay { target, frame });
        Ok(())
    }
}
