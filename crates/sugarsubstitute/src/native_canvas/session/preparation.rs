// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Select canonical navigation/fine backings and admit one immutable image-operation request.
use super::*;

/// What the shown backing contributes to the next frame of one tier.
pub(super) struct FrameReuse {
    /// Whether the shown pixels stay as the base the new frame repairs.
    pub(super) compatible: bool,
    /// Pixels the new frame prepares.
    pub(super) damage: DamagePlan,
}

/// Decide which shown pixels a new frame keeps. A backing of the same document, source,
/// comparison, reconstruction and size keeps its pixels: a camera move repaints the exposed
/// strips, and an edit or staged gesture step since the shown content, `identity`, also
/// repaints what it changed, provided the shown content's snapshot was kept. Anything else is
/// prepared in full.
/// # Errors
/// Preserves viewport mapping and region construction failures.
#[expect(clippy::too_many_arguments, reason = "Each argument is one independent reuse fact")]
pub(super) fn frame_reuse(
    shown: Option<&Presented>,
    shown_document: Option<&(work::DocumentKey, DocumentSnapshot)>,
    document: &DocumentSnapshot,
    identity: work::DocumentKey,
    source_revision: u64,
    comparison: Option<Comparison>,
    reconstruction: Reconstruction,
    view: Viewport,
) -> Result<FrameReuse, sugarsubstitute_canvas::CanvasError> {
    let earlier = shown_document
        .filter(|(key, _)| shown.is_some_and(|shown| shown.document == *key))
        .map(|(_, snapshot)| snapshot);
    let compatible = shown.filter(|shown| {
        (shown.document == identity || shown.document.0 == identity.0 && earlier.is_some())
            && shown.source_revision == source_revision
            && shown.comparison == comparison
            && shown.reconstruction == reconstruction
            && shown.view.surface() == view.surface()
    });
    let Some(shown) = compatible else {
        return Ok(FrameReuse { compatible: false, damage: DamagePlan::full(view.surface()) });
    };
    let mut damage = view.repair_from(shown.view)?;
    if shown.document != identity
        && let Some(earlier) = earlier
    {
        damage.include_document_damage(&document.damage_since(earlier), view)?;
    }
    Ok(FrameReuse { compatible: true, damage })
}

impl Session {
    /// Preserve an already sharper document/source product while a coarser destination is pending.
    /// Exact preparation cannot exceed its device grid or the original raster sampling density.
    pub(super) fn has_finer_ready_detail(&self, prepared: Viewport) -> bool {
        let Some(scene) = &self.document.scene else {
            return false;
        };
        let identity = self.document.shown_key();
        self.document.presented.as_ref().is_some_and(|shown| {
            Some(shown.document) == identity
                && shown.source_revision == scene.source_revision
                && shown.view.zoom() > prepared.zoom()
                && shown.sampling_scale >= prepared.zoom().min(1.0)
        })
    }
    /// Anticipate a zoom-out destination without changing the displayed camera or authored graph.
    /// Fine pixel inspection retains the current physical scale until it crosses twofold zoom.
    pub(super) fn navigation_view(&self) -> Viewport {
        let navigation = self.document.tools.navigation();
        let current = navigation.viewport();
        let destination = navigation.target_viewport();
        if current.zoom() <= 2.0 && destination.zoom() < current.zoom() {
            destination
        } else {
            current
        }
    }

    /// Keep navigation products on canonical source pyramids; no sparse bootstrap sampling.
    fn navigation_request(
        &self,
        document: work::DocumentKey,
        source_revision: u64,
        extent: sugarsubstitute_image_ops::Extent,
    ) -> Result<Option<(BackingCoverage, Reconstruction)>, Box<dyn Error>> {
        let view = self.navigation_view();
        let reconstruction = Reconstruction::navigation_for_display_zoom(view.zoom())?;
        if let Some(shown) = self.document.presented.as_ref()
            && shown.document == document
            && shown.source_revision == source_revision
            && !shown.reconstruction.is_preview()
            && shown.view.shares_sampling_grid(view)
            && !coverage::detail_needs_refresh(shown.coverage, view, extent)?
        {
            return Ok(None);
        }
        let ready = self.document.navigation_presented.as_ref().is_some_and(|shown| {
            shown.document == document
                && shown.source_revision == source_revision
                && shown.reconstruction == reconstruction
                && shown.view.shares_sampling_grid(view)
        });
        if ready
            && let Some(shown) = self.document.navigation_presented.as_ref()
            && !coverage::detail_needs_refresh(shown.coverage, view, extent)?
        {
            return Ok(None);
        }
        Ok(Some((coverage::detail(view)?, reconstruction)))
    }
    /// Use a direct-source preview only until the bounded fallback first completes.
    pub(super) fn reconstruction(&self) -> Result<Reconstruction, Box<dyn Error>> {
        if self.document.initial_preview {
            Ok(Reconstruction::Preview)
        } else {
            Ok(Reconstruction::finished_for_display_zoom(
                self.document.tools.navigation().viewport().zoom(),
            )?)
        }
    }
    /// Keep pan repairs on settled pixels; fractional motion remains a display offset until release.
    pub(super) fn detail_view(
        &self,
        target: Viewport,
        document: work::DocumentKey,
        source_revision: u64,
        reconstruction: Reconstruction,
    ) -> Result<Viewport, Box<dyn Error>> {
        if self.document.tools.navigation().captured()
            && !self.document.tools.navigation().resampling()
            && let Some(shown) = &self.document.presented
            && shown.tier == BackingTier::Detail
            && shown.document == document
            && shown.source_revision == source_revision
            && !shown.reconstruction.is_preview()
            && shown.reconstruction == reconstruction
            && shown.view.zoom().to_bits() == target.zoom().to_bits()
        {
            return Ok(target.on_sampling_grid(shown.view)?);
        }
        Ok(target)
    }
    /// Commit a camera request only when the preceding request has actually completed.
    /// Navigation remains coalesced intent while a current document frame is being prepared.
    pub(super) fn submit_frame(&mut self) -> Result<(), Box<dyn Error>> {
        let scene = self.document.scene.as_ref().ok_or("Canvas document is unavailable")?;
        let (identity, document) = self.document.shown().ok_or("Canvas document is unavailable")?;
        let target_view = self.document.tools.navigation().viewport();
        let comparison = self.comparison_for(target_view)?;
        let document_extent = document.extent();
        let display_extent = target_view.surface();
        let needs_display_buffers = self.needs_display_buffers(display_extent);
        let fallback_current = self.document.fallback_presented.as_ref().is_some_and(|shown| {
            shown.document == identity && shown.source_revision == scene.source_revision
        });
        let navigating = self.document.tools.navigation().resampling();
        let navigation_request = if fallback_current && !needs_display_buffers && navigating {
            self.navigation_request(identity, scene.source_revision, document_extent)?
        } else {
            None
        };
        let (tier, coverage, reconstruction) = if !fallback_current {
            (BackingTier::Fallback, coverage::fallback(document_extent)?, Reconstruction::Preview)
        } else if needs_display_buffers {
            let shown = self
                .document
                .fallback_presented
                .as_ref()
                .ok_or("Canvas fallback identity was lost")?;
            (BackingTier::Fallback, shown.coverage, shown.reconstruction)
        } else if let Some((coverage, reconstruction)) = navigation_request {
            (BackingTier::Navigation, coverage, reconstruction)
        } else {
            let preparation_view = if navigating { self.navigation_view() } else { target_view };
            let reconstruction = if navigating {
                Reconstruction::finished_for_display_zoom(preparation_view.zoom())?
            } else {
                self.reconstruction()?
            };
            let detail_view = self.detail_view(
                preparation_view,
                identity,
                scene.source_revision,
                reconstruction,
            )?;
            let desired = coverage::detail(detail_view)?;
            let current = if let Some(shown) = self.document.presented.as_ref() {
                shown.document == identity
                    && shown.source_revision == scene.source_revision
                    && shown.comparison == comparison
                    && shown.reconstruction == reconstruction
                    && shown.view.zoom() == detail_view.zoom()
                    && !coverage::detail_needs_refresh(
                        shown.coverage,
                        detail_view,
                        document_extent,
                    )?
                    && shown.coverage.crop(target_view, document_extent)?.is_some()
            } else {
                false
            };
            if current {
                return Ok(());
            }
            (BackingTier::Detail, desired, reconstruction)
        };
        let view = coverage.viewport();
        let shown = match tier {
            BackingTier::Fallback => self.document.fallback_presented.as_ref(),
            BackingTier::Navigation => self.document.navigation_presented.as_ref(),
            BackingTier::Detail => self.document.presented.as_ref(),
        };
        let reuse = frame_reuse(
            shown,
            self.document.presented_documents[tier.index()].as_ref(),
            &document,
            identity,
            scene.source_revision,
            comparison,
            reconstruction,
            view,
        )?;
        let damage = reuse.damage;
        let replace_surface = if self.config.gpu.is_some() {
            let surface = match tier {
                BackingTier::Fallback => self.document.fallback_gpu_front.as_ref(),
                BackingTier::Navigation => self.document.navigation_gpu_front.as_ref(),
                BackingTier::Detail => self.document.gpu_front.as_ref(),
            };
            surface.is_none_or(|surface| surface.extent() != view.surface())
        } else {
            let surface = match tier {
                BackingTier::Fallback => self.document.fallback_front.as_ref(),
                BackingTier::Navigation => self.document.navigation_front.as_ref(),
                BackingTier::Detail => self.document.front.as_ref(),
            };
            surface.is_none_or(|surface| surface.extent() != view.surface())
        };
        let replace_buffers = needs_display_buffers;
        let base = if replace_surface {
            None
        } else {
            reuse
                .compatible
                .then(|| match tier {
                    BackingTier::Fallback => {
                        if self.config.gpu.is_some() {
                            self.document.fallback_gpu_front.as_ref().and_then(GpuSurface::current)
                        } else {
                            self.document
                                .fallback_front
                                .as_ref()
                                .and_then(|surface| surface.current())
                        }
                    }
                    BackingTier::Detail => self.current_frame(),
                    BackingTier::Navigation => self.backing_stamp(BackingTier::Navigation),
                })
                .flatten()
        };
        let next = self.frame_revision.checked_add(1).ok_or("Canvas frame revision exhausted")?;
        let target = FrameStamp { surface: SurfaceId::new(1)?, generation: Generation::new(next)? };
        let bytes = if self.config.gpu.is_some() {
            if self.config.capture { work::GPU_PROOF_BYTES } else { work::GPU_PREPARE_BYTES }
        } else {
            damage
                .dirty()
                .pixels()
                .checked_mul(4)
                .and_then(|bytes| bytes.checked_add(256 * 1024))
                .ok_or("Canvas patch reservation overflow")?
        };
        let spec = self.spec(
            Operation::ImageEvaluate,
            JobClass::CpuDecode,
            bytes,
            Duration::from_secs(300),
            None,
        )?;
        let resources =
            self.resources.as_ref().ok_or("Canvas preparation resources are unavailable")?.clone();
        let source = scene.source.clone();
        let compared = scene.compared.as_ref().map(|compared| compared.source.clone());
        let placed = scene.placed.iter().map(|cell| cell.source.clone()).collect();
        let source_revision = scene.source_revision;
        let gpu_surface = match tier {
            BackingTier::Fallback => self.document.fallback_gpu_front.clone(),
            BackingTier::Navigation => self.document.navigation_gpu_front.clone(),
            BackingTier::Detail => self.document.gpu_front.clone(),
        };
        let snapshot = document.clone();
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        match runtime.client.submit_cpu(&runtime.scope, spec, move |_| work::Prepare {
            tier,
            reconstruction,
            resources,
            document,
            key: identity,
            source,
            compared,
            placed,
            comparison,
            source_revision,
            view,
            target,
            base,
            damage,
            replace_surface,
            replace_buffers,
            display_extent,
            gpu_surface,
            finished: false,
            cpu: None,
            display_buffers: None,
            started: None,
            source_stats_before: None,
        }) {
            Ok(job) => {
                self.rendering = Some(Rendering {
                    job,
                    document: identity,
                    snapshot,
                    source_revision,
                    reconstruction,
                    tier,
                    coverage,
                    display_extent,
                    comparison,
                    superseded: false,
                });
                self.frame_revision = next;
            }
            Err(cause) if activity::retryable(&cause) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }
}
