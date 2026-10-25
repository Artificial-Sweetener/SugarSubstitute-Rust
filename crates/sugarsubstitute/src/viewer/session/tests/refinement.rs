// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise navigation cancellation against genuine owned execution jobs and terminal delivery.
use super::*;
use std::sync::{Condvar, Mutex, atomic::AtomicBool};

#[derive(Default)]
struct Wake {
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
}
fn preparing(filter: Filter) -> Result<Fixture, Box<dyn Error>> {
    let (mut session, _) = session()?;
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
    session.rendering = Some(Rendering { job, filter, superseded: false });
    session.presented = Some(Presented {
        view: session.view,
        filter: Filter::Nearest,
        document: (1, 1),
        source_revision: 0,
        damage: DamagePlan::full(session.view.surface()),
        base: None,
    });
    Ok(Fixture { session, wake, release })
}
fn observe_terminal(session: &mut Session, wake: &Wake) -> Result<(), Box<dyn Error>> {
    while session.rendering.is_some() {
        let observed = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        session.consume_frame()?;
        let runtime = session.runtime.as_ref().ok_or("Missing execution")?;
        runtime.client.drain_completions(std::num::NonZeroUsize::MIN)?;
        session.consume_frame()?;
        if session.rendering.is_none() {
            break;
        }
        let count = wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let (_count, timeout) = wake
            .changed
            .wait_timeout_while(count, Duration::from_secs(10), |count| *count == observed)
            .map_err(|cause| std::io::Error::other(cause.to_string()))?;
        if timeout.timed_out() {
            return Err("Preparation terminal delivery did not arrive".into());
        }
    }
    Ok(())
}

#[test]
fn panning_a_preview_cancels_pending_finished_refinement_without_failing_the_viewer()
-> Result<(), Box<dyn Error>> {
    let Fixture { mut session, wake, .. } = preparing(Filter::Lanczos3)?;
    session.input(Input::Pan(true))?;
    session.input(Input::Pointer([7.0, 3.0]))?;
    assert_eq!(session.filter(), Filter::Nearest);
    observe_terminal(&mut session, &wake)?;
    assert!(session.failed.is_none());
    assert!(session.presented.is_some());
    Ok(())
}

#[test]
fn panning_keeps_nearest_work_alive_and_preserves_its_actual_failure() -> Result<(), Box<dyn Error>>
{
    let Fixture { mut session, wake, release } = preparing(Filter::Nearest)?;
    session.input(Input::Pan(true))?;
    session.input(Input::Pointer([7.0, 3.0]))?;
    release.store(true, Ordering::SeqCst);
    observe_terminal(&mut session, &wake)?;
    assert!(session.failed.is_some());
    Ok(())
}
