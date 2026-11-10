// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep input, source replacement and shutdown correct while an external native present is held.
use super::*;
use crate::native_canvas::session::presentation::Presenting;
use std::sync::{Condvar, Mutex};
use sugarsubstitute_image_gpu::WindowPresentation;

#[derive(Default)]
struct State {
    entered: bool,
    released: bool,
}

#[derive(Default)]
struct Gate {
    state: Mutex<State>,
    changed: Condvar,
}

impl Gate {
    fn wait_entered(&self) {
        let state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        drop(
            self.changed
                .wait_while(state, |state| !state.entered)
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
    }

    fn present(&self, target: FrameStamp) -> Result<GpuPresentation, WorkError> {
        let mut state = self
            .state
            .lock()
            .map_err(|cause| WorkError::new(std::io::Error::other(cause.to_string())))?;
        state.entered = true;
        self.changed.notify_all();
        drop(
            self.changed
                .wait_while(state, |state| !state.released)
                .map_err(|cause| WorkError::new(std::io::Error::other(cause.to_string())))?,
        );
        Ok(GpuPresentation { target, receipt: WindowPresentation { needs_follow_up: true } })
    }

    fn release(&self) {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).released = true;
        self.changed.notify_all();
    }
}

/// Release the fake external boundary before the real executor can join, including assertion unwind.
pub(super) struct Release(Arc<Gate>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.release();
    }
}

pub(super) fn presenting(
    with_storage: bool,
) -> Result<(Fixture, Release, FrameStamp), Box<dyn Error>> {
    let mut fixture = pending_draw()?;
    consume(&mut fixture.session, &fixture.wake)?;
    if with_storage {
        install_storage(&mut fixture)?;
    }
    let session = &mut fixture.session;
    let previous = session.document.display_evidence.ok_or("Missing actual display evidence")?;
    let target = FrameStamp { generation: Generation::new(3)?, ..previous.target };
    session.document.pending_gpu_evidence = Some(DisplayEvidence { target, ..previous });
    let now = session.pacing.next_deadline(Duration::ZERO)?.unwrap_or(Duration::ZERO);
    fixture.clock.0.store(u64::try_from(now.as_nanos())?, Ordering::SeqCst);
    session.pacing.submitted(target, PresentationMode::Projected, now, now)?;
    session.pacing.prepared(target, now)?;
    let spec = session.spec(
        Operation::Infrastructure,
        JobClass::InfrastructureOrdinary,
        65536,
        Duration::from_secs(30),
        None,
    )?;
    let gate = Arc::new(Gate::default());
    let release = Release(gate.clone());
    let boundary = gate.clone();
    let runtime = session.runtime.as_ref().ok_or("Fixture execution is unavailable")?;
    let job =
        runtime.client.submit_blocking(&runtime.scope, spec, move |_| boundary.present(target))?;
    session.presenting = Some(Presenting { target, job });
    gate.wait_entered();
    Ok((fixture, release, target))
}

pub(super) fn consume_present(session: &mut Session, wake: &Wake) -> Result<(), Box<dyn Error>> {
    while session.presenting.is_some() {
        let observed = wake.revision();
        session
            .runtime
            .as_ref()
            .ok_or("Missing execution")?
            .client
            .drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        session.consume_gpu_presentation()?;
        if session.presenting.is_some() {
            wake.wait_after(observed);
        }
    }
    Ok(())
}

#[test]
fn blocked_native_present_keeps_below_fit_input_responsive_until_a_real_receipt()
-> Result<(), Box<dyn Error>> {
    let (mut fixture, release, target) = presenting(false)?;
    let session = &mut fixture.session;
    let fit_zoom = session.document.tools.navigation().viewport().zoom();
    session.input(Input::Tool(ToolInput::Wheel { steps: -4.0, phase: ScrollPhase::Unphased }))?;
    fixture.clock.0.store(250_000_000, Ordering::SeqCst);
    session.poll(Duration::ZERO)?;
    assert!(session.document.tools.navigation().viewport().zoom() < fit_zoom);
    assert_eq!(session.feedback().cursor, sugarsubstitute_canvas::CursorIntent::Arrow);
    assert!(session.gpu_presentation_pending());
    assert!(session.take_gpu_presentation().is_none());
    assert_eq!(session.pacing.stats().projected_accepted, 0);

    release.0.release();
    consume_present(session, &fixture.wake)?;
    let accepted = session.take_gpu_presentation().ok_or("Native receipt was not delivered")?;
    assert_eq!(accepted.target, target);
    assert!(accepted.receipt.needs_follow_up);
    session.did_present_gpu(target)?;
    assert_eq!(session.pacing.stats().projected_accepted, 1);
    assert!(!session.gpu_presentation_pending());
    assert!(session.did_present_gpu(target).is_err());
    Ok(())
}

#[test]
fn close_retains_native_work_until_the_driver_boundary_actually_returns()
-> Result<(), Box<dyn Error>> {
    let (mut fixture, release, _) = presenting(false)?;
    fixture.session.close()?;
    assert!(!fixture.session.settled()?);
    assert!(fixture.session.presenting.is_some());
    assert!(!fixture.session.shutdown_started);
    release.0.release();
    consume_present(&mut fixture.session, &fixture.wake)?;
    assert!(fixture.session.gpu_presentation.is_none());
    assert!(fixture.session.document.pending_gpu_evidence.is_none());
    assert!(fixture.session.failed.is_none());
    Ok(())
}

#[test]
fn source_replacement_waits_for_native_return_and_rejects_the_old_receipt()
-> Result<(), Box<dyn Error>> {
    let (mut fixture, release, _) = presenting(true)?;
    let replacement = fixture._directory.path().join("replacement.png");
    fixture.session.open(replacement, CacheReuse::Compatible, None)?;
    fixture.session.poll(Duration::ZERO)?;
    assert!(fixture.session.pending_open.is_some());
    assert!(fixture.session.loading.is_none());
    assert!(fixture.session.presenting.is_some());
    release.0.release();
    consume_present(&mut fixture.session, &fixture.wake)?;
    assert!(fixture.session.gpu_presentation.is_none());
    assert!(fixture.session.document.pending_gpu_evidence.is_none());
    assert!(!fixture.session.gpu_presentation_pending());
    assert!(fixture.session.failed.is_none());
    Ok(())
}

#[cfg(feature = "native-composition")]
#[test]
fn cancelled_selection_retains_native_presentation_until_actual_return_and_rejects_its_receipt()
-> Result<(), Box<dyn Error>> {
    let (mut fixture, release, _) = presenting(true)?;
    fixture.session.cancel_selection()?;
    let presenting = fixture.session.presenting.as_ref().ok_or("Native custody was discarded")?;
    assert!(presenting.job.status().cancellation_requested);
    assert!(!matches!(presenting.job.status().phase, JobPhase::Terminal(_)));
    release.0.release();
    consume_present(&mut fixture.session, &fixture.wake)?;
    assert!(fixture.session.presenting.is_none());
    assert!(fixture.session.gpu_presentation.is_none());
    assert!(fixture.session.document.pending_gpu_evidence.is_none());
    assert!(!fixture.session.gpu_presentation_pending());
    assert!(fixture.session.failed.is_none());
    Ok(())
}

fn install_storage(fixture: &mut Fixture) -> Result<(), Box<dyn Error>> {
    let anchor = fixture._directory.path().join("cache");
    let display = fixture.session.config.maximum_extent;
    let presentation = budget::presentation_bytes(display, false);
    let spec = fixture.session.spec(
        Operation::Infrastructure,
        JobClass::InfrastructureOrdinary,
        budget::reservation_bytes(display, false, fixture.session.config.retained_document_bytes),
        Duration::from_secs(30),
        None,
    )?;
    let runtime = fixture.session.runtime.as_ref().ok_or("Missing execution")?;
    let mut setup = runtime.client.submit_blocking(&runtime.scope, spec, move |context| {
        work::initialize(anchor, false, false, presentation, &context)
            .map(|(resources, _)| resources)
    })?;
    let JobOutcome::Success(resources) =
        fixture.wake.completion(&runtime.client, &mut setup)?.outcome
    else {
        return Err("Presentation fixture storage initialization failed".into());
    };
    fixture.session.resources = Some(resources);
    fixture.session.gpu_ready = true;
    Ok(())
}

/// Hold an actual cooperative CPU slot at the external reconstruction boundary.
struct HeldCpu {
    gate: Arc<Gate>,
    target: FrameStamp,
}
impl CpuTask<(), WorkError> for HeldCpu {
    fn step(&mut self, _: &JobContext) -> Result<CpuStep<()>, WorkError> {
        self.gate.present(self.target)?;
        Ok(CpuStep::Complete(()))
    }
}

#[test]
fn native_display_prepares_while_reconstruction_retains_the_cpu_interactive_slot()
-> Result<(), Box<dyn Error>> {
    let mut fixture = pending_draw()?;
    consume(&mut fixture.session, &fixture.wake)?;
    let previous = fixture.session.document.display_evidence.ok_or("Missing display evidence")?;
    let target = FrameStamp { generation: Generation::new(3)?, ..previous.target };
    let cpu_spec = fixture.session.display_spec(target, false)?;
    let native_spec = fixture.session.display_spec(target, true)?;
    assert_eq!(native_spec.class, JobClass::InfrastructureOrdinary);
    assert_eq!(fixture.session.display_spec(target, false)?.class, JobClass::CpuInteractive);
    let gate = Arc::new(Gate::default());
    let release = Release(gate.clone());
    let held = gate.clone();
    let runtime = fixture.session.runtime.as_ref().ok_or("Missing execution")?;
    let mut reconstruction = runtime
        .client
        .submit_cpu(&runtime.scope, cpu_spec, move |_| HeldCpu { gate: held, target })?;
    gate.wait_entered();
    let mut input = Some(work::Draw {
        underlay: previous.underlay,
        tier: previous.tier,
        target,
        source: previous.source,
        view: previous.view,
        coverage: previous.coverage,
        projection: previous.projection,
        pixels: work::DrawPixels::Gpu(None),
        document: previous.document,
        source_revision: previous.source_revision,
        overlay_generation: 0,
        backdrop_generation: 0,
        backdrop_active: false,
        verified: true,
    });
    let job = runtime.client.submit_blocking_owned_input(
        &runtime.scope,
        native_spec,
        &mut input,
        |drawing, _| Ok::<_, WorkError>(drawing),
    )?;
    let now = fixture.session.pacing.next_deadline(Duration::ZERO)?.unwrap_or(Duration::ZERO);
    fixture.clock.0.store(u64::try_from(now.as_nanos())?, Ordering::SeqCst);
    fixture.session.pacing.submitted(target, PresentationMode::Projected, now, now)?;
    fixture.session.displaying = Some(super::super::super::display::Displaying {
        target,
        document: previous.document,
        source_revision: previous.source_revision,
        backdrop_generation: previous.backdrop_generation,
        job,
    });
    consume(&mut fixture.session, &fixture.wake)?;
    assert_eq!(
        fixture.session.document.display_evidence.ok_or("Missing prepared display")?.target,
        target
    );
    assert!(reconstruction.try_take_completion().is_none());
    drop(release);
    let runtime = fixture.session.runtime.as_ref().ok_or("Missing execution")?;
    assert!(matches!(
        fixture.wake.completion(&runtime.client, &mut reconstruction)?.outcome,
        JobOutcome::Success(())
    ));
    Ok(())
}
