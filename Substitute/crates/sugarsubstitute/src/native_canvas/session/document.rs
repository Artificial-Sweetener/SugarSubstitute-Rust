// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one document's tools, image backings and exact prepared/presented frame evidence.

use super::*;
use sugarsubstitute_desktop::canvas::{CpuFrame, CpuFrameKind};

/// Movable document custody within one execution and graphics lifetime.
/// Sessions and retirement workers transfer its heap allocation, keeping native callback
/// stack frames independent of the retained document's inline tool and presentation state.
/// Shared jobs must reach their observed terminal outcomes before this owner is withdrawn.
/// Its pixels and originals retain their existing memory and storage claims while held.
pub(super) struct DocumentState {
    pub(super) scene: Option<Scene>,
    pub(super) tools: ToolManager,
    pub(super) initial_preview: bool,
    pub(super) front: Option<Arc<CpuSurface>>,
    pub(super) fallback_front: Option<Arc<CpuSurface>>,
    pub(super) navigation_front: Option<Arc<CpuSurface>>,
    pub(super) gpu_front: Option<GpuSurface>,
    pub(super) fallback_gpu_front: Option<GpuSurface>,
    pub(super) navigation_gpu_front: Option<GpuSurface>,
    pub(super) native_presented: Option<FrameStamp>,
    pub(super) native_verified: bool,
    pub(super) display_source: Option<FrameStamp>,
    pub(super) display_view: Option<Viewport>,
    pub(super) display_evidence: Option<DisplayEvidence>,
    pub(super) pending_cpu_evidence: Option<DisplayEvidence>,
    pub(super) pending_gpu_evidence: Option<DisplayEvidence>,
    pub(super) cpu_pending: Option<FrameStamp>,
    pub(super) cpu_projection: Option<Box<CpuProjection>>,
    pub(super) projection_damage: Option<DamagePlan>,
    pub(super) projection_base: Option<FrameStamp>,
    pub(super) display_buffers: [Option<ProjectionBuffer>; 2],
    pub(super) display_buffer_extent: Option<sugarsubstitute_image_ops::Extent>,
    pub(super) presented: Option<Presented>,
    pub(super) fallback_presented: Option<Presented>,
    pub(super) navigation_presented: Option<Presented>,
    /// The content each tier's presented backing shows, by tier.
    pub(super) presented_documents: [Option<(work::DocumentKey, DocumentSnapshot)>; 3],
    /// The staged gesture preview frames show instead of the published revision.
    pub(super) staged: Option<Staged>,
    /// The latest staged preview generation; each latched preview step takes a new one.
    #[cfg(feature = "native-composition")]
    pub(super) staged_generation: u64,
    pub(super) rebuilt: bool,
    /// CPU-prepared previews keep CPU backings and display even in a GPU session.
    pub(super) cpu_only: bool,
    /// Host identity of the output this document shows, when it may be retained for reselection.
    #[cfg(feature = "native-composition")]
    pub(super) retention: Option<retention::RetentionKey>,
    /// Identity of the synced input document this is, which is never retained or evicted.
    #[cfg(feature = "native-composition")]
    pub(super) input: Option<inputs::InputDocumentId>,
}

/// A staged gesture preview latched for display. Frames keep showing it until no frame work
/// is in flight, so live strokes publish frames instead of discarding stale ones.
pub(super) struct Staged {
    /// The published revision the preview builds on.
    pub(super) base: (u64, u64),
    /// The edit session and stage the preview shows.
    #[cfg(feature = "native-composition")]
    pub(super) step: (u64, u64),
    /// The frame identity of this preview.
    pub(super) key: work::DocumentKey,
    /// The staged content.
    pub(super) snapshot: DocumentSnapshot,
}

impl DocumentState {
    /// The content frames show, with its key: the latched gesture preview while it still builds
    /// on the published revision, otherwise the published revision.
    pub(super) fn shown(&self) -> Option<(work::DocumentKey, DocumentSnapshot)> {
        let published = self.scene.as_ref()?.document.snapshot();
        match &self.staged {
            Some(staged) if staged.base == (published.identity(), published.revision()) => {
                Some((staged.key, staged.snapshot.clone()))
            }
            _ => Some(((published.identity(), published.revision(), 0), published)),
        }
    }

    /// The key of the content frames show; see [`Self::shown`].
    pub(super) fn shown_key(&self) -> Option<work::DocumentKey> {
        self.shown().map(|(key, _)| key)
    }

    /// Start an empty document at the existing physical viewport without allocating pixels.
    pub(super) fn new(view: Viewport) -> Self {
        Self {
            scene: None,
            tools: ToolManager::new(Navigation::new(view)),
            initial_preview: false,
            front: None,
            fallback_front: None,
            navigation_front: None,
            gpu_front: None,
            fallback_gpu_front: None,
            navigation_gpu_front: None,
            native_presented: None,
            native_verified: false,
            display_source: None,
            display_view: None,
            display_evidence: None,
            pending_cpu_evidence: None,
            pending_gpu_evidence: None,
            cpu_pending: None,
            cpu_projection: None,
            projection_damage: None,
            projection_base: None,
            display_buffers: [None, None],
            display_buffer_extent: None,
            presented: None,
            fallback_presented: None,
            navigation_presented: None,
            presented_documents: [None, None, None],
            staged: None,
            #[cfg(feature = "native-composition")]
            staged_generation: 0,
            rebuilt: false,
            cpu_only: false,
            #[cfg(feature = "native-composition")]
            retention: None,
            #[cfg(feature = "native-composition")]
            input: None,
        }
    }

    /// Borrow only a complete prepared projection or its exact acknowledged predecessor.
    pub(super) fn cpu_frame(&self) -> Option<CpuFrame<'_>> {
        let frame = self.cpu_projection.as_ref()?;
        if self.cpu_pending != Some(frame.target())
            && self.display_evidence.is_none_or(|evidence| evidence.target != frame.target())
        {
            return None;
        }
        Some(CpuFrame {
            pending: self.cpu_pending == Some(frame.target()),
            kind: CpuFrameKind::Projected,
            target: frame.target(),
            base: self.projection_base,
            damage: self.projection_damage.as_ref()?,
            pixels: frame.pixels(),
        })
    }

    /// Report retained resources that require worker-owned disposal before execution closes.
    pub(super) fn has_resources(&self) -> bool {
        self.scene.is_some()
            || self.front.is_some()
            || self.fallback_front.is_some()
            || self.navigation_front.is_some()
            || self.gpu_front.is_some()
            || self.fallback_gpu_front.is_some()
            || self.navigation_gpu_front.is_some()
            || self.cpu_projection.is_some()
            || self.display_buffers.iter().any(Option::is_some)
    }
}

impl Session {
    /// Transfer this document intact and leave an empty document at the same physical viewport.
    /// The caller retains custody until promotion or submission through the retirement worker.
    ///
    /// # Errors
    /// Refuses unobserved document jobs or CPU/GPU evidence awaiting native acknowledgement.
    pub(super) fn withdraw_document(&mut self) -> Result<Box<DocumentState>, Box<dyn Error>> {
        if self.initializing.is_some()
            || self.loading.is_some()
            || self.rendering.is_some()
            || self.displaying.is_some()
            || self.presenting.is_some()
        {
            return Err("Canvas document still has unobserved worker outcomes".into());
        }
        if self.document.cpu_pending.is_some()
            || self.document.pending_cpu_evidence.is_some()
            || self.document.pending_gpu_evidence.is_some()
            || self.awaiting_gpu.is_some()
        {
            return Err("Canvas document still has unacknowledged presentation evidence".into());
        }
        let mut empty = DocumentState::new(Viewport::new(
            self.document.tools.navigation().viewport().surface(),
        )?);
        empty.tools.set_refresh_rate(self.display_refresh)?;
        Ok(std::mem::replace(&mut self.document, Box::new(empty)))
    }
}
