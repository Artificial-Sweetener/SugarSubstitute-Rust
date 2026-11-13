// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise a canvas scope alongside another owner of the same execution lifetime.

use super::{refinement::Wake, *};
use std::sync::{Condvar, Mutex};

mod retirement;

#[cfg(all(windows, feature = "native-composition"))]
#[test]
fn embedded_graphics_waits_for_native_custody_before_worker_retirement()
-> Result<(), Box<dyn Error>> {
    let (mut session, clock, _directory) = session()?;
    session.config.initial = InitialSource::AwaitHost;
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(clock, wake.clone())?;
    let client = owner.client();
    session.start_runtime(Runtime::shared(
        client.clone(),
        ScopeSpec { id: ScopeId::new(1)?, deadline: None },
    )?)?;
    while !session.initialized() {
        let revision = wake.revision();
        client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        session.poll(Duration::ZERO)?;
        if !session.initialized() {
            wake.wait_after(revision);
        }
    }
    assert!(session.host_graphics().is_none());
    session.request_host_graphics()?;
    let graphics = loop {
        let revision = wake.revision();
        client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        session.poll(Duration::ZERO)?;
        if let Some(graphics) = session.host_graphics() {
            break graphics.clone();
        }
        wake.wait_after(revision);
    };
    assert!(session.host_graphics_job.is_none());
    session.request_host_graphics()?;
    assert!(!session.host_graphics_pending);
    session.close_until(Moment::from_nanos(10_000_000_000))?;
    assert!(!session.settled()?);
    assert!(session.host_graphics().is_some());
    assert!(!session.shutdown_started);
    drop(graphics);
    loop {
        let revision = wake.revision();
        client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        if session.settled()? {
            break;
        }
        wake.wait_after(revision);
    }
    session.finish()?;
    wake.finish_shutdown(&mut owner, &client, Moment::from_nanos(10_000_000_000))?;
    Ok(())
}

/// Record the actual worker entry and explicit test release under one lock.
#[derive(Default)]
struct HeldState {
    entered: bool,
    released: bool,
}

/// Synchronize a finite blocking task without creating another scheduler.
#[derive(Default)]
struct HeldWork {
    state: Mutex<HeldState>,
    changed: Condvar,
}

impl HeldWork {
    /// Signal genuine entry and hold the execution worker until its release.
    fn run(&self) {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.entered = true;
        self.changed.notify_all();
        drop(crate::completion_wake::wait_while_watched(
            &self.changed,
            state,
            || "the test to release the held canvas work".to_owned(),
            |state| !state.released,
        ));
    }

    /// Wait for the actual worker entry, which the entering worker publishes under the same lock.
    fn wait_until_entered(&self) {
        let state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        drop(crate::completion_wake::wait_while_watched(
            &self.changed,
            state,
            || "held canvas work to enter".to_owned(),
            |state| !state.entered,
        ));
    }

    /// Allow the same already-started blocking task to return.
    fn release(&self) {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.released = true;
        self.changed.notify_all();
    }
}

/// Release the worker even when a test assertion or setup fails.
struct ReleaseHeld(Arc<HeldWork>);
impl ReleaseHeld {
    /// Permit the worker to return; repeated release is harmless.
    fn release(&self) {
        self.0.release();
    }
}
impl Drop for ReleaseHeld {
    fn drop(&mut self) {
        self.release();
    }
}

#[cfg(all(windows, feature = "native-composition"))]
#[test]
fn closing_waits_for_running_host_graphics_preparation() -> Result<(), Box<dyn Error>> {
    let (mut session, clock, _directory) = session()?;
    session.config.initial = InitialSource::AwaitHost;
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(clock, wake.clone())?;
    let client = owner.client();
    session.start_runtime(Runtime::shared(
        client.clone(),
        ScopeSpec { id: ScopeId::new(1)?, deadline: None },
    )?)?;
    while !session.initialized() {
        let revision = wake.revision();
        client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        session.poll(Duration::ZERO)?;
        if !session.initialized() {
            wake.wait_after(revision);
        }
    }

    let memory = session.resources.as_ref().ok_or("Canvas storage absent")?.memory.clone();
    let boundary = Arc::new(HeldWork::default());
    let release = ReleaseHeld(boundary.clone());
    let worker = boundary.clone();
    let spec = session.spec(
        Operation::Infrastructure,
        JobClass::InfrastructureOrdinary,
        64 * 1024,
        Duration::from_secs(30),
        None,
    )?;
    let runtime = session.runtime.as_ref().ok_or("Canvas execution absent")?;
    session.host_graphics_job =
        Some(client.submit_blocking(&runtime.scope, spec, move |context| {
            worker.run();
            sugarsubstitute_desktop::canvas::CanvasGraphics::fixture(&memory, &context)
                .map_err(WorkError::new)
        })?);
    boundary.wait_until_entered();
    session.close_until(Moment::from_nanos(10_000_000_000))?;
    assert!(
        session
            .host_graphics_job
            .as_ref()
            .ok_or("Graphics job absent")?
            .status()
            .cancellation_requested
    );
    assert!(!session.settled()?);
    release.release();
    loop {
        let revision = wake.revision();
        client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        if session.settled()? {
            break;
        }
        wake.wait_after(revision);
    }
    assert!(session.host_graphics_job.is_none());
    assert!(session.host_graphics().is_none());
    session.finish()?;
    wake.finish_shutdown(&mut owner, &client, Moment::from_nanos(10_000_000_000))?;
    Ok(())
}

#[test]
fn pacing_failure_still_fences_input_and_cancels_the_actual_worker() -> Result<(), Box<dyn Error>> {
    let (mut session, clock, _directory) = session()?;
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let runtime =
        Runtime::shared(client.clone(), ScopeSpec { id: ScopeId::new(1)?, deadline: None })?;
    let boundary = Arc::new(HeldWork::default());
    let release = ReleaseHeld(boundary.clone());
    let worker = boundary.clone();
    let spec = session.spec(
        Operation::Infrastructure,
        JobClass::InfrastructureOrdinary,
        1024,
        Duration::from_secs(30),
        None,
    )?;
    let job = client.submit_blocking(&runtime.scope, spec, move |_| {
        worker.run();
        Err::<(Resources, Scene), _>(WorkError::invalid("held initialization"))
    })?;
    session.runtime = Some(runtime);
    session.initializing = Some(job);
    session.initialization_pending = true;
    session.picker = true;
    boundary.wait_until_entered();
    session.pacing.set_frame_period(session.pacing.period(), Duration::from_secs(1))?;
    assert!(session.close_until(Moment::from_nanos(15_000_000_000)).is_err());
    assert!(session.closing);
    assert!(!session.initialization_pending);
    assert!(!session.picker);
    assert!(
        session
            .initializing
            .as_ref()
            .ok_or("Initialization job absent")?
            .status()
            .cancellation_requested
    );

    release.release();
    let result = wake
        .completion(&client, session.initializing.as_mut().ok_or("Initialization job absent")?)?;
    assert!(matches!(result.outcome, JobOutcome::Cancelled));
    drop(result);
    drop(session.initializing.take());
    clock.0.store(1_000_000_000, Ordering::SeqCst);
    session.close_until(Moment::from_nanos(15_000_000_000))?;
    loop {
        let revision = wake.revision();
        client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        if session.settled()? {
            break;
        }
        wake.wait_after(revision);
    }
    session.finish()?;
    wake.finish_shutdown(&mut owner, &client, Moment::from_nanos(15_000_000_000))?;
    Ok(())
}

#[test]
fn rejected_standalone_scope_keeps_its_executor_until_checked_shutdown()
-> Result<(), Box<dyn Error>> {
    let (mut session, clock, _directory) = session()?;
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(clock, wake.clone())?;
    let client = owner.client();
    let spec = ScopeSpec { id: ScopeId::new(1)?, deadline: None };
    let existing = client.create_scope(spec, None)?;
    session.starting_owner = Some(owner);
    assert!(session.start_owned(spec).is_err());
    assert!(session.starting_owner.is_some());
    assert!(!existing.snapshot().cancellation_requested);

    session.close_until(Moment::from_nanos(15_000_000_000))?;
    drop(existing);
    loop {
        let revision = wake.revision();
        client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        if session.settled()? {
            break;
        }
        wake.wait_after(revision);
    }
    session.finish()?;
    assert!(session.starting_owner.is_none());
    Ok(())
}

#[test]
fn shared_canvas_preserves_other_completion_and_waits_for_actual_scope_retirement()
-> Result<(), Box<dyn Error>> {
    let sequence = JobSequence::new();
    let (mut session, clock, _directory) = session_with_sequence(sequence.clone())?;
    session.config.initial = InitialSource::AwaitHost;
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let foreign = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    assert!(
        Runtime::shared(client.clone(), ScopeSpec { id: foreign.id(), deadline: None }).is_err()
    );
    assert!(!foreign.snapshot().cancellation_requested);
    let canvas = ScopeSpec { id: ScopeId::new(2)?, deadline: None };
    session.start_runtime(Runtime::shared(client.clone(), canvas)?)?;
    while session.resources.is_none() {
        let observed = wake.revision();
        client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        session.poll(Duration::ZERO)?;
        if session.resources.is_none() {
            wake.wait_after(observed);
        }
    }
    assert!(!session.take_picker_request());
    assert!(session.pending_open.is_none());
    assert!(session.document.scene.as_ref().is_some_and(|scene| scene.source.is_none()));
    assert!(session.initializing.is_none());
    // Initialization may already have admitted canvas preparation; its notices are legitimate.
    let startup = client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
    assert!(startup.notifications.iter().all(|notice| notice.scope_id == canvas.id));

    let foreign_spec = session.spec(
        Operation::Infrastructure,
        JobClass::InfrastructureOrdinary,
        1024,
        Duration::from_secs(30),
        None,
    )?;
    let mut foreign_job =
        client.submit_blocking(&foreign, foreign_spec, |_| Ok::<_, std::io::Error>(7))?;
    // Canvas preparation can share this wake. Keep the mailbox untouched while observing
    // the foreign terminal itself, so the following poll proves shared queue ownership.
    loop {
        let revision = wake.revision();
        if matches!(foreign_job.status().phase, JobPhase::Terminal(_)) {
            break;
        }
        wake.wait_after(revision);
    }
    // Taking the scope snapshot synchronizes with installation of the terminal notice.
    assert_eq!(foreign.snapshot().terminal_unobserved, 1);
    session.poll(Duration::ZERO)?;
    let batch = client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
    assert!(
        batch
            .notifications
            .iter()
            .all(|notice| { notice.scope_id == foreign.id() || notice.scope_id == canvas.id })
    );
    let foreign_notices: Vec<_> =
        batch.notifications.iter().filter(|notice| notice.scope_id == foreign.id()).collect();
    assert_eq!(foreign_notices.len(), 1);
    assert_eq!(foreign_notices[0].job_id, foreign_spec.id);
    let result = foreign_job.try_take_completion().ok_or("Foreign completion absent")?;
    assert!(matches!(result.outcome, JobOutcome::Success(7)));
    drop(result);
    drop(foreign_job);

    let retained_spec = session.spec(
        Operation::Infrastructure,
        JobClass::InfrastructureOrdinary,
        1024,
        Duration::from_secs(30),
        None,
    )?;
    let canvas_scope = &session.runtime.as_ref().ok_or("Canvas scope absent")?.scope;
    let mut retained_job =
        client.submit_blocking(canvas_scope, retained_spec, |_| Ok::<_, std::io::Error>(11))?;
    let retained = wake.completion(&client, &mut retained_job)?;
    assert!(matches!(retained.outcome, JobOutcome::Success(11)));
    drop(retained_job);

    let boundary = Arc::new(HeldWork::default());
    let release = ReleaseHeld(boundary.clone());
    let worker = boundary.clone();
    let spec = session.spec(
        Operation::Infrastructure,
        JobClass::InfrastructureOrdinary,
        1024,
        Duration::from_secs(30),
        None,
    )?;
    let canvas_scope = &session.runtime.as_ref().ok_or("Canvas scope absent")?.scope;
    let mut held = client.submit_blocking(canvas_scope, spec, move |_| {
        worker.run();
        Ok::<_, std::io::Error>(())
    })?;
    boundary.wait_until_entered();
    session.close_until(Moment::from_nanos(2_000_000_000))?;
    loop {
        let observed = wake.revision();
        client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        assert!(!session.settled()?);
        if session.shutdown_started {
            break;
        }
        wake.wait_after(observed);
    }
    assert!(!foreign.snapshot().cancellation_requested);
    clock.0.store(3_000_000_000, Ordering::SeqCst);
    assert!(session.settled().is_err());
    assert!(!session.settled()?);

    release.release();
    let result =
        wake.completion(&client, &mut held).map_err(|cause| format!("Held work: {cause}"))?;
    assert!(matches!(result.outcome, JobOutcome::Cancelled));
    drop(result);
    drop(held);
    client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
    let runtime = session.runtime.as_mut().ok_or("Canvas scope absent")?;
    let snapshot = runtime.scope.snapshot();
    assert!(snapshot.reserved_bytes >= 1024);
    assert_eq!(snapshot.queued, 0);
    assert_eq!(snapshot.running, 0);
    assert_eq!(snapshot.terminal_unobserved, 0);
    assert_eq!(snapshot.children_live, 0);
    assert_eq!(snapshot.native_reservations, 0);
    assert_eq!(snapshot.native_outputs_in_custody, 0);
    assert!(!runtime.settled()?);
    drop(retained);
    loop {
        let observed = wake.revision();
        client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        if session.settled()? {
            break;
        }
        wake.wait_after(observed);
    }
    session.finish()?;

    let later_spec = JobSpec { id: sequence.allocate()?, ..foreign_spec };
    let mut later = client.submit_blocking(&foreign, later_spec, |_| Ok::<_, std::io::Error>(9))?;
    let result =
        wake.completion(&client, &mut later).map_err(|cause| format!("Later work: {cause}"))?;
    assert!(matches!(result.outcome, JobOutcome::Success(9)));
    drop(result);
    drop(later);
    drop(foreign);
    wake.finish_shutdown(&mut owner, &client, Moment::from_nanos(5_000_000_000))?;
    Ok(())
}
