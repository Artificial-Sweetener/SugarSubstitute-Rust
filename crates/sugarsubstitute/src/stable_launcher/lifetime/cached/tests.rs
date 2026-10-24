// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Capacity backpressure preserves cached startup and remains cancellable on the real executor.
use super::super::{
    startup_tests::absent_cache,
    tests::{finish, fixture},
};
use super::*;
use crate::updates::test_support::{observe_notifications, retain_capacity};

#[test]
fn capacity_deferral_preserves_startup_until_actual_release_wakes_admission() -> Result<()> {
    let (mut launcher, _, wake) = fixture()?;
    launcher.installed(None);
    launcher.cached = Some(absent_cache()?);
    let held = retain_capacity(
        launcher.client.as_ref().ok_or("Client absent")?,
        launcher.work_scope.as_ref().ok_or("Scope absent")?,
        launcher.jobs.allocate()?,
        &wake,
    )?;
    let until = launcher.until;
    launcher.advance_cached()?;
    let cached = launcher.cached.as_ref().ok_or("Capacity failure discarded cache")?;
    assert_eq!(cached.worker.progress(), Progress::Opening);
    assert!(!cached.worker.pending());
    assert!(cached.receipt.is_none());
    assert!(cached.cleanup_until.is_none());
    assert!(cached.failure.is_none());
    assert!(matches!(launcher.choice, Choice::Cache));
    drop(held);
    observe_notifications(&wake, || {
        launcher.advance_cached()?;
        Ok(launcher.cached.as_ref().is_some_and(|cached| cached.worker.pending()))
    })?;
    assert_eq!(launcher.until, until);
    assert!(launcher.cached.as_ref().ok_or("Cache absent")?.receipt.is_some());
    launcher.request_shutdown()?;
    finish(&mut launcher, &wake)
}

#[test]
fn closing_capacity_deferred_startup_retires_without_admitting_storage() -> Result<()> {
    let (mut launcher, _, wake) = fixture()?;
    launcher.installed(None);
    launcher.cached = Some(absent_cache()?);
    let held = retain_capacity(
        launcher.client.as_ref().ok_or("Client absent")?,
        launcher.work_scope.as_ref().ok_or("Scope absent")?,
        launcher.jobs.allocate()?,
        &wake,
    )?;
    launcher.advance_cached()?;
    assert!(!launcher.cached.as_ref().ok_or("Cache absent")?.worker.pending());
    launcher.request_shutdown()?;
    drop(held);
    finish(&mut launcher, &wake)?;
    assert!(launcher.cached.is_none());
    assert!(!launcher.completed);
    Ok(())
}
