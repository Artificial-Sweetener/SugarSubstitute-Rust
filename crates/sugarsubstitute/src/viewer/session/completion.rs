// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish only matching document/source snapshots and retire superseded native source custody.
use super::*;
use sugarsubstitute_image_io::ImportError;

impl Session {
    /// Retain actual initialization results or transfer their complete custody to owned cleanup.
    pub(super) fn consume_initialization(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(outcome) = Self::take(&mut self.initializing) else {
            return Ok(());
        };
        match Self::terminal(outcome) {
            Ok((resources, scene)) if self.closing => self.retire(Retired {
                _resources: Some(resources),
                _scene: Some(scene),
                ..Default::default()
            })?,
            Ok((resources, scene)) => {
                self.resources = Some(resources);
                self.scene = Some(scene);
                if let Some(path) = self.options.image.take() {
                    self.open(path, CacheReuse::Compatible, None)?;
                } else {
                    self.picker = true;
                }
            }
            Err(cause) if self.closing => {
                tracing::debug!(operation="canvas_initialization_stopped", cause=%cause, "Canvas initialization reached a terminal outcome during close")
            }
            Err(cause) => self.fail(cause),
        }
        Ok(())
    }
    /// A cache rebuild changes source custody while preserving the document's authored history.
    pub(super) fn consume_load(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(completion) =
            self.loading.as_mut().and_then(|loading| loading.job.try_take_completion())
        else {
            return Ok(());
        };
        let loading = self.loading.take().ok_or("Canvas load identity was lost")?;
        Self::observe_terminal(&loading.job, &completion.outcome);
        let current = !self.closing && loading.intent == self.selection;
        match Self::terminal(completion.outcome) {
            Ok(scene) if !current => {
                self.retire(Retired { _scene: Some(scene), ..Default::default() })?
            }
            Ok(mut scene) => {
                if let Some(expected) = loading.expected {
                    if scene.source.as_ref().map(|source| source.descriptor().asset())
                        != Some(expected)
                    {
                        self.retire(Retired { _scene: Some(scene), ..Default::default() })?;
                        self.fail(WorkError::invalid(
                            "Original image changed; the preceding source cannot be reconstructed",
                        ));
                        return Ok(());
                    }
                    let current =
                        self.scene.as_mut().ok_or("Canvas rebuild has no current document")?;
                    std::mem::swap(&mut current.source, &mut scene.source);
                    current.source_revision = scene.source_revision;
                    self.retire(Retired { _scene: Some(scene), ..Default::default() })?;
                } else {
                    self.tools.set_content(
                        Some(scene.document.snapshot().extent()),
                        Some(&scene.document),
                        self.navigation_time(),
                    )?;
                    if let Some(previous) = self.scene.replace(scene) {
                        self.retire(Retired { _scene: Some(previous), ..Default::default() })?;
                    }
                    self.rebuilt = false;
                }
                self.initial_preview = true;
                self.failed = None;
                self.title = self.labels.title.to_owned();
            }
            Err(cause) if !current => {
                tracing::debug!(operation="canvas_source_stopped", cause=%cause, "Superseded canvas source reached a terminal outcome")
            }
            Err(cause) => self.fail(cause),
        }
        Ok(())
    }
    /// A current accepted camera request may finish while later camera intent is coalesced.
    /// Its pixels are always labelled with their actual camera and current authored revision.
    pub(super) fn consume_frame(&mut self) -> Result<(), Box<dyn Error>> {
        let backing_borrowed =
            self.rendering.as_ref().is_some_and(|rendering| match rendering.tier {
                BackingTier::Fallback => {
                    self.fallback_front.as_ref().is_some_and(|front| Arc::strong_count(front) > 1)
                }
                BackingTier::Detail => {
                    self.front.as_ref().is_some_and(|front| Arc::strong_count(front) > 1)
                }
            });
        if self.options.gpu.is_none()
            && !self.closing
            && self.pending_open.is_none()
            && (self.cpu_pending.is_some()
                || backing_borrowed
                || (self.native_target.is_some()
                    && self.pacing.readiness(self.navigation_time())?
                        != PresentationReadiness::Ready))
        {
            return Ok(());
        }
        let Some(completion) =
            self.rendering.as_mut().and_then(|rendering| rendering.job.try_take_completion())
        else {
            return Ok(());
        };
        let rendering = self.rendering.take().ok_or("Canvas rendering identity was lost")?;
        Self::observe_terminal(&rendering.job, &completion.outcome);
        if rendering.superseded {
            match completion.outcome {
                JobOutcome::Success(packet) => {
                    self.retire(Retired { _packet: Some(packet), ..Default::default() })?
                }
                JobOutcome::Failure(cause) => self.fail(cause),
                JobOutcome::Panicked(cause) => self.fail(WorkerPanic { _cause: cause }),
                stopped @ (JobOutcome::Cancelled
                | JobOutcome::TimedOut
                | JobOutcome::Coalesced { .. }) => {
                    tracing::debug!(operation="canvas_refinement_stopped", outcome=?stopped.kind(), "Superseded refinement reached its actual terminal outcome");
                }
            }
            return Ok(());
        }
        match Self::terminal(completion.outcome) {
            Ok(mut packet) => {
                if packet.tier != rendering.tier
                    || packet.reconstruction != rendering.reconstruction
                    || packet.view != rendering.coverage.viewport()
                {
                    self.retire(Retired { _packet: Some(packet), ..Default::default() })?;
                    return Err("Canvas received foreign backing coverage".into());
                }
                let current = !self.closing
                    && self.pending_open.is_none()
                    && self.loading.is_none()
                    && self.scene.as_ref().is_some_and(|scene| {
                        let snapshot = scene.document.snapshot();
                        packet.document == (snapshot.identity(), snapshot.revision())
                            && packet.source_revision == scene.source_revision
                            && (packet.tier == BackingTier::Fallback
                                || rendering
                                    .coverage
                                    .crop(self.tools.navigation().viewport(), snapshot.extent())
                                    .is_ok_and(|crop| crop.is_some()))
                    });
                retain_first_fallback_io(
                    &mut self.first_fallback_preparation_io,
                    current,
                    packet.tier,
                    packet.reconstruction,
                    packet.source_io,
                );
                if !current {
                    self.retire(Retired { _packet: Some(packet), ..Default::default() })?;
                    return Ok(());
                }
                let mut retired = Retired::default();
                if let Some(buffers) = packet.display_buffers.take() {
                    let (first, second) = buffers.into_parts();
                    if first.extent() != rendering.display_extent
                        || second.extent() != rendering.display_extent
                    {
                        self.retire(Retired {
                            _packet: Some(packet),
                            _buffers: [Some(first), Some(second)],
                            ..Default::default()
                        })?;
                        return Err("Canvas received projection buffers for another extent".into());
                    }
                    retired._buffers = self.install_display_buffers(
                        [Some(first), Some(second)],
                        rendering.display_extent,
                    );
                }
                let work = match packet.pixels {
                    work::Pixels::Cpu(work::CpuPacket::Replace { surface }) => {
                        let work = sugarsubstitute_canvas_render::SurfaceWork {
                            repaired_pixels: surface.extent().pixels(),
                            reused_pixels: 0,
                        };
                        match packet.tier {
                            BackingTier::Fallback => {
                                retired._fallback_front =
                                    self.fallback_front.replace(Arc::new(surface));
                            }
                            BackingTier::Detail => {
                                retired._front = self.front.replace(Arc::new(surface));
                            }
                        }
                        Ok(work)
                    }
                    work::Pixels::Cpu(work::CpuPacket::Repair(update)) => {
                        let retained = match packet.tier {
                            BackingTier::Fallback => &mut self.fallback_front,
                            BackingTier::Detail => &mut self.front,
                        };
                        let result = retained
                            .as_mut()
                            .ok_or_else(|| {
                                Box::<dyn Error>::from("Canvas retained viewport is unavailable")
                            })
                            .and_then(|surface| {
                                let retained = Arc::get_mut(surface).ok_or_else(|| Box::<dyn Error>::from("Canvas canonical surface is still borrowed by display work"))?;
                                retained.apply(&update).map_err(Box::<dyn Error>::from)
                            });
                        retired._update = Some(update);
                        result
                    }
                    work::Pixels::Gpu(pixels) => {
                        match packet.tier {
                            BackingTier::Fallback => {
                                retired._fallback_gpu_front =
                                    self.fallback_gpu_front.replace(pixels.surface);
                                if let Some(snapshot) = pixels.snapshot {
                                    retired._fallback_front =
                                        self.fallback_front.replace(Arc::new(snapshot));
                                }
                            }
                            BackingTier::Detail => {
                                retired._gpu_front = self.gpu_front.replace(pixels.surface);
                                if let Some(snapshot) = pixels.snapshot {
                                    retired._front = self.front.replace(Arc::new(snapshot));
                                }
                            }
                        }
                        self.native_verified = false;
                        self.expose_pending = true;
                        Ok(pixels.work)
                    }
                };
                if retired._front.is_some()
                    || retired._gpu_front.is_some()
                    || retired._fallback_front.is_some()
                    || retired._fallback_gpu_front.is_some()
                    || retired._update.is_some()
                    || retired._projection.is_some()
                    || retired._buffers.iter().any(Option::is_some)
                {
                    self.retire(retired)?;
                }
                let work = work?;
                if packet.tier == BackingTier::Fallback && rendering.reconstruction.is_preview() {
                    self.initial_preview = false;
                }
                let shown = Presented {
                    tier: packet.tier,
                    coverage: rendering.coverage,
                    view: packet.view,
                    reconstruction: packet.reconstruction,
                    document: packet.document,
                    source_revision: packet.source_revision,
                };
                match packet.tier {
                    BackingTier::Fallback => self.fallback_presented = Some(shown),
                    BackingTier::Detail => self.presented = Some(shown),
                }
                self.last_prepare = packet.elapsed;
                self.last_stats = packet.stats;
                self.last_surface_work = work;
                if self.scene.as_ref().is_some_and(|scene| scene.source.is_some()) {
                    self.title = self.labels.viewing(self.tools.navigation().viewport().zoom());
                }
            }
            Err(cause) if self.closing || self.pending_open.is_some() => {
                tracing::debug!(operation="canvas_frame_stopped", cause=%cause, "Canvas frame reached a terminal outcome before replacement")
            }
            Err(cause) => {
                if !self.rebuilt
                    && corrupt(cause.as_ref())
                    && let Some(scene) = &self.scene
                    && let (Some(path), Some(source)) = (&scene.original, &scene.source)
                {
                    let path = path.clone();
                    let expected = source.descriptor().asset();
                    self.rebuilt = true;
                    self.open(path, CacheReuse::Rebuild, Some(expected))?;
                    return Ok(());
                }
                self.fail(cause);
            }
        }
        Ok(())
    }
}
/// Retain only the first completed, current-source fallback preview preparation measurement.
pub(super) fn retain_first_fallback_io(
    retained: &mut Option<ArchiveStats>,
    current: bool,
    tier: BackingTier,
    reconstruction: Reconstruction,
    source_io: Option<ArchiveStats>,
) {
    if current && retained.is_none() && tier == BackingTier::Fallback && reconstruction.is_preview()
    {
        *retained = source_io;
    }
}
/// Match the typed cache-integrity cause without interpreting error text as a recovery command.
fn corrupt(mut cause: &(dyn Error + 'static)) -> bool {
    for _ in 0..16 {
        if cause
            .downcast_ref::<ImportError>()
            .is_some_and(|cause| matches!(cause, ImportError::Corrupt(_)))
        {
            return true;
        }
        let Some(next) = cause.source() else {
            break;
        };
        cause = next;
    }
    false
}
