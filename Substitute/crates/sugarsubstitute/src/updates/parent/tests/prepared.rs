// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepared helper adoption takes unique input and bounds only probes by an outer cutoff.
use super::*;
use crate::updates::test_support::untrusted_input;

/// Keep scope admission live so the constructor alone decides the helper's cutoff.
fn scope(client: &ExecutionClient) -> Result<LaunchScope> {
    Ok(LaunchScope {
        work: client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?,
        cleanup: client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?,
        native: ScopeSpec { id: ScopeId::new(3)?, deadline: None },
        key: CoordinatorKey {
            namespace: NamespaceId::new([5; 32]),
            epoch: CoordinatorEpoch::new([8; 16])?,
        },
    })
}

#[test]
fn adoption_without_input_is_rejected_before_any_helper_exists() -> TestResult {
    let clock = Arc::new(Manual::default());
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let mut input = None;
    let scope = scope(&client).map_err(|cause| cause as Box<dyn Error>)?;
    assert!(Parent::from_input(client.clone(), scope, &mut input, None).is_err());
    owner.request_shutdown(ShutdownBudget::new(clock.now())?.settle_until);
    observe(&wake, || {
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        Ok(owner.poll_shutdown()?.settled)
    })?;
    owner.finish_shutdown()?;
    Ok(())
}

#[test]
fn adopted_preparation_input_has_no_outer_cutoff_and_returns_completion_once() -> TestResult {
    let clock = Arc::new(Manual::default());
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let scope = scope(&client).map_err(|cause| cause as Box<dyn Error>)?;
    let mut input = Some(untrusted_input()?);
    let mut parent = Parent::from_input(client, scope, &mut input, None)
        .map_err(|cause| cause as Box<dyn Error>)?;
    assert!(input.is_none());
    assert_eq!(parent.until, None);
    assert_eq!(parent.next_deadline(), None);
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
