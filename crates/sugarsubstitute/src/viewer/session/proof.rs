// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive the real viewer coordinator through a bounded headless navigation and artifact lifetime.
mod motion;
mod native;
use super::*;
use crate::viewer::{
    options::ProofPaths,
    proof::{self, Input as ProofInput, ProofBacking, Sample},
};
use motion::Motion;
use std::{num::NonZeroUsize, time::Instant};
use sugarsubstitute_canvas::{ScrollPhase, ToolButton, ToolInput};
use sugarsubstitute_desktop::{
    HeadlessLifetime, HeadlessStep, StartupPreparationStep,
    canvas::{Input, Lifetime},
};

struct Proof {
    session: Session,
    pending_selection: Option<PathBuf>,
    source_selection_port_exercised: bool,
    fallback_before_phase_six: Option<u64>,
    zoom_out_fallback_exercised: bool,
    paths: Option<ProofPaths>,
    phase: u8,
    samples: [Option<Sample>; 32],
    observed: Option<FrameStamp>,
    previous_stats: sugarsubstitute_image_ops::EvaluationStats,
    pending: Option<ProofInput>,
    publishing: Option<OwnedJob<(), WorkError>>,
    complete: bool,
    cutoff: Instant,
    motion: Option<Motion>,
    motion_duration: Duration,
    motion_done: bool,
    motion_collecting: bool,
    motion_stats: Option<sugarsubstitute_canvas_render::PresentationStats>,
    warmup_stats: Option<sugarsubstitute_canvas_render::PresentationStats>,
}
impl Session {
    /// Exercise original loading, settled zoom, retained pan and checked shutdown through either
    /// a windowless event pump or the ordinary hidden native window adapter.
    pub(in crate::viewer) fn run_proof(
        mut self,
        paths: ProofPaths,
        window: Option<sugarsubstitute_desktop::canvas::WindowOptions>,
    ) -> Result<(), Box<dyn Error>> {
        let motion_duration = paths.motion_duration;
        let pending_selection =
            self.options.image.take().ok_or("Viewer proof source selection is unavailable")?;
        let mut proof = Proof {
            session: self,
            pending_selection: Some(pending_selection),
            source_selection_port_exercised: false,
            fallback_before_phase_six: None,
            zoom_out_fallback_exercised: false,
            paths: Some(paths),
            phase: 0,
            samples: [None; 32],
            observed: None,
            previous_stats: Default::default(),
            pending: None,
            publishing: None,
            complete: false,
            cutoff: Instant::now()
                .checked_add(Duration::from_secs(900))
                .ok_or("Proof deadline overflow")?,
            motion: None,
            motion_duration,
            motion_done: motion_duration.is_zero(),
            motion_collecting: false,
            motion_stats: None,
            warmup_stats: None,
        };
        match window {
            Some(window) => sugarsubstitute_desktop::canvas::run(window, &mut proof)?,
            None => sugarsubstitute_desktop::run_headless(&mut proof)?,
        }
        Ok(())
    }
}
impl Proof {
    /// Advance only after an actual applied frame matches current source, revision and camera.
    fn navigate(&mut self) -> Result<(), Box<dyn Error>> {
        if self.motion.is_some() {
            return self.drive_motion();
        }
        let session = &mut self.session;
        if !session.displayed_coverage_is_valid()? {
            return Err("Viewer published output without current document coverage".into());
        }
        if session.loading.is_some()
            || session.pending_open.is_some()
            || session.scene.as_ref().is_none_or(|scene| scene.source.is_none())
        {
            return Ok(());
        }
        let Some(shown) = &session.presented else {
            return Ok(());
        };
        let target_view = session.tools.navigation().viewport();
        let scene = session.scene.as_ref().ok_or("Proof document is unavailable")?;
        let snapshot = scene.document.snapshot();
        let Some(_) = shown.coverage.crop(target_view, snapshot.extent())? else {
            return Ok(());
        };
        if !shown.view.shares_sampling_grid(target_view) {
            return Ok(());
        }
        let stamp = session.current_frame().ok_or("Proof lost its displayed frame")?;
        if session.displaying.is_some()
            || session.cpu_pending.is_some()
            || session.display_source != Some(stamp)
            || session.display_view != Some(target_view)
        {
            return Ok(());
        }
        if session.options.gpu.is_some()
            && session.native_target.is_some()
            && (session.gpu_presentation_pending()
                || session.native_presented.is_none()
                || !session.native_verified)
        {
            return Ok(());
        }
        if self.observed != Some(stamp) {
            let (computed_pixels, source_pixels) =
                proof::difference(session.last_stats, self.previous_stats);
            let slot = self
                .samples
                .iter_mut()
                .find(|slot| slot.is_none())
                .ok_or("Viewer proof exceeded its bounded frame inventory")?;
            *slot = Some(Sample {
                phase: self.phase,
                physical_zoom: target_view.zoom(),
                nearest_reconstruction: shown.reconstruction.filter()
                    == sugarsubstitute_image_ops::Filter::Nearest,
                preview: shown.reconstruction.is_preview(),
                prepare_microseconds: session.last_prepare.as_micros(),
                computed_pixels,
                source_pixels,
                repaired_pixels: session.last_surface_work.repaired_pixels,
                reused_pixels: session.last_surface_work.reused_pixels,
            });
            self.previous_stats = session.last_stats;
            self.observed = Some(stamp);
        }
        if shown.reconstruction.is_preview()
            || shown.reconstruction
                != Reconstruction::finished_for_display_zoom(target_view.zoom())?
            || session.rendering.is_some()
            || session.tools.navigation().interacting()
        {
            return Ok(());
        }
        if !self.motion_done {
            let now = session.navigation_time();
            let Some(stats) = take_settled_presentation_stats(&mut session.pacing, now)? else {
                return Ok(());
            };
            self.warmup_stats = Some(stats);
            self.motion_collecting = true;
            self.motion = Some(Motion::new(
                session.navigation_time(),
                session.tools.navigation().frame_period(),
                self.motion_duration,
            )?);
            return self.drive_motion();
        }
        if self.motion_collecting {
            let now = session.navigation_time();
            let Some(stats) = take_settled_presentation_stats(&mut session.pacing, now)? else {
                return Ok(());
            };
            self.motion_stats = Some(stats);
            self.motion_collecting = false;
        }
        if self.phase == 7 && !self.zoom_out_fallback_exercised {
            let before = self
                .fallback_before_phase_six
                .ok_or("Proof lost its phase-six fallback acceptance baseline")?;
            if session.fallback_projection_acceptances <= before {
                return Err("One-to-one zoom-out did not publish a fallback projection".into());
            }
            self.zoom_out_fallback_exercised = true;
        }
        let center = [
            f64::from(session.tools.navigation().viewport().surface().width()) * 0.5,
            f64::from(session.tools.navigation().viewport().surface().height()) * 0.5,
        ];
        match self.phase {
            0 => {
                session.input(Input::Tool(ToolInput::Pointer(center)))?;
                session.input(Input::Tool(ToolInput::OneToOne))?;
            }
            6 => {
                self.fallback_before_phase_six = Some(session.fallback_projection_acceptances);
                session.input(Input::Tool(ToolInput::Pointer(center)))?;
                session.input(Input::Tool(ToolInput::OneToOne))?;
            }
            // Cross the inner repair band while remaining within the outer backing guard.
            1 | 7 => pan(session, center, [97.0, -83.0])?,
            2 | 8 => pan(session, center, [-79.0, 61.0])?,
            3 => {
                session.input(Input::Tool(ToolInput::Pointer(center)))?;
                session.input(Input::Tool(ToolInput::Wheel {
                    steps: 4.0,
                    phase: ScrollPhase::Unphased,
                }))?;
            }
            4 => pan(session, center, [13.0, -7.0])?,
            5 => pan(session, center, [-5.0, 3.0])?,
            9 => session.input(Input::Tool(ToolInput::Fit))?,
            10 => {
                session.input(Input::Tool(ToolInput::Pointer(center)))?;
                session.input(Input::Tool(ToolInput::Wheel {
                    steps: -4.0,
                    phase: ScrollPhase::Unphased,
                }))?;
            }
            11 => {
                let extent = snapshot.extent();
                let fit = (f64::from(target_view.surface().width()) / f64::from(extent.width()))
                    .min(f64::from(target_view.surface().height()) / f64::from(extent.height()));
                if target_view.zoom() >= fit {
                    return Err("Viewer proof did not reach below-Fit zoom".into());
                }
                session.input(Input::Tool(ToolInput::Fit))?;
            }
            12 => return self.prepare_output(),
            _ => return Err("Invalid viewer proof navigation phase".into()),
        }
        self.phase += 1;
        session.advance()
    }
    /// Transfer the actual retained surface only after admission; output paths never imply overwrite.
    fn prepare_output(&mut self) -> Result<(), Box<dyn Error>> {
        let session = &mut self.session;
        if self.pending.is_none() {
            if !self.source_selection_port_exercised {
                return Err("Proof did not exercise the source selection lifetime port".into());
            }
            if session.coverage_acceptances == 0 {
                return Err("Proof observed no coverage-validated native presentations".into());
            }
            if !self.zoom_out_fallback_exercised {
                return Err("Proof observed no fallback presentation during zoom-out".into());
            }
            let scene = session.scene.as_ref().ok_or("Proof document is unavailable")?;
            let document = scene.document.snapshot();
            let target = session.tools.navigation().viewport();
            let shown = session.presented.ok_or("Proof detailed backing is unavailable")?;
            let projection = shown
                .coverage
                .crop(target, document.extent())?
                .ok_or("Proof detailed backing does not cover its output camera")?;
            let backing = match session.options.gpu {
                Some(_) => ProofBacking::Gpu(
                    session
                        .gpu_front
                        .as_ref()
                        .and_then(GpuSurface::frame)
                        .ok_or("Proof GPU backing is unavailable")?
                        .clone(),
                ),
                None => ProofBacking::Cpu(
                    session.front.take().ok_or("Proof CPU backing is unavailable")?,
                ),
            };
            self.pending = Some(ProofInput {
                resources: session
                    .resources
                    .as_ref()
                    .ok_or("Proof resources are unavailable")?
                    .clone(),
                document,
                source: scene.source.clone(),
                view: target,
                backing_view: shown.view,
                projection,
                reconstruction: shown.reconstruction,
                backing,
                samples: self.samples,
                paths: self.paths.take().ok_or("Proof output custody was lost")?,
                adapter: session.gpu_descriptor.clone(),
                presentations: session.pacing.stats(),
                motion: self.motion_stats,
                warmup: self.warmup_stats,
                canonical_gpu_snapshot_readback: session.options.gpu.is_some()
                    && session.options.capture,
                native_gpu_display_verification_readback: session.options.gpu.is_some()
                    && session.options.verify_native
                    && session.native_target.is_some(),
                coverage_acceptances: session.coverage_acceptances,
                fallback_projection_acceptances: session.fallback_projection_acceptances,
                navigation_projection_acceptances: session.navigation_projection_acceptances,
                bootstrap_projection_acceptances: session.bootstrap_projection_acceptances,
                zoom_out_fallback_exercised: self.zoom_out_fallback_exercised,
                first_fallback_preparation_io: session.first_fallback_preparation_io,
                first_image_queue_latency: session.first_image_queue_latency,
                source_selection_port_exercised: self.source_selection_port_exercised,
            });
        }
        let runtime = session.runtime.as_ref().ok_or("Proof execution is unavailable")?;
        let spec = session.spec(
            Operation::Infrastructure,
            JobClass::InfrastructureOrdinary,
            work::IMPORT_BYTES,
            Duration::from_secs(300),
            None,
        )?;
        match runtime.client.submit_blocking_owned_input(
            &runtime.scope,
            spec,
            &mut self.pending,
            |input, context| proof::publish(input, &context),
        ) {
            Ok(job) => self.publishing = Some(job),
            Err(sugarsubstitute_execution::OwnedInputError::Admission(cause))
                if super::activity::retryable(&cause) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }
}
impl Proof {
    /// Deliver one latest due qualification observation through the ordinary normalized input port.
    fn drive_motion(&mut self) -> Result<(), Box<dyn Error>> {
        if !self.session.displayed_coverage_is_valid()? {
            return Err("Motion published output without current document coverage".into());
        }
        let now = self.session.navigation_time();
        let motion = self.motion.as_mut().ok_or("Proof motion lifetime is unavailable")?;
        if let Some(step) = motion.next(now)? {
            self.session
                .input(Input::Tool(ToolInput::Wheel { steps: step.steps, phase: step.phase }))?;
        }
        if motion.finished() {
            self.motion = None;
            self.motion_done = true;
            self.session.input(Input::Tool(ToolInput::Fit))?;
        }
        self.session.advance()
    }
}

/// Split qualification statistics only after every submitted presentation releases pacer custody.
fn take_settled_presentation_stats(
    pacing: &mut PresentationPacer,
    now: Duration,
) -> Result<Option<sugarsubstitute_canvas_render::PresentationStats>, Box<dyn Error>> {
    if matches!(pacing.readiness(now)?, PresentationReadiness::Busy { .. }) {
        Ok(None)
    } else {
        Ok(Some(pacing.take_stats()))
    }
}

/// Native observations exercise exactly the same camera input path used by a visible window.
fn pan(session: &mut Session, start: [f64; 2], delta: [f64; 2]) -> Result<(), Box<dyn Error>> {
    session.input(Input::Tool(ToolInput::Pointer(start)))?;
    session.input(Input::Tool(ToolInput::Button { button: ToolButton::Primary, pressed: true }))?;
    session.input(Input::Tool(ToolInput::Pointer([start[0] + delta[0], start[1] + delta[1]])))?;
    session.input(Input::Tool(ToolInput::Button { button: ToolButton::Primary, pressed: false }))
}

#[cfg(test)]
#[path = "proof/tests.rs"]
mod tests;
impl HeadlessLifetime for Proof {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>> {
        self.session.start(wake)
    }
    fn prepare_native_runtime(
        &mut self,
        _: NonZeroUsize,
    ) -> Result<StartupPreparationStep, Box<dyn Error>> {
        Ok(StartupPreparationStep::Ready)
    }
    fn activity(&mut self, _: NonZeroUsize) -> Result<HeadlessStep, Box<dyn Error>> {
        self.session.poll(Duration::ZERO)?;
        if self.session.take_picker_request() {
            if self.source_selection_port_exercised {
                return Err("Proof received a duplicate source selection request".into());
            }
            let path =
                self.pending_selection.take().ok_or("Proof source selection custody was lost")?;
            self.session.selected(Ok(Some(path)))?;
            self.source_selection_port_exercised = true;
            self.session.poll(Duration::ZERO)?;
        }
        if let Some(outcome) = Session::take(&mut self.publishing) {
            Session::terminal(outcome)?;
            self.complete = true;
        }
        if self.session.closing || self.complete {
            return Ok(HeadlessStep::Shutdown);
        }
        if let Some(cause) = self.session.failed.take() {
            return Err(cause);
        }
        if Instant::now() >= self.cutoff {
            return Err("Viewer proof exceeded its original deadline".into());
        }
        if self.pending.is_some() {
            self.prepare_output()?;
        } else if self.publishing.is_none() {
            self.navigate()?;
        }
        Ok(HeadlessStep::Active)
    }
    fn next_deadline(&self) -> Option<Instant> {
        if self.session.closing {
            None
        } else {
            let ordinary = self
                .session
                .next_deadline()
                .map_or(self.cutoff, |navigation| navigation.min(self.cutoff));
            let motion =
                self.motion.as_ref().and_then(Motion::next_deadline).and_then(|deadline| {
                    Instant::now()
                        .checked_add(deadline.saturating_sub(self.session.navigation_time()))
                });
            Some(motion.map_or(ordinary, |motion| ordinary.min(motion)))
        }
    }
    fn request_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(job) = &self.publishing {
            job.cancel();
        }
        if let Some(input) = self.pending.take() {
            let (front, proof_gpu) = match input.backing {
                ProofBacking::Cpu(front) => (Some(front), None),
                ProofBacking::Gpu(frame) => (None, Some(frame)),
            };
            self.session.retire(Retired {
                _front: front,
                _proof_gpu: proof_gpu,
                _resources: Some(input.resources),
                ..Default::default()
            })?;
        }
        self.session.close()
    }
    fn poll_shutdown(&mut self) -> Result<bool, Box<dyn Error>> {
        if let Some(outcome) = Session::take(&mut self.publishing) {
            match outcome {
                JobOutcome::Cancelled => {}
                outcome => {
                    Session::terminal(outcome)?;
                }
            }
        }
        if self.publishing.is_some() {
            return Ok(false);
        }
        self.session.settled()
    }
    fn finish_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        self.session.finish()
    }
}
