// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise navigation cancellation against genuine owned execution jobs and terminal delivery.
use super::*;
use std::sync::{Condvar, Mutex, atomic::AtomicBool};

#[derive(Default)]
pub(super) struct Wake {
    count: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *self.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
impl Wake {
    /// Observe a real terminal boundary without sleeps or depending on worker speed.
    pub(super) fn completion<T, E>(
        &self,
        client: &ExecutionClient,
        job: &mut OwnedJob<T, E>,
    ) -> Result<JobCompletion<T, E>, Box<dyn Error>> {
        loop {
            let observed = self.revision();
            client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
            if let Some(completion) = job.try_take_completion() {
                return Ok(completion);
            }
            self.wait_after(observed)?;
        }
    }
    /// Snapshot completion notification revision before inspecting the owned typed result.
    pub(super) fn revision(&self) -> u64 {
        *self.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    /// Bound a stalled external observer while remaining insensitive to ordinary scheduling order.
    pub(super) fn wait_after(&self, observed: u64) -> Result<(), Box<dyn Error>> {
        let count = self.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let (count, timeout) = self
            .changed
            .wait_timeout_while(count, Duration::from_secs(10), |count| *count == observed)
            .map_err(|cause| std::io::Error::other(cause.to_string()))?;
        if timeout.timed_out() && *count == observed {
            return Err("Preparation terminal delivery did not arrive".into());
        }
        Ok(())
    }
}
struct PreparationBoundary(Arc<AtomicBool>);
impl CpuTask<Packet, WorkError> for PreparationBoundary {
    fn step(&mut self, _: &JobContext) -> Result<CpuStep<Packet>, WorkError> {
        if self.0.load(Ordering::SeqCst) {
            Err(WorkError::invalid("Preparation boundary failure"))
        } else {
            Ok(CpuStep::Yield)
        }
    }
}
struct Fixture {
    session: Session,
    wake: Arc<Wake>,
    _directory: tempfile::TempDir,
}
fn preparing(reconstruction: Reconstruction) -> Result<Fixture, Box<dyn Error>> {
    let (mut session, clock, directory) = session()?;
    session.document.tools.set_content(Some(Extent::new(4096, 4096)?), None, Duration::ZERO)?;
    session.input(Input::Tool(ToolInput::OneToOne))?;
    clock.0.store(80_000_000, Ordering::SeqCst);
    session.document.tools.advance(Duration::from_millis(80))?;
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(session.clock.clone(), wake.clone())?;
    let client = owner.client();
    let runtime =
        Runtime::owned(owner, client.clone(), ScopeSpec { id: ScopeId::new(1)?, deadline: None })?;
    let setup_spec = session.spec(
        Operation::Infrastructure,
        JobClass::InfrastructureOrdinary,
        65536,
        Duration::from_secs(30),
        None,
    )?;
    let mut setup = client.submit_blocking(&runtime.scope, setup_spec, |context| {
        sugarsubstitute_canvas::Document::new(1, Extent::new(4096, 4096)?, context.memory())
    })?;
    let JobOutcome::Success(document) = wake.completion(&client, &mut setup)?.outcome else {
        return Err("Refinement document setup failed".into());
    };
    let identity = (document.snapshot().identity(), document.snapshot().revision());
    session.document.scene =
        Some(Scene { document, source: None, source_revision: 0, original: None });
    let spec = session.spec(
        Operation::FramePrepare,
        JobClass::CpuInteractive,
        65536,
        Duration::from_secs(30),
        Some(ReplacementKey { surface: SurfaceId::new(1)?, revision: Generation::new(1)? }),
    )?;
    let release = Arc::new(AtomicBool::new(false));
    let task_release = release.clone();
    let job =
        client.submit_cpu(&runtime.scope, spec, move |_| PreparationBoundary(task_release))?;
    session.runtime = Some(runtime);
    let coverage = coverage::detail(session.document.tools.navigation().viewport())?;
    session.rendering = Some(Rendering {
        job,
        reconstruction,
        tier: BackingTier::Detail,
        coverage,
        display_extent: session.document.tools.navigation().viewport().surface(),
        superseded: false,
    });
    session.document.presented = Some(Presented {
        tier: BackingTier::Detail,
        coverage,
        view: session.document.tools.navigation().viewport(),
        sampling_scale: session.document.tools.navigation().viewport().zoom(),
        reconstruction: Reconstruction::Preview,
        document: identity,
        source_revision: 0,
    });
    Ok(Fixture { session, wake, _directory: directory })
}
fn observe_terminal(session: &mut Session, wake: &Wake) -> Result<(), Box<dyn Error>> {
    while session.rendering.is_some() {
        let observed = wake.revision();
        session.consume_frame()?;
        let runtime = session.runtime.as_ref().ok_or("Missing execution")?;
        runtime.client.drain_completions(std::num::NonZeroUsize::MIN)?;
        session.consume_frame()?;
        if session.rendering.is_none() {
            break;
        }
        wake.wait_after(observed)?;
    }
    Ok(())
}

#[test]
fn reversing_zoom_retires_an_unreached_coarser_destination_and_keeps_finer_ready_pixels()
-> Result<(), Box<dyn Error>> {
    let Fixture { mut session, wake, _directory, .. } =
        preparing(Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3))?;
    session.input(Input::Tool(ToolInput::Wheel {
        steps: -1.0,
        phase: sugarsubstitute_canvas::ScrollPhase::Unphased,
    }))?;
    let future = session.document.tools.navigation().target_viewport();
    assert!(session.has_finer_ready_detail(future));
    session.rendering.as_mut().ok_or("Missing refinement")?.coverage = coverage::detail(future)?;
    session.input(Input::Tool(ToolInput::Wheel {
        steps: 1.0,
        phase: sugarsubstitute_canvas::ScrollPhase::Unphased,
    }))?;
    assert!(session.rendering.as_ref().is_some_and(|rendering| rendering.superseded));
    observe_terminal(&mut session, &wake)?;
    assert!(session.failed.is_none());
    assert_eq!(session.document.presented.ok_or("Lost finer ready pixels")?.sampling_scale, 1.0);
    Ok(())
}

#[test]
fn panning_a_preview_keeps_covered_finished_refinement_running() -> Result<(), Box<dyn Error>> {
    let Fixture { mut session, wake, _directory, .. } =
        preparing(Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3))?;
    session.input(Input::Tool(ToolInput::Button { button: ToolButton::Primary, pressed: true }))?;
    session.input(Input::Tool(ToolInput::Pointer([7.0, 3.0])))?;
    assert_eq!(
        session.reconstruction()?,
        Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3)
    );
    let rendering = session.rendering.as_mut().ok_or("Pan lost its covered refinement")?;
    assert!(!rendering.superseded);
    rendering.superseded = true;
    rendering.job.cancel();
    observe_terminal(&mut session, &wake)?;
    assert!(session.failed.is_none());
    assert!(session.document.presented.is_some());
    Ok(())
}

#[test]
fn overlapping_repair_survives_pan_beyond_the_complete_coverage_guard() -> Result<(), Box<dyn Error>>
{
    let Fixture { mut session, wake, _directory, .. } =
        preparing(Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3))?;
    session.input(Input::Tool(ToolInput::Button { button: ToolButton::Primary, pressed: true }))?;
    session.input(Input::Tool(ToolInput::Pointer([-129.0, 3.0])))?;
    let rendering = session.rendering.as_mut().ok_or("Overlapping pan lost useful refinement")?;
    assert!(!rendering.superseded);
    rendering.superseded = true;
    rendering.job.cancel();
    observe_terminal(&mut session, &wake)?;
    assert!(session.failed.is_none());
    Ok(())
}

#[test]
fn zero_overlap_pan_cancels_obsolete_refinement_without_failing_viewer()
-> Result<(), Box<dyn Error>> {
    let Fixture { mut session, wake, _directory, .. } =
        preparing(Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3))?;
    session.input(Input::Tool(ToolInput::Button { button: ToolButton::Primary, pressed: true }))?;
    session.input(Input::Tool(ToolInput::Pointer([-2000.0, 3.0])))?;
    assert!(session.rendering.as_ref().is_some_and(|rendering| rendering.superseded));
    observe_terminal(&mut session, &wake)?;
    assert!(session.failed.is_none());
    Ok(())
}

#[test]
fn zoom_keeps_overlapping_compatible_finished_refinement_running() -> Result<(), Box<dyn Error>> {
    let Fixture { mut session, wake, _directory, .. } =
        preparing(Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3))?;
    session.input(Input::Tool(ToolInput::Wheel {
        steps: 1.0,
        phase: sugarsubstitute_canvas::ScrollPhase::Unphased,
    }))?;
    assert_eq!(
        session.reconstruction()?,
        Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3)
    );
    let rendering = session.rendering.as_mut().ok_or("Zoom lost useful refinement")?;
    assert!(!rendering.superseded);
    rendering.superseded = true;
    rendering.job.cancel();
    observe_terminal(&mut session, &wake)?;
    assert!(session.failed.is_none());
    Ok(())
}

#[test]
fn completed_detail_is_current_for_partial_overlap_but_not_disjoint_motion()
-> Result<(), Box<dyn Error>> {
    let document = Extent::new(4096, 4096)?;
    let mut target = Viewport::new(Extent::new(320, 240)?)?;
    target.reframe([1000.0, 1000.0], 1.0)?;
    let coverage = coverage::detail(target)?;
    target.pan_by([200.0, 0.0])?;
    assert!(completion::completed_coverage_is_current(
        BackingTier::Detail,
        coverage,
        target,
        document,
    )?);
    target.pan_by([2000.0, 0.0])?;
    assert!(!completion::completed_coverage_is_current(
        BackingTier::Detail,
        coverage,
        target,
        document,
    )?);
    Ok(())
}

#[test]
fn canonical_navigation_reaches_publication_before_finished_settling_replaces_it()
-> Result<(), Box<dyn Error>> {
    let Fixture { mut session, wake, _directory, .. } =
        preparing(Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Bilinear))?;
    session.rendering.as_mut().ok_or("Missing navigation job")?.tier = BackingTier::Navigation;
    session.input(Input::Tool(ToolInput::Wheel {
        steps: -1.0,
        phase: sugarsubstitute_canvas::ScrollPhase::Unphased,
    }))?;
    assert!(!session.rendering.as_ref().ok_or("Navigation job was lost")?.superseded);
    session.document.tools.advance(Duration::from_secs(1))?;
    session.cancel_obsolete_refinement()?;
    assert!(!session.rendering.as_ref().ok_or("Navigation job was lost")?.superseded);
    assert_eq!(session.reconstruction()?.filter(), sugarsubstitute_image_ops::Filter::Lanczos3);
    let rendering = session.rendering.as_mut().ok_or("Missing navigation job")?;
    rendering.superseded = true;
    rendering.job.cancel();
    observe_terminal(&mut session, &wake)?;
    assert!(session.failed.is_none());
    Ok(())
}
