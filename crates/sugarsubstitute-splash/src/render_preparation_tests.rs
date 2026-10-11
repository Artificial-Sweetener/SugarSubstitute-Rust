// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Verify real execution admission, retained memory and synchronous/resumable pixel identity.

use super::*;
#[path = "render_test_support.rs"]
mod support;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::execution::{ExecutionPort, Generation, SurfaceId};
use support::{Rig, TestResult, spec, success};

struct Counted {
    task: SplashRenderTask,
    steps: Arc<AtomicUsize>,
}
impl CpuTask<SplashPreparationResult, PreparationError> for Counted {
    fn step(
        &mut self,
        context: &JobContext,
    ) -> Result<CpuStep<SplashPreparationResult>, PreparationError> {
        self.steps.fetch_add(1, Ordering::Relaxed);
        self.task.step(context)
    }
}
fn request(scale: f64, expanded: bool) -> TestResult<SplashRenderRequest> {
    Ok(SplashRenderRequest {
        shape: FrameShape::new(
            (558.0 * scale).round() as u32,
            (558.0 * scale).round() as u32,
            scale,
        )?,
        identity: FrameIdentity {
            surface: SurfaceId::new(1)?,
            state_generation: Generation::new(1)?,
            sequence: Generation::new(1)?,
        },
        appearance: Appearance { expanded, ..Appearance::demo() },
        character: AnimationFrame::bootstrap(),
        feedback: ProgressFrame { completed: 0.4, activity: Some(0.5) },
        bootstrap: false,
    })
}
fn initialize(rig: &Rig) -> TestResult<SplashRenderer> {
    let mut job =
        rig.client.submit_cpu(&rig.scope, spec(1, SPLASH_CACHE_RESERVATION_BYTES)?, |context| {
            SplashRenderer::initialize("en".into(), context.memory().clone())
        })?;
    let renderer = success(rig.completion(&mut job)?)??;
    assert_eq!(renderer.characters.decoded_count(), 0);
    assert!(renderer.text.is_none());
    assert_eq!(rig.scope.snapshot().reserved_bytes, SPLASH_CACHE_RESERVATION_BYTES);
    Ok(renderer)
}
/// Cold work yields through PNG rows; warm work retains the exact cache-owning renderer.
#[test]
fn admitted_cold_and_warm_frames_preserve_pixels_cache_and_memory() -> TestResult {
    let rig = Rig::new()?;
    let mut renderer = initialize(&rig)?;
    let mut offline = SplashRenderer::new("en")?;
    let mut previous_steps = 0;
    for id in 2..=3 {
        let request = request(1.25, true)?;
        let steps = Arc::new(AtomicUsize::new(0));
        let count = steps.clone();
        let mut job = rig.client.submit_cpu(
            &rig.scope,
            spec(id, request.reservation_bytes())?,
            move |context| Counted {
                task: renderer.prepare(request, context.memory().clone()),
                steps: count,
            },
        )?;
        let result = success(rig.completion(&mut job)?)?;
        let frame = result.frame?;
        renderer = result.renderer;
        let expected = offline.render(
            request.shape.width(),
            request.shape.height(),
            request.shape.scale(),
            request.appearance,
            request.character,
            request.feedback,
        )?;
        assert_eq!(frame.pixels(), expected.data());
        assert_eq!(frame.identity(), request.identity);
        assert_eq!(renderer.characters.decoded_count(), 1);
        let count = steps.load(Ordering::Relaxed);
        if id == 2 {
            assert!(count > 450);
            previous_steps = count;
        } else {
            assert!(count + 400 < previous_steps);
        }
        assert_eq!(
            rig.scope.snapshot().reserved_bytes,
            SPLASH_CACHE_RESERVATION_BYTES + request.reservation_bytes()
        );
        let clone = frame.clone();
        drop(frame);
        assert_eq!(
            rig.scope.snapshot().reserved_bytes,
            SPLASH_CACHE_RESERVATION_BYTES + request.reservation_bytes()
        );
        drop(clone);
        assert!(rig.scope.snapshot().reserved_bytes >= SPLASH_CACHE_RESERVATION_BYTES);
    }
    drop(renderer);
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
/// Bad input and insufficient job storage are ordinary frame errors that recover the owner.
#[test]
fn rejected_render_inputs_preserve_renderer_and_release_frame_reservation() -> TestResult {
    let rig = Rig::new()?;
    let mut renderer = initialize(&rig)?;
    for (id, scale, bytes) in [(2, 0.25, 1024), (3, 1.0, 1)] {
        let request = request(scale, false)?;
        let mut job = rig.client.submit_cpu(&rig.scope, spec(id, bytes)?, move |context| {
            renderer.prepare(request, context.memory().clone())
        })?;
        let result = success(rig.completion(&mut job)?)?;
        assert!(matches!(
            result.frame,
            Err(RenderError::InvalidFrame) | Err(RenderError::Memory(_))
        ));
        renderer = result.renderer;
        assert_eq!(renderer.characters.decoded_count(), 0);
        assert!(rig.scope.snapshot().reserved_bytes >= SPLASH_CACHE_RESERVATION_BYTES);
    }
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, SPLASH_CACHE_RESERVATION_BYTES);
    drop(renderer);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
/// Validation and sampling remain identical at a fractional collapsed flip edge.
#[test]
fn admitted_fractional_flip_uses_the_same_sync_raster_stages() -> TestResult {
    let rig = Rig::new()?;
    let renderer = initialize(&rig)?;
    let mut request = request(1.75, false)?;
    request.character.progress = 0.94;
    request.character.pose = 39;
    let mut offline = SplashRenderer::new("en")?;
    let expected = offline.render(
        request.shape.width(),
        request.shape.height(),
        request.shape.scale(),
        request.appearance,
        request.character,
        request.feedback,
    )?;
    let mut job = rig.client.submit_cpu(
        &rig.scope,
        spec(2, request.reservation_bytes())?,
        move |context| renderer.prepare(request, context.memory().clone()),
    )?;
    let result = success(rig.completion(&mut job)?)?;
    assert_eq!(result.frame?.pixels(), expected.data());
    drop(result.renderer);
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

struct Paused {
    task: SplashRenderTask,
    entered: Arc<support::Gate>,
    release: Arc<support::Gate>,
    paused: bool,
}
impl CpuTask<SplashPreparationResult, PreparationError> for Paused {
    fn step(
        &mut self,
        context: &JobContext,
    ) -> Result<CpuStep<SplashPreparationResult>, PreparationError> {
        let result = self.task.step(context)?;
        if !self.paused {
            self.paused = true;
            self.entered.open().map_err(|_| PreparationError::InvalidTaskState)?;
            self.release.wait().map_err(|_| PreparationError::InvalidTaskState)?;
        }
        Ok(result)
    }
}
/// Cancellation cannot publish or release live renderer/frame storage until a step returns.
#[test]
fn cancellation_disposes_admitted_renderer_only_after_actual_step_return() -> TestResult {
    let rig = Rig::new()?;
    let renderer = initialize(&rig)?;
    let request = request(1.0, true)?;
    let entered = Arc::new(support::Gate::default());
    let release = Arc::new(support::Gate::default());
    let worker_entered = entered.clone();
    let worker_release = release.clone();
    let mut job = rig.client.submit_cpu(
        &rig.scope,
        spec(2, request.reservation_bytes())?,
        move |context| Paused {
            task: renderer.prepare(request, context.memory().clone()),
            entered: worker_entered,
            release: worker_release,
            paused: false,
        },
    )?;
    entered.wait()?;
    job.cancel();
    assert!(job.try_take_completion().is_none());
    assert_eq!(
        rig.scope.snapshot().reserved_bytes,
        SPLASH_CACHE_RESERVATION_BYTES + request.reservation_bytes()
    );
    release.open()?;
    assert!(matches!(
        rig.completion(&mut job)?.outcome,
        sugarsubstitute_contracts::execution::JobOutcome::Cancelled
    ));
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
struct RepeatChecked {
    task: SplashRenderTask,
    checked: Arc<std::sync::atomic::AtomicBool>,
}
impl CpuTask<SplashPreparationResult, PreparationError> for RepeatChecked {
    fn step(
        &mut self,
        context: &JobContext,
    ) -> Result<CpuStep<SplashPreparationResult>, PreparationError> {
        let result = self.task.step(context)?;
        if matches!(&result, CpuStep::Complete(_)) {
            self.checked.store(
                matches!(self.task.step(context), Err(PreparationError::InvalidTaskState)),
                Ordering::Relaxed,
            );
        }
        Ok(result)
    }
}
/// Completed task reuse fails explicitly while its ordinary render error recovers the cache.
#[test]
fn completed_tasks_reject_a_second_terminal_step() -> TestResult {
    let rig = Rig::new()?;
    let renderer = initialize(&rig)?;
    let request = request(0.25, false)?;
    let checked = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let worker_checked = checked.clone();
    let mut job =
        rig.client.submit_cpu(&rig.scope, spec(2, 1024)?, move |context| RepeatChecked {
            task: renderer.prepare(request, context.memory().clone()),
            checked: worker_checked,
        })?;
    let result = success(rig.completion(&mut job)?)?;
    assert!(checked.load(Ordering::Relaxed));
    assert!(matches!(result.frame, Err(RenderError::InvalidFrame)));
    drop(result.renderer);
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

/// Initialization and offline owners cannot bypass persistent cache admission.
#[test]
fn cache_initialization_enforces_its_owned_reservation() -> TestResult {
    let rig = Rig::new()?;
    let mut initialization = rig.client.submit_cpu(&rig.scope, spec(1, 1)?, |context| {
        SplashRenderer::initialize("en".into(), context.memory().clone())
    })?;
    assert!(matches!(success(rig.completion(&mut initialization)?)?, Err(RenderError::Memory(_))));
    let renderer = SplashRenderer::new("en")?;
    let request = request(1.0, true)?;
    let mut job = rig.client.submit_cpu(
        &rig.scope,
        spec(2, request.reservation_bytes())?,
        move |context| renderer.prepare(request, context.memory().clone()),
    )?;
    let result = success(rig.completion(&mut job)?)?;
    assert!(matches!(result.frame, Err(RenderError::UnadmittedCache)));
    assert_eq!(result.renderer.characters.decoded_count(), 0);
    drop(result.renderer);
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

/// Output validation keeps the existing pixel claim until the immutable frame claims its Vec.
#[test]
fn validation_handoff_cannot_publish_an_undercharged_completed_raster() -> TestResult {
    let rig = Rig::new()?;
    let renderer = initialize(&rig)?;
    let mut request = request(1.0, false)?;
    request.shape = FrameShape::new(1320, 1320, 1.0)?;
    request.bootstrap = true;
    let only_raster_peak = request.shape.byte_len() as u64 + request.scratch_bytes();
    assert!(only_raster_peak < request.reservation_bytes());
    let mut job =
        rig.client.submit_cpu(&rig.scope, spec(2, only_raster_peak)?, move |context| {
            renderer.prepare(request, context.memory().clone())
        })?;
    let result = success(rig.completion(&mut job)?)?;
    assert!(matches!(
        result.frame,
        Err(RenderError::Frame(
            sugarsubstitute_contracts::presentation::FrameError::AllocationBudget(_)
        ))
    ));
    assert_eq!(result.renderer.characters.decoded_count(), 1);
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, SPLASH_CACHE_RESERVATION_BYTES);
    drop(result.renderer);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
