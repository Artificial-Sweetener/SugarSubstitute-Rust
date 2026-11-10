// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A workspace open waits for its predecessor's workers and loads without a cutoff of its own.
use super::{closing_tests::Fixture, *};
use std::{num::NonZeroUsize, sync::Condvar};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// Drain execution completions and advance the workspace at `now` until `done` holds, waiting on
/// completion signals between attempts.
fn pump(fixture: &mut Fixture, now: Moment, done: impl Fn(&WorkspaceWork) -> bool) -> TestResult {
    loop {
        let revision = *fixture.wake.revision.lock().map_err(|_| "Wake lock")?;
        let work = fixture.host.workspace.as_mut().ok_or("Workspace")?;
        work.client
            .as_ref()
            .ok_or("Client")?
            .drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        work.submit_publication(now)?;
        work.advance(now)?;
        if done(work) {
            return Ok(());
        }
        let lock = fixture.wake.revision.lock().map_err(|_| "Wake lock")?;
        let (lock, timeout) = fixture
            .wake
            .changed
            .wait_timeout_while(lock, Duration::from_secs(10), |value| *value == revision)
            .map_err(|_| "Wake wait")?;
        if timeout.timed_out() && *lock == revision {
            return Err("Workspace work made no progress".into());
        }
    }
}

#[test]
fn a_workspace_open_waits_for_a_slow_predecessor_and_loads_after_any_elapsed_time() -> TestResult {
    let mut fixture = Fixture::new(false)?;
    // Two hours past the moment the open was requested: far beyond any bound it once had.
    let later = fixture.host.clock.now().checked_add(Duration::from_secs(7_200))?;
    let launch = fixture.host.launch_id();
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let work = fixture.host.workspace.as_mut().ok_or("Workspace")?;
    let context = work.context.clone().ok_or("Context")?;
    let effect = StartupEffect::LoadFrontend {
        launch,
        navigation: Generation::new(1)?,
        origin: context.origin,
    };
    // The previous workspace's transfer retirement is still running when the open is requested.
    let held = gate.clone();
    let spec = work.job(sugarsubstitute_comfy::output_media::OUTPUT_TRANSFER_BYTES, None)?;
    let predecessor = work.client.as_ref().ok_or("Client")?.submit_blocking(
        work.scope.as_ref().ok_or("Scope")?,
        spec,
        move |_| {
            let (released, changed) = &*held;
            let mut released = released.lock().map_err(|_| StoreError::Changed)?;
            while !*released {
                released = changed.wait(released).map_err(|_| StoreError::Changed)?;
            }
            Ok::<(), StoreError>(())
        },
    )?;
    work.retirement_job = Some(predecessor);
    work.begin(context, effect)?;

    work.advance(later)?;
    assert!(work.pending_open.is_some(), "the open waits for its predecessor");
    assert!(work.load.is_none());

    *gate.0.lock().map_err(|_| "Gate lock")? = true;
    gate.1.notify_all();
    pump(&mut fixture, later, |work| work.ready)?;
    let work = fixture.host.workspace.as_ref().ok_or("Workspace")?;
    assert!(work.pending_open.is_none() && work.load.is_none());
    assert!(work.driver.is_some(), "the workspace loaded without an error");
    Ok(())
}
