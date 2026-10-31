// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish only matching document/source snapshots and retire superseded native source custody.
use super::*;
use sugarsubstitute_image_io::ImportError;

impl Session {
    /// Match an admitted authored snapshot and source to the selected document before publication
    /// or failure handling. Camera changes do not retire work for the same authored source.
    pub(super) fn admitted_source_is_current(
        &self,
        document: (u64, u64),
        source_revision: u64,
    ) -> bool {
        source_revision == self.selection
            && self.document.scene.as_ref().is_some_and(|scene| {
                let snapshot = scene.document.snapshot();
                document == (snapshot.identity(), snapshot.revision())
                    && source_revision == scene.source_revision
            })
    }
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
                self.document.scene = Some(scene);
                match std::mem::replace(&mut self.config.initial, InitialSource::AwaitHost) {
                    InitialSource::AwaitHost => {}
                    InitialSource::PickFile => self.picker = true,
                    InitialSource::File(path) => self.open(path, CacheReuse::Compatible, None)?,
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
        #[cfg(feature = "native-composition")]
        let incomplete = loading.acquirer.as_ref().is_some_and(|owner| !owner.settled())
            && matches!(&completion.outcome, JobOutcome::Success(_));
        #[cfg(not(feature = "native-composition"))]
        let incomplete = false;
        #[cfg(feature = "native-composition")]
        let retirement = loading.acquirer.map_or(Ok(()), |owner| self.retain_acquirer(owner));
        #[cfg(not(feature = "native-composition"))]
        let retirement: Result<(), Box<dyn Error>> = Ok(());
        let current_intent = !self.closing && loading.intent == self.selection;
        if incomplete && current_intent {
            self.fail(WorkError::invalid("Acquisition returned before native retirement"));
        }
        let current = current_intent && !incomplete;
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
                        return retirement;
                    }
                    let current = self
                        .document
                        .scene
                        .as_mut()
                        .ok_or("Canvas rebuild has no current document")?;
                    std::mem::swap(&mut current.source, &mut scene.source);
                    current.source_revision = scene.source_revision;
                    self.retire(Retired { _scene: Some(scene), ..Default::default() })?;
                } else {
                    self.document.tools.set_content(
                        Some(scene.document.snapshot().extent()),
                        Some(&scene.document),
                        self.navigation_time(),
                    )?;
                    if let Some(previous) = self.document.scene.replace(scene) {
                        self.retire(Retired { _scene: Some(previous), ..Default::default() })?;
                    }
                    self.document.rebuilt = false;
                }
                self.document.initial_preview = true;
                self.failed = None;
                self.title = self.labels.title.to_owned();
            }
            Err(cause) if !current => {
                tracing::debug!(operation="canvas_source_stopped", cause=%cause, "Superseded canvas source reached a terminal outcome")
            }
            Err(cause) => self.fail(cause),
        }
        retirement
    }
    /// A current accepted camera request may finish while later camera intent is coalesced.
    /// Its pixels are always labelled with their actual camera and current authored revision.
    pub(super) fn consume_frame(&mut self) -> Result<(), Box<dyn Error>> {
        let current_source = self.rendering.as_ref().is_some_and(|rendering| {
            self.admitted_source_is_current(rendering.document, rendering.source_revision)
        });
        if !self.closing
            && current_source
            && self.drawable
            && self.pending_open.is_none()
            && self.loading.is_none()
            && self.document.tools.navigation().resampling()
            && self.rendering.as_ref().is_some_and(|rendering| {
                rendering.tier == BackingTier::Detail
                    && !rendering.superseded
                    && rendering.coverage.viewport().zoom()
                        < self.document.tools.navigation().viewport().zoom()
                    && self.has_finer_ready_detail(rendering.coverage.viewport())
            })
        {
            return Ok(());
        }
        let backing_borrowed =
            self.rendering.as_ref().is_some_and(|rendering| match rendering.tier {
                BackingTier::Fallback => self
                    .document
                    .fallback_front
                    .as_ref()
                    .is_some_and(|front| Arc::strong_count(front) > 1),
                BackingTier::Detail => {
                    self.document.front.as_ref().is_some_and(|front| Arc::strong_count(front) > 1)
                }
                BackingTier::Navigation => self
                    .document
                    .navigation_front
                    .as_ref()
                    .is_some_and(|front| Arc::strong_count(front) > 1),
            });
        if self.config.gpu.is_none()
            && current_source
            && !self.closing
            && self.drawable
            && self.pending_open.is_none()
            && (self.document.cpu_pending.is_some()
                || backing_borrowed
                || (self.presentation.attached()
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
        if !current_source {
            match Self::terminal(completion.outcome) {
                Ok(packet) => {
                    self.retire(Retired { _packet: Some(packet), ..Default::default() })?
                }
                Err(cause) => {
                    tracing::debug!(operation="canvas_frame_stopped", job_id=rendering.job.spec().id.get(), document_id=rendering.document.0, document_revision=rendering.document.1, source_revision=rendering.source_revision, cause=%cause, "Retired canvas source reached its actual terminal outcome");
                }
            }
            return Ok(());
        }
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
                    || packet.document != rendering.document
                    || packet.source_revision != rendering.source_revision
                    || packet.reconstruction != rendering.reconstruction
                    || packet.view != rendering.coverage.viewport()
                    || !packet.sampling_scale.is_finite()
                    || packet.sampling_scale <= 0.0
                    || packet.sampling_scale > packet.view.zoom()
                {
                    self.retire(Retired { _packet: Some(packet), ..Default::default() })?;
                    return Err("Canvas received foreign backing coverage".into());
                }
                let current =
                    if self.closing || self.pending_open.is_some() || self.loading.is_some() {
                        false
                    } else if let Some(scene) = &self.document.scene {
                        let snapshot = scene.document.snapshot();
                        packet.document == (snapshot.identity(), snapshot.revision())
                            && packet.source_revision == scene.source_revision
                            && packet.source_revision == self.selection
                            && completed_coverage_is_current(
                                packet.tier,
                                rendering.coverage,
                                self.document.tools.navigation().viewport(),
                                snapshot.extent(),
                            )?
                    } else {
                        false
                    };
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
                                    self.document.fallback_front.replace(Arc::new(surface));
                            }
                            BackingTier::Detail => {
                                retired._front = self.document.front.replace(Arc::new(surface));
                            }
                            BackingTier::Navigation => {
                                retired._navigation_front =
                                    self.document.navigation_front.replace(Arc::new(surface));
                            }
                        }
                        Ok(work)
                    }
                    work::Pixels::Cpu(work::CpuPacket::Repair(update)) => {
                        let retained = match packet.tier {
                            BackingTier::Fallback => &mut self.document.fallback_front,
                            BackingTier::Detail => &mut self.document.front,
                            BackingTier::Navigation => &mut self.document.navigation_front,
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
                                    self.document.fallback_gpu_front.replace(pixels.surface);
                                if let Some(snapshot) = pixels.snapshot {
                                    retired._fallback_front =
                                        self.document.fallback_front.replace(Arc::new(snapshot));
                                }
                            }
                            BackingTier::Detail => {
                                retired._gpu_front =
                                    self.document.gpu_front.replace(pixels.surface);
                                if let Some(snapshot) = pixels.snapshot {
                                    retired._front =
                                        self.document.front.replace(Arc::new(snapshot));
                                }
                            }
                            BackingTier::Navigation => {
                                retired._navigation_gpu_front =
                                    self.document.navigation_gpu_front.replace(pixels.surface);
                                if let Some(snapshot) = pixels.snapshot {
                                    retired._navigation_front =
                                        self.document.navigation_front.replace(Arc::new(snapshot));
                                }
                            }
                        }
                        self.document.native_verified = false;
                        self.expose_pending = true;
                        Ok(pixels.work)
                    }
                };
                if retired._front.is_some()
                    || retired._gpu_front.is_some()
                    || retired._fallback_front.is_some()
                    || retired._fallback_gpu_front.is_some()
                    || retired._navigation_front.is_some()
                    || retired._navigation_gpu_front.is_some()
                    || retired._update.is_some()
                    || retired._projection.is_some()
                    || retired._buffers.iter().any(Option::is_some)
                {
                    self.retire(retired)?;
                }
                let work = work?;
                if packet.tier == BackingTier::Fallback && rendering.reconstruction.is_preview() {
                    self.document.initial_preview = false;
                }
                let shown = Presented {
                    sampling_scale: packet.sampling_scale,
                    tier: packet.tier,
                    coverage: rendering.coverage,
                    view: packet.view,
                    reconstruction: packet.reconstruction,
                    document: packet.document,
                    source_revision: packet.source_revision,
                };
                match packet.tier {
                    BackingTier::Fallback => self.document.fallback_presented = Some(shown),
                    BackingTier::Detail => self.document.presented = Some(shown),
                    BackingTier::Navigation => self.document.navigation_presented = Some(shown),
                }
                self.last_prepare = packet.elapsed;
                self.last_stats = packet.stats;
                self.last_surface_work = work;
                if self.document.scene.as_ref().is_some_and(|scene| scene.source.is_some()) {
                    self.title =
                        self.labels.viewing(self.document.tools.navigation().viewport().zoom());
                }
            }
            Err(cause) if self.closing || self.pending_open.is_some() => {
                tracing::debug!(operation="canvas_frame_stopped", cause=%cause, "Canvas frame reached a terminal outcome before replacement")
            }
            Err(cause) => {
                if !self.document.rebuilt
                    && corrupt(cause.as_ref())
                    && let Some(scene) = &self.document.scene
                    && let (Some(original), Some(source)) = (&scene.original, &scene.source)
                {
                    let mut original = Some(original.clone());
                    let expected = source.descriptor().asset();
                    self.document.rebuilt = true;
                    if !self.offer_source(&mut original, CacheReuse::Rebuild, Some(expected))? {
                        return Err("Canvas rebuild source could not be retained".into());
                    }
                    return Ok(());
                }
                self.fail(cause);
            }
        }
        Ok(())
    }
}
/// Completed detail remains publishable while any of its current document pixels stay visible.
pub(super) fn completed_coverage_is_current(
    tier: BackingTier,
    coverage: BackingCoverage,
    target: Viewport,
    document: sugarsubstitute_image_ops::Extent,
) -> Result<bool, Box<dyn Error>> {
    Ok(tier == BackingTier::Fallback || coverage.overlaps(target, document)?)
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
