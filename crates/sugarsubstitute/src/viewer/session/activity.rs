// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit coalesced viewer work and owned cleanup through the existing execution boundary.
use super::*;
use sugarsubstitute_execution::BlockingInputError;

impl Session {
    /// Drain metadata and typed completions before submitting the latest application intent.
    pub(super) fn advance(&mut self) -> Result<(), Box<dyn Error>> {
        self.tools.advance(self.navigation_time())?;
        self.cancel_obsolete_refinement();
        if let Some(runtime) = &self.runtime {
            let batch =
                runtime.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
            if batch.notifications.iter().any(|notice| notice.scope_id != runtime.scope.id()) {
                return Err("Canvas received a foreign execution scope notice".into());
            }
        }
        self.consume_initialization()?;
        self.consume_device()?;
        self.consume_load()?;
        self.consume_frame()?;
        if let Some(outcome) = Self::take(&mut self.retirement) {
            if let Err(cause) = Self::terminal(outcome) {
                self.fail(cause);
            }
            self.retirement_deadline = None;
        }
        if self.closing {
            self.advance_close()?;
        }
        self.submit_retirement()?;
        if self.closing || self.pending_retirement.is_some() || self.retirement.is_some() {
            return Ok(());
        }
        if self.initialization_pending {
            self.submit_initialization()?;
        }
        if self.initializing.is_some() || self.resources.is_none() {
            return Ok(());
        }
        if !self.gpu_ready {
            if self.failed.is_none() {
                self.initialize_gpu()?;
            }
            return Ok(());
        }
        if self.pending_open.is_some() {
            if self.loading.is_none() && self.rendering.is_none() {
                self.submit_load()?;
            }
            return Ok(());
        }
        if self.loading.is_none() && self.rendering.is_none() && self.failed.is_none() {
            if self.gpu_presentation.is_some() || self.awaiting_gpu.is_some() {
                return Ok(());
            }
            self.submit_frame()?;
        }
        Ok(())
    }
    /// Startup storage shares the original retained admission used by every later resource.
    fn submit_initialization(&mut self) -> Result<(), Box<dyn Error>> {
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        let spec = self.spec(
            Operation::Infrastructure,
            JobClass::InfrastructureLifecycle,
            if self.options.gpu.is_some() {
                work::GPU_RETAINED_BYTES
            } else {
                work::RETAINED_BYTES
            },
            Duration::from_secs(30),
            None,
        )?;
        let anchor = self.options.cache.clone();
        let capture = self.options.capture;
        match runtime.client.submit_blocking(&runtime.scope, spec, move |context| {
            work::initialize(anchor, capture, &context)
        }) {
            Ok(job) => {
                self.initializing = Some(job);
                self.initialization_pending = false;
            }
            Err(cause) if retryable(&cause) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }
    /// The typed owned-input API keeps the exact selected path intact on admission rejection.
    fn submit_load(&mut self) -> Result<(), Box<dyn Error>> {
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        let resources = self.resources.as_ref().ok_or("Canvas storage is unavailable")?.clone();
        let pending = self.pending_open.as_ref().ok_or("Canvas source intent is unavailable")?;
        let intent = pending.intent;
        let expected = pending.expected;
        let spec = self.spec(
            Operation::Infrastructure,
            JobClass::InfrastructureOrdinary,
            work::IMPORT_BYTES,
            Duration::from_secs(300),
            None,
        )?;
        match runtime.client.submit_blocking_owned_input(
            &runtime.scope,
            spec,
            &mut self.pending_open,
            move |open, context| {
                work::load(open.path, open.intent, open.reuse, &resources, &context)
            },
        ) {
            Ok(job) => self.loading = Some(Loading { intent, expected, job }),
            Err(BlockingInputError::Admission(cause)) if retryable(&cause) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }
    /// Commit a camera request only when the preceding request has actually completed.
    /// Navigation remains coalesced intent while a current document frame is being prepared.
    fn submit_frame(&mut self) -> Result<(), Box<dyn Error>> {
        let scene = self.scene.as_ref().ok_or("Canvas document is unavailable")?;
        let document = scene.document.snapshot();
        let identity = (document.identity(), document.revision());
        let filter = self.filter();
        if !self.expose_pending
            && self.presented.as_ref().is_some_and(|shown| {
                shown.view == self.tools.navigation().viewport()
                    && shown.filter == filter
                    && shown.document == identity
                    && shown.source_revision == scene.source_revision
            })
        {
            return Ok(());
        }
        let extent = self.tools.navigation().viewport().surface();
        let compatible = self.presented.as_ref().filter(|shown| {
            shown.document == identity
                && shown.source_revision == scene.source_revision
                && shown.filter == filter
        });
        let damage = match compatible {
            Some(shown) => self.tools.navigation().viewport().repair_from(shown.view)?,
            None => DamagePlan::full(extent),
        };
        let replace_surface = if self.options.gpu.is_some() {
            self.gpu_front.as_ref().is_none_or(|surface| surface.extent() != extent)
        } else {
            self.front.as_ref().is_none_or(|surface| surface.extent() != extent)
        };
        let base =
            if replace_surface { None } else { compatible.and_then(|_| self.current_frame()) };
        let next = self.frame_revision.checked_add(1).ok_or("Canvas frame revision exhausted")?;
        let target = FrameStamp { surface: SurfaceId::new(1)?, generation: Generation::new(next)? };
        let bytes = if self.options.gpu.is_some() {
            if self.options.capture { work::GPU_PROOF_BYTES } else { work::GPU_PREPARE_BYTES }
        } else {
            damage
                .dirty()
                .pixels()
                .checked_mul(4)
                .and_then(|bytes| bytes.checked_add(256 * 1024))
                .ok_or("Canvas patch reservation overflow")?
        };
        let spec = self.spec(
            Operation::FramePrepare,
            JobClass::CpuInteractive,
            bytes,
            Duration::from_secs(300),
            Some(ReplacementKey { surface: target.surface, revision: target.generation }),
        )?;
        let resources =
            self.resources.as_ref().ok_or("Canvas preparation resources are unavailable")?.clone();
        let source = scene.source.clone();
        let source_revision = scene.source_revision;
        let view = self.tools.navigation().viewport();
        let gpu_surface = self.gpu_front.clone();
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        match runtime.client.submit_cpu(&runtime.scope, spec, move |_| work::Prepare {
            resources,
            document,
            source,
            source_revision,
            view,
            filter,
            target,
            base,
            damage,
            replace_surface,
            gpu_surface,
            finished: false,
            cpu: None,
            started: None,
        }) {
            Ok(job) => {
                self.rendering = Some(Rendering { job, filter, superseded: false });
                self.frame_revision = next;
            }
            Err(cause) if retryable(&cause) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }
    /// Fund and transfer disposal metadata only after actual admission; never drop source handles
    /// or large prepared payloads on the UI thread merely because a newer selection superseded them.
    fn submit_retirement(&mut self) -> Result<(), Box<dyn Error>> {
        if self.retirement.is_some() || self.pending_retirement.is_none() {
            return Ok(());
        }
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        let mut spec = self.spec(
            Operation::Infrastructure,
            JobClass::InfrastructureLifecycle,
            64 * 1024,
            Duration::from_secs(5),
            None,
        )?;
        spec.deadline = self.retirement_deadline;
        match runtime.client.submit_blocking_owned_box(
            &runtime.scope,
            spec,
            &mut self.pending_retirement,
            |value, _| {
                drop(value);
                Ok::<(), WorkError>(())
            },
        ) {
            Ok(job) => self.retirement = Some(job),
            Err(BlockingInputError::Admission(cause)) if retryable(&cause) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }
    /// Dispose retained state while cleanup admission is still open, then ask execution to stop.
    fn advance_close(&mut self) -> Result<(), Box<dyn Error>> {
        if self.initializing.is_some()
            || self.device_job.is_some()
            || self.loading.is_some()
            || self.rendering.is_some()
            || self.pending_retirement.is_some()
            || self.retirement.is_some()
        {
            return Ok(());
        }
        if self.resources.is_some()
            || self.scene.is_some()
            || self.front.is_some()
            || self.gpu_front.is_some()
            || self.gpu_presentation.is_some()
        {
            let retired = Retired {
                _scene: self.scene.take(),
                _resources: self.resources.take(),
                _front: self.front.take(),
                _gpu_front: self.gpu_front.take(),
                _presentation: self.gpu_presentation.take(),
                ..Default::default()
            };
            self.presented = None;
            self.retire(retired)?;
        } else if !self.shutdown_started {
            if let Some(runtime) = &mut self.runtime {
                runtime
                    .owner
                    .request_shutdown(self.clock.now().checked_add(Duration::from_secs(5))?);
            }
            self.shutdown_started = true;
        }
        Ok(())
    }
}
/// Actual releases notify this execution owner's completion wake when a feasible retry can run.
pub(super) fn retryable(cause: &AdmissionError) -> bool {
    matches!(
        cause,
        AdmissionError::QueueSaturated { .. }
            | AdmissionError::MemorySaturated { .. }
            | AdmissionError::CompletionSaturated
    )
}
