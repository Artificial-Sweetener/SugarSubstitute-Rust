// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Submit preview imports and cooperative preparation through the session's existing scope.

use super::{preview::*, *};
use sugarsubstitute_execution::OwnedInputError;
mod tasks;
use tasks::{PrepareTask, ReprojectTask};

impl Session {
    /// Observe one active candidate before considering another import or ordinary display work.
    pub(in crate::native_canvas::session) fn advance_preview(
        &mut self,
    ) -> Result<(), Box<dyn Error>> {
        self.advance_preview_retirement()?;
        if self.deferred_retirement.is_some() {
            return Ok(());
        }
        if self.closing || self.preview.restoring {
            self.withdraw_preview()?;
        }
        if self.deferred_retirement.is_some() {
            return Ok(());
        }
        if self.preview.candidate.is_none()
            && !self.closing
            && !self.preview.restoring
            && self.preview.resize_job.is_none()
            && let Some(input) = self.preview.pending.take()
        {
            self.preview.candidate = Some(Candidate {
                token: input.token,
                deadline: input.deadline,
                cancelled: input.cancelled,
                expired: false,
                owner_selection: input.owner_selection,
                stage: Stage::Input(Some(input)),
                custody: None,
            });
        }
        let Some(mut candidate) = self.preview.candidate.take() else {
            return self.advance_preview_restoration();
        };
        if !candidate.cancelled && self.clock.now() >= candidate.deadline {
            candidate.expired = true;
            candidate.cancelled = true;
        }
        if self.closing || self.selection != candidate.owner_selection {
            candidate.cancelled = true;
        }
        let outcome = match &mut candidate.stage {
            Stage::Loading(job) => Self::take(job)
                .map(|outcome| Self::terminal(outcome).map(|scene| Stage::Scene(Some(scene)))),
            _ => None,
        };
        if let Some(outcome) = outcome {
            match outcome {
                Ok(stage) => candidate.stage = stage,
                Err(cause) => {
                    self.record_preview_failure(
                        candidate.token,
                        candidate.cancelled && !candidate.expired,
                        cause,
                    );
                    self.retire(Retired {
                        _preview: Some(PreviewRetired {
                            _custody: candidate.custody.take(),
                            ..Default::default()
                        }),
                        ..Default::default()
                    })?;
                    return self.advance_preview_restoration();
                }
            }
        }
        if let Stage::Preparing(job) = &mut candidate.stage
            && let Some(outcome) = Self::take(job)
        {
            match Self::terminal(outcome) {
                Ok(ready) => candidate.stage = Stage::Ready(Some(Box::new(ready))),
                Err(cause) => {
                    self.record_preview_failure(
                        candidate.token,
                        candidate.cancelled && !candidate.expired,
                        cause,
                    );
                    self.retire(Retired {
                        _preview: Some(PreviewRetired {
                            _custody: candidate.custody.take(),
                            ..Default::default()
                        }),
                        ..Default::default()
                    })?;
                    return self.advance_preview_restoration();
                }
            }
        }
        if candidate.cancelled {
            match &candidate.stage {
                Stage::Loading(Some(job)) => job.cancel(),
                Stage::Preparing(Some(job)) => job.cancel(),
                _ => {
                    self.retire_preview_candidate(candidate)?;
                    return self.advance_preview_restoration();
                }
            }
        } else {
            let result = self.submit_preview_stage(&mut candidate);
            self.preview.candidate = Some(candidate);
            result?;
            return self.advance_preview_restoration();
        }
        self.preview.candidate = Some(candidate);
        self.advance_preview_restoration()
    }

    /// Keep diagnostics owned by preview failure without failing the displayed final document.
    fn record_preview_failure(
        &mut self,
        token: PreviewToken,
        cancelled: bool,
        cause: Box<dyn Error>,
    ) {
        tracing::debug!(operation="canvas_preview_stopped", preview_id=token.0, cause=%cause, "Preview reached a terminal outcome");
        self.preview.last =
            Some((token, if cancelled { PreviewStatus::Cancelled } else { PreviewStatus::Failed }));
    }

    /// Bind every retry and every stage to the application-supplied original deadline.
    fn preview_spec(
        &self,
        candidate: &Candidate,
        storage: bool,
        target: FrameStamp,
    ) -> Result<JobSpec, Box<dyn Error>> {
        let view = self.document.tools.navigation().viewport().surface();
        let samples = (u64::from(view.width()) + u64::from(view.height()))
            .checked_mul(std::mem::size_of::<sugarsubstitute_image_ops::DisplaySample>() as u64)
            .ok_or("Preview sample reservation overflow")?;
        let patch = match &candidate.stage {
            Stage::Scene(Some(scene)) => coverage::fallback(scene.document.snapshot().extent())?
                .viewport()
                .surface()
                .pixels()
                .checked_mul(4)
                .ok_or("Preview patch reservation overflow")?,
            _ => 0,
        };
        let bytes = if storage {
            work::IMPORT_BYTES
        } else {
            patch
                .checked_add(samples)
                .and_then(|bytes| bytes.checked_add(256 * 1024))
                .ok_or("Preview reservation overflow")?
        };
        let evaluating = matches!(candidate.stage, Stage::Scene(_));
        let mut spec = self.spec(
            if storage {
                Operation::Infrastructure
            } else if evaluating {
                Operation::ImageEvaluate
            } else {
                Operation::FramePrepare
            },
            if storage {
                JobClass::InfrastructureOrdinary
            } else if evaluating {
                JobClass::CpuDecode
            } else {
                JobClass::CpuInteractive
            },
            bytes,
            Duration::from_secs(30),
            if storage || evaluating {
                None
            } else {
                Some(ReplacementKey {
                    surface: SurfaceId::new(PREVIEW_PROJECTION_SURFACE)?,
                    revision: target.generation,
                })
            },
        )?;
        spec.deadline = Some(candidate.deadline);
        Ok(spec)
    }

    /// Preserve constructor input on admission refusal and advance only its exact original stage.
    fn submit_preview_stage(&mut self, candidate: &mut Candidate) -> Result<(), Box<dyn Error>> {
        if matches!(candidate.stage, Stage::Offered(_, _) | Stage::Loading(_) | Stage::Preparing(_))
        {
            return Ok(());
        }
        let storage = matches!(candidate.stage, Stage::Input(_));
        let resources =
            self.resources.as_ref().ok_or("Canvas preview resources are unavailable")?.clone();
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        let extent = self.document.tools.navigation().viewport().surface();
        if matches!(&candidate.stage, Stage::Ready(Some(ready)) if ready.drawing.view.surface() == extent)
        {
            return self.offer_ready_preview(candidate);
        }
        let next = self.frame_revision.checked_add(1).ok_or("Canvas display revision exhausted")?;
        let next = match &candidate.stage {
            Stage::Ready(Some(ready)) => next.max(
                ready
                    .drawing
                    .target
                    .generation
                    .get()
                    .checked_add(1)
                    .ok_or("Canvas display revision exhausted")?,
            ),
            _ => next,
        };
        let target = FrameStamp { surface: SurfaceId::new(1)?, generation: Generation::new(next)? };
        let display_target = if matches!(candidate.stage, Stage::Scene(_)) {
            FrameStamp {
                surface: target.surface,
                generation: Generation::new(
                    next.checked_add(1).ok_or("Canvas display revision exhausted")?,
                )?,
            }
        } else {
            target
        };
        let spec = self.preview_spec(candidate, storage, target)?;
        let result = match &mut candidate.stage {
            Stage::Input(input) => runtime
                .client
                .submit_blocking_owned_input(&runtime.scope, spec, input, move |input, context| {
                    work::load(
                        work::Originals::single(input.source),
                        input.revision,
                        CacheReuse::Compatible,
                        &resources,
                        &context,
                    )
                })
                .map(|job| Some(Stage::Loading(Some(job)))),
            Stage::Scene(scene) => {
                if candidate.custody.is_none()
                    && let Some(scene) = scene.as_ref()
                {
                    candidate.custody = Some(SourceCustody {
                        _original: scene.original.clone(),
                        _source: scene.source.clone(),
                    });
                }
                runtime
                    .client
                    .submit_cpu_owned_input(&runtime.scope, spec, scene, move |scene, _| {
                        PrepareTask::new(work::preview::PreparePreview::new(
                            scene,
                            &resources,
                            target,
                            display_target,
                            extent,
                        ))
                    })
                    .map(|job| Some(Stage::Preparing(Some(job))))
            }
            Stage::Ready(ready)
                if ready.as_ref().is_some_and(|ready| ready.drawing.view.surface() != extent) =>
            {
                runtime
                    .client
                    .submit_cpu_owned_input(&runtime.scope, spec, ready, move |ready, _| {
                        ReprojectTask { ready: Some(ready), target, extent }
                    })
                    .map(|job| Some(Stage::Preparing(Some(job))))
            }
            _ => return self.offer_ready_preview(candidate),
        };
        match result {
            Ok(Some(stage)) => {
                if matches!(stage, Stage::Preparing(_)) {
                    self.frame_revision = display_target.generation.get();
                }
                candidate.stage = stage;
            }
            Ok(None) => {}
            Err(OwnedInputError::Admission(cause)) if activity::retryable(&cause) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }

    /// Transfer unpublished custody to the existing disposal worker after actual terminal outcome.
    fn retire_preview_candidate(&mut self, candidate: Candidate) -> Result<(), Box<dyn Error>> {
        let mut retired = Retired {
            _preview: Some(PreviewRetired { _custody: candidate.custody, ..Default::default() }),
            ..Default::default()
        };
        match candidate.stage {
            Stage::Input(input) => {
                retired._preview.as_mut().ok_or("Preview retirement disappeared")?._input = input
            }
            Stage::Scene(scene) => retired._scene = scene,
            Stage::Ready(ready) => {
                retired._preview.as_mut().ok_or("Preview retirement disappeared")?._ready = ready
            }
            Stage::Offered(mut document, resources) => {
                if let Some(target) = document.cpu_pending.take() {
                    self.pacing.abandoned(target, self.navigation_time())?;
                }
                let value = retired._preview.as_mut().ok_or("Preview retirement disappeared")?;
                value._document = Some(document);
                value._resources = resources;
            }
            Stage::Loading(_) | Stage::Preparing(_) => {
                return Err("Preview worker outcome was not observed".into());
            }
        }
        self.preview.last = Some((
            candidate.token,
            if candidate.expired { PreviewStatus::Failed } else { PreviewStatus::Cancelled },
        ));
        self.retire(retired)
    }
}
