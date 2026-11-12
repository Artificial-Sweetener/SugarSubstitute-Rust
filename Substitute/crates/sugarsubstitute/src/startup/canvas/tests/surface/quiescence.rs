// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify the fixture waits themselves: a timer-held answer arrives on injected time, and an
//! answer nothing can deliver fails at once instead of waiting out the wall-clock backstop.

use super::*;
use boundary::{Execution, classify};
use std::sync::{Condvar, Mutex, PoisonError};
use sugarsubstitute_contracts::execution::{CompletionWake, ExecutionError};
use viewport::fixture::Fixture;

/// Delivery progress of [`HeldWake`] and the held job it observes.
#[derive(Default)]
struct HeldState {
    job_entered: bool,
    job_released: bool,
    armed: bool,
    wake_entered: bool,
    wake_released: bool,
    delivered: u64,
}

/// A completion wake that holds the first delivery after arming inside its callback.
#[derive(Default)]
struct HeldWake {
    state: Mutex<HeldState>,
    changed: Condvar,
}

impl HeldWake {
    fn update(&self, change: impl FnOnce(&mut HeldState)) {
        change(&mut self.state.lock().unwrap_or_else(PoisonError::into_inner));
        self.changed.notify_all();
    }

    fn wait(&self, done: impl Fn(&HeldState) -> bool) -> u64 {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let state = crate::completion_wake::wait_while_watched(
            &self.changed,
            state,
            || "the held job or wake to reach its boundary".to_owned(),
            |state| !done(state),
        );
        state.delivered
    }
}

/// Release the held job and wake before the owner joins its workers, including on assertion unwind.
struct ReleaseOnDrop(Arc<HeldWake>);

impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.0.update(|state| {
            state.job_released = true;
            state.wake_released = true;
        });
    }
}

impl CompletionWake for HeldWake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.armed {
            state.armed = false;
            state.wake_entered = true;
            self.changed.notify_all();
            state = crate::completion_wake::wait_while_watched(
                &self.changed,
                state,
                || "the test to release the held wake".to_owned(),
                |state| !state.wake_released,
            );
        }
        state.delivered += 1;
        drop(state);
        self.changed.notify_all();
        Ok(())
    }
}

/// Retiring an abandoned record removes it before its capacity wake is delivered. The boundary
/// must see that owed wake as running work; otherwise a quiescence wait fails as a deadlock in
/// the instant between removal and delivery.
#[test]
fn a_capacity_wake_still_being_delivered_is_running_work() -> Result<(), Box<dyn Error>> {
    let wake = Arc::new(HeldWake::default());
    let mut owner = sugarsubstitute_execution::ExecutionOwner::new(
        Arc::new(super::super::TestClock),
        wake.clone(),
    )?;
    let release = ReleaseOnDrop(wake.clone());
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let job_wake = wake.clone();
    let job = client.submit_blocking(
        &scope,
        super::super::spec(sugarsubstitute_contracts::execution::JobId::new(1)?, 4096),
        move |_| {
            job_wake.update(|state| state.job_entered = true);
            job_wake.wait(|state| state.job_released);
            Ok::<_, std::io::Error>(())
        },
    )?;
    wake.wait(|state| state.job_entered);
    drop(job);
    wake.update(|state| {
        state.armed = true;
        state.job_released = true;
    });
    // The worker that retired the abandoned record is now delivering its capacity wake.
    wake.wait(|state| state.wake_entered);
    let progress = owner.poll_shutdown()?;
    assert!(progress.unresolved_jobs.is_empty(), "{:?}", progress.unresolved_jobs);
    assert_eq!(classify(&owner)?, Execution::Running);
    drop(release);
    drop(scope);
    owner.request_shutdown(Moment::from_nanos(0));
    loop {
        let delivered = wake.wait(|_| true);
        if owner.poll_shutdown()?.settled {
            break;
        }
        wake.wait(|state| state.delivered > delivered);
    }
    owner.finish_shutdown()?;
    Ok(())
}

/// Require `result` to fail as a deadlock naming `awaiting`.
fn deadlocked<T>(result: Result<T, Box<dyn Error>>, awaiting: &str) -> Result<(), Box<dyn Error>> {
    let Err(cause) = result else {
        return Err(
            format!("A wait for {awaiting} returned although nothing could deliver it").into()
        );
    };
    let message = cause.to_string();
    assert!(message.starts_with(&format!("Deadlocked waiting for {awaiting}:")), "{message}");
    Ok(())
}

#[test]
fn frames_owed_behind_the_camera_animation_timer_arrive_without_the_test_advancing_time()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    let fitted = fixture.commit(4)?;
    fixture.viewport(5, json!({"type":"native"}))?;
    fixture.canvas.presented(fitted.frame)?;
    assert!(fixture.canvas.take_command().is_none());
    // The animation's first step is due only on its own timer, after every job has settled.
    let intermediate = fixture.pending()?;
    fixture.canvas.presented(intermediate)?;
    let reply = fixture.complete(5)?;
    assert_eq!(reply["viewport"]["mode"], "native");
    fixture.finish()
}

#[test]
fn a_frame_the_settled_canvas_never_prepares_fails_at_once_naming_it() -> Result<(), Box<dyn Error>>
{
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    fixture.settle()?;
    deadlocked(fixture.pending(), "a pending native frame")?;
    fixture.finish()
}

#[test]
fn a_reply_owed_only_to_native_acceptance_fails_at_once_naming_the_request()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.send(4, "surface.commit", json!({}))?;
    deadlocked(
        fixture.wait_reply(4, "applied"),
        "request 4 surface.commit to attachment:one at revision 4",
    )?;
    let target = fixture.pending()?;
    fixture.canvas.presented(target)?;
    fixture.take(4, "applied")?;
    fixture.finish()
}
