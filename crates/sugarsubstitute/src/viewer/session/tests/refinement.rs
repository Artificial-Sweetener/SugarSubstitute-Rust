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
    release: Arc<AtomicBool>,
    _directory: tempfile::TempDir,
}
fn preparing(filter: Filter) -> Result<Fixture, Box<dyn Error>> {
    let (mut session, clock, directory) = session()?;
    session.input(Input::Tool(ToolInput::OneToOne))?;
    clock.0.store(80_000_000, Ordering::SeqCst);
    session.tools.advance(Duration::from_millis(80))?;
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(session.clock.clone(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let spec = session.spec(
        Operation::FramePrepare,
        JobClass::CpuInteractive,
        65536,
        Duration::from_secs(30),
        Some(ReplacementKey { surface: SurfaceId::new(1)?, revision: Generation::new(1)? }),
    )?;
    let release = Arc::new(AtomicBool::new(false));
    let task_release = release.clone();
    let job = client.submit_cpu(&scope, spec, move |_| PreparationBoundary(task_release))?;
    session.runtime = Some(Runtime { owner, client, scope });
    session.rendering =
        Some(Rendering { job, filter, superseded: false, input_at: Duration::ZERO });
    session.presented = Some(Presented {
        view: session.tools.navigation().viewport(),
        filter: Filter::Nearest,
        document: (1, 1),
        source_revision: 0,
        damage: DamagePlan::full(session.tools.navigation().viewport().surface()),
        base: None,
        input_at: Duration::ZERO,
    });
    Ok(Fixture { session, wake, release, _directory: directory })
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
fn panning_a_preview_cancels_pending_finished_refinement_without_failing_the_viewer()
-> Result<(), Box<dyn Error>> {
    let Fixture { mut session, wake, _directory, .. } = preparing(Filter::Lanczos3)?;
    session.input(Input::Tool(ToolInput::Button { button: ToolButton::Primary, pressed: true }))?;
    session.input(Input::Tool(ToolInput::Pointer([7.0, 3.0])))?;
    assert_eq!(session.filter(), Filter::Nearest);
    observe_terminal(&mut session, &wake)?;
    assert!(session.failed.is_none());
    assert!(session.presented.is_some());
    Ok(())
}

#[test]
fn panning_keeps_nearest_work_alive_and_preserves_its_actual_failure() -> Result<(), Box<dyn Error>>
{
    let Fixture { mut session, wake, release, _directory } = preparing(Filter::Nearest)?;
    session.input(Input::Tool(ToolInput::Button { button: ToolButton::Primary, pressed: true }))?;
    session.input(Input::Tool(ToolInput::Pointer([7.0, 3.0])))?;
    release.store(true, Ordering::SeqCst);
    observe_terminal(&mut session, &wake)?;
    assert!(session.failed.is_some());
    Ok(())
}
