// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepared helper adoption preserves input on error and does not renew campaign authority.
use super::*;
use crate::updates::test_support::untrusted_input;
use std::time::Duration;

/// Keep scope admission live so the constructor itself must reject an expired campaign.
fn scope(client: &ExecutionClient, until: Moment) -> Result<LaunchScope> {
    Ok(LaunchScope {
        work: client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?,
        cleanup: client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?,
        native: ScopeSpec { id: ScopeId::new(3)?, deadline: None },
        key: CoordinatorKey {
            namespace: NamespaceId::new([5; 32]),
            epoch: CoordinatorEpoch::new([8; 16])?,
        },
        until,
    })
}

#[test]
fn failed_adoption_at_original_cutoff_preserves_the_callers_unique_input() -> TestResult {
    let clock = Arc::new(Manual::default());
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let mut input = Some(untrusted_input()?);
    let scope = scope(&client, clock.now()).map_err(|cause| cause as Box<dyn Error>)?;
    assert!(Parent::from_input(client.clone(), scope, &mut input).is_err());
    assert!(input.is_some());
    owner.request_shutdown(ShutdownBudget::new(clock.now())?.settle_until);
    observe(&wake, || {
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        Ok(owner.poll_shutdown()?.settled)
    })?;
    owner.finish_shutdown()?;
    Ok(())
}

#[test]
fn adopted_input_uses_original_cutoff_and_returns_observed_completion_once() -> TestResult {
    let clock = Arc::new(Manual::default());
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let until = clock.now().checked_add(Duration::from_secs(7))?;
    let scope = scope(&client, until).map_err(|cause| cause as Box<dyn Error>)?;
    let mut input = Some(untrusted_input()?);
    let mut parent =
        Parent::from_input(client, scope, &mut input).map_err(|cause| cause as Box<dyn Error>)?;
    assert!(input.is_none());
    assert_eq!(parent.until, until);
    assert!(!parent.borrows_application());
    assert!(parent.take_completion().is_none());
    parent.close(None).map_err(|cause| cause as Box<dyn Error>)?;
    finish(&mut parent, &mut owner, &wake, &JobSequence::new())?;
    let done = parent.take_completion().ok_or("Terminal completion absent")?;
    assert!(!done.succeeded);
    assert!(done.failure.is_none());
    assert!(done.probe.is_none());
    assert!(parent.take_completion().is_none());
    Ok(())
}
