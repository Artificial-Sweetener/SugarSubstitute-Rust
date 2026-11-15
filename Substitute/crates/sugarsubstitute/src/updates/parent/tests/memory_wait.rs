// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Composing the helper waits for execution memory another startup job holds.
use super::*;
use std::num::NonZeroU64;

const RESERVATION: u64 = setup_storage::LAUNCH_RESERVATION_BYTES;

/// A check that starts while another startup job holds the memory its helper's composition
/// reserves is not a failed check: the composition keeps its source, and is admitted once that
/// job has finished.
#[test]
fn composing_the_helper_waits_for_memory_another_startup_job_holds() -> TestResult {
    let clock = Arc::new(Manual::default());
    let wake = Arc::new(Wake::default());
    // Room for one composition and not for two.
    let capacity = NonZeroU64::new(2 * RESERVATION - 1).ok_or("capacity")?;
    let mut owner = ExecutionOwner::with_memory_capacity(clock.clone(), wake.clone(), capacity)?;
    let parent = partial(owner.client()).map_err(|cause| cause as Box<dyn Error>)?;
    let jobs = JobSequence::new();
    let spec = |parent: &Parent| -> TestResult<JobSpec> {
        let deadline = parent.step_deadline().map_err(|cause| cause as Box<dyn Error>)?;
        parent
            .spec(&jobs, JobClass::InfrastructureLifecycle, RESERVATION, Some(deadline))
            .map_err(|cause| cause as Box<dyn Error>)
    };
    // The other startup job: its reservation stays held for as long as its memory lease lives.
    let mut holder = parent.client.submit_blocking(&parent.work, spec(&parent)?, |context| {
        Ok::<_, io::Error>(context.memory().clone())
    })?;
    let mut held = None;
    observe(&wake, || {
        parent.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        held = holder.try_take_completion().map(|completion| completion.outcome);
        Ok(held.is_some())
    })?;
    assert!(matches!(held, Some(JobOutcome::Success(_))), "the holder did not run");

    let mut source = Some(7_u32);
    let compose = |source: &mut Option<u32>, spec: JobSpec| {
        launch::compose(&parent.client, &parent.work, spec, source, |value: u32, _| {
            Ok::<u32, io::Error>(value)
        })
        .map_err(|cause| cause as Box<dyn Error>)
    };
    assert!(compose(&mut source, spec(&parent)?)?.is_none(), "admitted beside the holder");
    assert_eq!(source, Some(7), "a composition that waits keeps its source");

    drop(held);
    drop(holder);
    let mut composed =
        compose(&mut source, spec(&parent)?)?.ok_or("not admitted once the memory returned")?;
    assert_eq!(source, None, "the admitted composition took its source");
    let mut outcome = None;
    observe(&wake, || {
        parent.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        outcome = composed.try_take_completion().map(|completion| completion.outcome);
        Ok(outcome.is_some())
    })?;
    assert!(matches!(outcome, Some(JobOutcome::Success(7))));
    drop(composed);

    owner.request_shutdown(ShutdownBudget::new(clock.now())?.settle_until);
    observe(&wake, || {
        parent.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        Ok(owner.poll_shutdown()?.settled)
    })?;
    owner.finish_shutdown()?;
    Ok(())
}
