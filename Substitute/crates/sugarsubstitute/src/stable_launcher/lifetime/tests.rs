// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real executor admission and inert creator custody exercise launcher cleanup without starting an EXE.
use super::*;
use crate::updates::test_support::{Manual, Wake, observe};
use std::sync::atomic::Ordering;
use sugarsubstitute_supervision::WindowsLaunchCommand;

type Fixture = (Launcher, Arc<Manual>, Arc<Wake>);

/// Start execution only; keep installation, native creation and selected-generation work unopened.
pub(super) fn fixture() -> Result<Fixture> {
    let clock = Arc::new(Manual::default());
    let wake = Arc::new(Wake::default());
    let mut launcher = Launcher::new(PathBuf::from("unopened-installation"));
    launcher.clock = clock.clone();
    let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    launcher.work_scope =
        Some(client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?);
    launcher.cleanup_scope =
        Some(client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?);
    launcher.client = Some(client);
    launcher.owner = Some(owner);
    launcher.until = Some(EntrySetupBudget::new(clock.now())?.until());
    let executable = std::env::current_exe()?;
    launcher.launch = Some(ApplicationLaunch::new(WindowsLaunchCommand::from_os(
        &executable,
        executable.parent().ok_or("Test executable has no parent")?,
        &[],
    )?));
    Ok((launcher, clock, wake))
}

/// Observe retirement before joining the original workers.
pub(super) fn finish(launcher: &mut Launcher, wake: &Wake) -> Result<()> {
    observe(wake, || launcher.poll_shutdown())?;
    assert!(launcher.launch.is_none());
    launcher.finish_shutdown()?;
    assert!(launcher.owner.is_none());
    assert!(launcher.finished);
    assert!(launcher.finish_shutdown().is_err());
    Ok(())
}

#[test]
fn repeated_close_and_cleanup_submission_keep_the_original_cutoff() -> Result<()> {
    let (mut launcher, clock, wake) = fixture()?;
    launcher.request_shutdown()?;
    let cutoff = launcher.cleanup_until;
    let wake_at = launcher.next_deadline();
    clock.0.store(1_000_000_000, Ordering::SeqCst);
    launcher.request_shutdown()?;
    assert_eq!(launcher.cleanup_until, cutoff);
    assert_eq!(launcher.next_deadline(), wake_at);
    assert!(!launcher.poll_shutdown()?);
    assert_eq!(launcher.job.as_ref().ok_or("Cleanup job absent")?.job.spec().deadline, cutoff);
    finish(&mut launcher, &wake)?;
    assert_eq!(launcher.cleanup_until, cutoff);
    Ok(())
}

#[test]
fn rejected_cleanup_submission_retains_creator_and_retries_in_the_original_scope() -> Result<()> {
    let (mut launcher, _, wake) = fixture()?;
    launcher.request_shutdown()?;
    let actual = launcher.cleanup_scope.take();
    launcher.cleanup_scope = Some(
        launcher
            .client
            .as_ref()
            .ok_or("Client absent")?
            .create_native_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?,
    );
    assert!(launcher.poll_shutdown().is_err());
    assert!(launcher.launch.is_some());
    assert!(launcher.job.is_none());
    assert!(!launcher.stopping);
    launcher.cleanup_scope = actual;
    finish(&mut launcher, &wake)
}

#[test]
fn expired_cleanup_joins_workers_but_never_claims_creator_retirement() -> Result<()> {
    let (mut launcher, clock, wake) = fixture()?;
    launcher.request_shutdown()?;
    let cutoff = launcher.cleanup_until;
    clock.0.store(16_000_000_000, Ordering::SeqCst);
    assert!(launcher.poll_shutdown().is_err());
    assert!(launcher.cutoff_reported);
    assert!(launcher.launch.is_some());
    assert!(launcher.job.is_none());
    assert!(launcher.next_deadline().is_none());
    observe(&wake, || launcher.poll_shutdown())?;
    assert!(launcher.finish_shutdown().is_err());
    assert!(launcher.launch.is_some());
    assert!(!launcher.finished);
    assert_eq!(launcher.cleanup_until, cutoff);
    assert_eq!(launcher.jobs.allocate()?.get(), 1);
    Ok(())
}

#[test]
fn actual_join_observed_late_is_not_reported_as_a_launcher_timeout() -> Result<()> {
    let (mut launcher, clock, wake) = fixture()?;
    launcher.request_shutdown()?;
    observe(&wake, || launcher.poll_shutdown())?;
    assert!(launcher.launch.is_none());
    clock.0.store(20_000_000_000, Ordering::SeqCst);
    assert!(launcher.poll_shutdown()?);
    assert!(!launcher.cutoff_reported);
    launcher.finish_shutdown()?;
    Ok(())
}
