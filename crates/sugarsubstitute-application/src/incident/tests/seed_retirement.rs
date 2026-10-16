// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise exact failed-seed retirement without fabricating commit or unrelated custody facts.
use super::support::*;

fn pending(
    coordinator: &mut IncidentCoordinator,
    resources: &mut ResourceCoordinator,
    id: u64,
) -> Result<IncidentOperationScope, Box<dyn std::error::Error>> {
    register(resources, run(id)?)?;
    match coordinator.seed(resources, seed(run(id)?)?, context(id, 100)?)?.iter().next() {
        Some(IncidentEffect::PublishSeed { scope, .. }) => Ok(*scope),
        _ => Err("Actual seed publication missing".into()),
    }
}
#[test]
fn retired_failed_seed_frees_only_its_duty_and_preserves_failure() -> TestResult {
    let mut coordinator = IncidentCoordinator::new(owner(2)?);
    let mut resources = ResourceCoordinator::new(owner(2)?);
    let first = pending(&mut coordinator, &mut resources, 1)?;
    let second = pending(&mut coordinator, &mut resources, 2)?;
    coordinator.observe(IncidentServiceObservation::Failed {
        scope: first,
        failure: IncidentServiceFailure::SeedPublication,
    })?;
    coordinator.observe(IncidentServiceObservation::SeedPublicationRetired { scope: first })?;
    let snapshot = coordinator.snapshot(first.incident()).ok_or("Retained seed missing")?;
    assert_eq!(snapshot.first_failure, Some(IncidentServiceFailure::SeedPublication));
    assert_eq!(snapshot.phase, IncidentServicePhase::Failed);
    assert_eq!(snapshot.current_operation, None);
    assert!(!snapshot.sources_retired);
    assert!(!snapshot.family_settled);
    assert_eq!(
        coordinator.snapshot(second.incident()).ok_or("Other seed missing")?.current_operation,
        Some(second)
    );
    assert!(coordinator.diagnostic_custody_pending());
    assert!(
        coordinator
            .observe(IncidentServiceObservation::SeedPublicationRetired { scope: first })?
            .is_empty()
    );
    coordinator.observe(IncidentServiceObservation::SeedPublicationRetired { scope: second })?;
    assert!(!coordinator.diagnostic_custody_pending());
    let third = pending(&mut coordinator, &mut resources, 3)?;
    assert_eq!(
        coordinator.snapshot(third.incident()).ok_or("New admission missing")?.current_operation,
        Some(third)
    );
    Ok(())
}
#[test]
fn changed_seed_scope_and_committed_seed_reject_retirement() -> TestResult {
    let mut coordinator = IncidentCoordinator::new(owner(2)?);
    let mut resources = ResourceCoordinator::new(owner(2)?);
    let scope = pending(&mut coordinator, &mut resources, 1)?;
    let mut changed = scope;
    changed.original = context(9, 100)?.budget();
    assert_eq!(
        coordinator.observe(IncidentServiceObservation::SeedPublicationRetired { scope: changed }),
        Err(IncidentDecisionError::StaleOperation)
    );
    assert_eq!(
        coordinator.snapshot(scope.incident()).ok_or("Seed missing")?.current_operation,
        Some(scope)
    );
    coordinator
        .observe(IncidentServiceObservation::SeedCommitted { scope, seed: seed(run(1)?)? })?;
    assert_eq!(
        coordinator.observe(IncidentServiceObservation::SeedPublicationRetired { scope }),
        Err(IncidentDecisionError::WrongPhase)
    );
    assert_eq!(coordinator.snapshot(scope.incident()).ok_or("Seed missing")?.first_failure, None);
    Ok(())
}
