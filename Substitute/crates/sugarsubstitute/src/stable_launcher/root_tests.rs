// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The stable entry settles on its job's outcome and exits even if that job never returns.
use super::*;
use crate::stable_launcher::test_support::{Hold, Released};
use crate::updates::test_support::{Manual, Wake, observe};
use std::sync::atomic::Ordering;

/// Seconds on the manual clock.
fn seconds(value: u64) -> u64 {
    value * 1_000_000_000
}

/// A root, its manual clock and wake, and the hold on its one job.
type Stuck = (Root, Arc<Manual>, Arc<Wake>, Released);

/// A root on a manual clock whose one job blocks until the test releases it, ignoring
/// cancellation, like a native call that never returns.
fn stuck() -> Result<Stuck> {
    let clock = Arc::new(Manual::default());
    let wake = Arc::new(Wake::default());
    let mut root = Root::new(PathBuf::from("unopened-installation"));
    root.clock = clock.clone();
    let hold = Hold::new();
    let held = hold.clone();
    root.admit(wake.clone(), move |_| {
        held.block();
        Ok(())
    })?;
    hold.entered();
    Ok((root, clock, wake, Released(hold)))
}

#[test]
fn a_root_job_that_returns_settles_without_the_exit_grace() -> Result<()> {
    let clock = Arc::new(Manual::default());
    let wake = Arc::new(Wake::default());
    let mut root = Root::new(PathBuf::from("unopened-installation"));
    root.clock = clock;
    root.admit(wake.clone(), |_| Ok(()))?;
    assert!(root.next_deadline().is_some(), "the exit grace is armed while the job runs");
    observe(&wake, || Ok(root.advance(NonZeroUsize::MIN)? == HeadlessStep::Shutdown))?;
    assert!(!root.abandoned);
    observe(&wake, || root.poll_shutdown())?;
    assert!(!root.abandoned, "a settled root is never abandoned");
    root.finish_shutdown()?;
    assert!(root.owner.is_none());
    Ok(())
}

#[test]
fn a_root_job_that_never_returns_is_abandoned_after_its_grace() -> Result<()> {
    let (mut root, clock, _wake, hold) = stuck()?;
    let wake_at = root.exit_at.and_then(|(_, wake)| wake);
    assert!(wake_at.is_some());
    clock.0.store(seconds(40), Ordering::SeqCst);
    assert_eq!(root.advance(NonZeroUsize::MIN)?, HeadlessStep::Active, "the job's own deadline");
    clock.0.store(seconds(45) - 1, Ordering::SeqCst);
    assert_eq!(root.advance(NonZeroUsize::MIN)?, HeadlessStep::Active);
    assert_eq!(root.next_deadline(), wake_at, "only the unchanged exit moment remains");
    clock.0.store(seconds(45), Ordering::SeqCst);
    assert_eq!(root.advance(NonZeroUsize::MIN)?, HeadlessStep::Shutdown);
    assert!(root.abandoned);
    assert!(root.next_deadline().is_none());
    assert!(root.poll_shutdown()?, "the pump can exit");
    assert!(root.finish_shutdown().is_err(), "the unfinished start is still reported");
    assert!(root.owner.is_none());
    drop(hold);
    Ok(())
}

#[test]
fn closing_a_root_whose_job_never_returns_still_exits_at_its_grace() -> Result<()> {
    let (mut root, clock, _wake, hold) = stuck()?;
    root.request_shutdown()?;
    assert!(!root.poll_shutdown()?, "cancellation is not proof that work stopped");
    clock.0.store(seconds(45), Ordering::SeqCst);
    assert!(root.poll_shutdown()?);
    assert!(root.abandoned);
    assert!(root.finish_shutdown().is_err());
    drop(hold);
    Ok(())
}

#[test]
fn a_slow_engine_takeover_shows_the_notice_until_the_root_is_done() -> Result<()> {
    let (mut root, clock, wake, hold) = stuck()?;
    clock.0.store(seconds(2) - 1, Ordering::SeqCst);
    assert!(matches!(
        root.prepare_native_runtime(NonZeroUsize::MIN)?,
        StartupPreparationStep::Waiting
    ));
    assert!(root.notice().is_none());
    clock.0.store(seconds(2), Ordering::SeqCst);
    assert!(matches!(
        root.prepare_native_runtime(NonZeroUsize::MIN)?,
        StartupPreparationStep::Ready
    ));
    assert!(root.notice().is_some());
    drop(hold);
    observe(&wake, || Ok(root.advance(NonZeroUsize::MIN)? == HeadlessStep::Shutdown))?;
    root.advance(NonZeroUsize::MIN)?;
    assert!(root.notice().is_none(), "the engine took over");
    observe(&wake, || root.poll_shutdown())?;
    root.finish_shutdown()
}
