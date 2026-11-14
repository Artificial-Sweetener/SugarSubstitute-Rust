// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A check ends at its limit as a failed check; a download and a preparation have no limit.
use super::*;
use sugarsubstitute_contracts::update::attempt::{AttemptOutcome, AttemptPhase};

fn advance(parent: &mut Parent, jobs: &JobSequence) -> TestResult {
    let batch = parent.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
    parent.advance(&batch, jobs).map_err(|cause| cause as Box<dyn Error>)
}

#[test]
fn only_the_check_and_leaving_after_the_result_are_limited() {
    use limits::Wait;
    for checking in [None, Some(AttemptPhase::Opening), Some(AttemptPhase::Checking)] {
        assert_eq!(Wait::of(checking, false), Some(Wait::Check), "{checking:?}");
    }
    for working in [
        AttemptPhase::CheckingUpdater,
        AttemptPhase::Downloading,
        AttemptPhase::Preparing,
        AttemptPhase::PreparingUpdater,
        AttemptPhase::Applying,
    ] {
        assert_eq!(Wait::of(Some(working), false), None, "{working:?}");
        assert_eq!(Wait::of(Some(working), true), Some(Wait::Leave), "{working:?}");
    }
    assert_eq!(Wait::of(None, true), Some(Wait::Leave));
    assert!(Wait::Leave.limit() < Wait::Check.limit());
}

#[test]
fn a_check_that_reaches_its_limit_closes_the_helper_and_is_a_failed_check() -> TestResult {
    let clock = Arc::new(Manual::default());
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let mut parent = partial(owner.client()).map_err(|cause| cause as Box<dyn Error>)?;
    let jobs = JobSequence::new();
    advance(&mut parent, &jobs)?;
    let limit = u64::try_from(limits::CHECK.as_nanos())?;
    clock.0.store(limit - 1, Ordering::SeqCst);
    advance(&mut parent, &jobs)?;
    assert!(parent.shutdown.is_none(), "the check was ended before its limit");
    assert!(parent.failure.is_none());
    clock.0.store(limit, Ordering::SeqCst);
    advance(&mut parent, &jobs)?;
    assert!(parent.shutdown.is_some(), "a check at its limit left the helper open");
    assert_eq!(
        parent
            .failure
            .as_ref()
            .and_then(|cause| cause.downcast_ref::<io::Error>())
            .map(io::Error::kind),
        Some(io::ErrorKind::TimedOut)
    );
    finish(&mut parent, &mut owner, &wake, &jobs)?;
    let (check, attempt) = parent.take_check_outcome().ok_or("Settled check absent")?;
    assert_eq!(check, crate::updates::schedule::CheckOutcome::Failed);
    let AttemptOutcome::Failed(report) = attempt else {
        return Err("A check that reached its limit was not a failure".into());
    };
    assert_eq!(report.phase, AttemptPhase::Opening);
    assert_eq!(report.causes, ["Update preparation did not start checking within 120 seconds"]);
    Ok(())
}

#[test]
fn a_closing_helper_is_not_failed_by_the_check_limit() -> TestResult {
    let clock = Arc::new(Manual::default());
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let mut parent = partial(owner.client()).map_err(|cause| cause as Box<dyn Error>)?;
    let jobs = JobSequence::new();
    advance(&mut parent, &jobs)?;
    parent.close(None).map_err(|cause| cause as Box<dyn Error>)?;
    // Within the cleanup budget and past nothing: the limit belongs to an open helper only.
    clock.0.store(1_000_000_000, Ordering::SeqCst);
    let original = parent.limit;
    advance(&mut parent, &jobs)?;
    assert_eq!(parent.limit, original);
    assert!(parent.failure.is_none());
    finish(&mut parent, &mut owner, &wake, &jobs)
}
