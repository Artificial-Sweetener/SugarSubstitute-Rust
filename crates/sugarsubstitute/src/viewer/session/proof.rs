// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive the real viewer coordinator through a bounded headless navigation and artifact lifetime.
mod native;
use super::*;
use crate::viewer::{
    options::ProofPaths,
    proof::{self, Input as ProofInput, Sample},
};
use std::{num::NonZeroUsize, time::Instant};
use sugarsubstitute_desktop::{
    HeadlessLifetime, HeadlessStep, StartupPreparationStep,
    canvas::{Input, Lifetime, WheelDelta},
};

struct Proof {
    session: Session,
    paths: Option<ProofPaths>,
    phase: u8,
    samples: [Option<Sample>; 32],
    observed: Option<FrameStamp>,
    previous_stats: sugarsubstitute_image_ops::EvaluationStats,
    pending: Option<ProofInput>,
    publishing: Option<OwnedJob<(), WorkError>>,
    complete: bool,
    cutoff: Instant,
}
impl Session {
    /// Exercise original loading, settled zoom, retained pan and checked shutdown through either
    /// a windowless event pump or the ordinary hidden native window adapter.
    pub(in crate::viewer) fn run_proof(
        self,
        paths: ProofPaths,
        window: Option<sugarsubstitute_desktop::canvas::WindowOptions>,
    ) -> Result<(), Box<dyn Error>> {
        let mut proof = Proof {
            session: self,
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
        let session = &mut self.session;
        if session.loading.is_some()
            || session.pending_open.is_some()
            || session.rendering.is_some()
            || session.scene.as_ref().is_none_or(|scene| scene.source.is_none())
        {
            return Ok(());
        }
        let Some(shown) = &session.presented else {
            return Ok(());
        };
        if shown.view != session.view {
            return Ok(());
        }
        let stamp = session
            .front
            .as_ref()
            .and_then(CpuSurface::current)
            .ok_or("Proof lost its displayed frame")?;
        if session.options.gpu.is_some()
            && session.native_target.is_some()
            && (session.native_presented != Some(stamp) || !session.native_verified)
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
                preview: shown.filter == Filter::Nearest,
                prepare_microseconds: session.last_prepare.as_micros(),
                computed_pixels,
                source_pixels,
                repaired_pixels: session.last_surface_work.repaired_pixels,
                reused_pixels: session.last_surface_work.reused_pixels,
            });
            self.previous_stats = session.last_stats;
            self.observed = Some(stamp);
        }
        if shown.filter != Filter::Lanczos3 {
            return Ok(());
        }
        let center = [
            f64::from(session.view.surface().width()) * 0.5,
            f64::from(session.view.surface().height()) * 0.5,
        ];
        match self.phase {
            0 | 4 => {
                session.input(Input::Pointer(center))?;
                session.input(Input::ActualSize)?;
            }
            1 | 5 => pan(session, center, [17.0, -11.0])?,
            2 | 6 => pan(session, center, [-7.0, 3.0])?,
            3 => {
                session.input(Input::Pointer(center))?;
                session.input(Input::Wheel(WheelDelta::Lines([0.0, 2.0])))?;
            }
            7 => return self.prepare_output(),
            _ => return Err("Invalid viewer proof navigation phase".into()),
        }
        self.phase += 1;
        session.advance()
    }
    /// Transfer the actual retained surface only after admission; output paths never imply overwrite.
    fn prepare_output(&mut self) -> Result<(), Box<dyn Error>> {
        let session = &mut self.session;
        if self.pending.is_none() {
            let scene = session.scene.as_ref().ok_or("Proof document is unavailable")?;
            self.pending = Some(ProofInput {
                resources: session
                    .resources
                    .as_ref()
                    .ok_or("Proof resources are unavailable")?
                    .clone(),
                document: scene.document.snapshot(),
                source: scene.source.clone(),
                view: session.view,
                front: session.front.take().ok_or("Proof surface is unavailable")?,
                samples: self.samples,
                paths: self.paths.take().ok_or("Proof output custody was lost")?,
                adapter: session.gpu_descriptor.clone(),
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
            Err(sugarsubstitute_execution::BlockingInputError::Admission(cause))
                if super::activity::retryable(&cause) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }
}
/// Native observations exercise exactly the same camera input path used by a visible window.
fn pan(session: &mut Session, start: [f64; 2], delta: [f64; 2]) -> Result<(), Box<dyn Error>> {
    session.input(Input::Pointer(start))?;
    session.input(Input::Pan(true))?;
    session.input(Input::Pointer([start[0] + delta[0], start[1] + delta[1]]))?;
    session.input(Input::Pan(false))
}
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
            Some(
                self.session
                    .next_deadline()
                    .map_or(self.cutoff, |navigation| navigation.min(self.cutoff)),
            )
        }
    }
    fn request_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(job) = &self.publishing {
            job.cancel();
        }
        if let Some(input) = self.pending.take() {
            self.session.retire(Retired {
                _front: Some(input.front),
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
