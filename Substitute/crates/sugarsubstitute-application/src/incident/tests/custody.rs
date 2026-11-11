// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep actual incident and queued mutation duties ahead of an ordinary idle request.
use super::support::*;

#[test]
fn diagnostic_custody_clean_seed_and_failed_collection_have_distinct_close_duties() -> TestResult {
    let mut coordinator = IncidentCoordinator::new(owner(2)?);
    let mut resources = ResourceCoordinator::new(owner(2)?);
    register(&mut resources, run(1)?)?;
    let seed = seed(run(1)?)?;
    assert!(!coordinator.diagnostic_custody_pending());
    let effects = coordinator.seed(&mut resources, seed.clone(), context(1, 100)?)?;
    assert!(coordinator.diagnostic_custody_pending());
    let Some(IncidentEffect::PublishSeed { scope, .. }) = effects.iter().next() else {
        return Err("Genuine seed publication missing".into());
    };
    coordinator.observe(IncidentServiceObservation::SeedCommitted { scope: *scope, seed })?;
    assert!(!coordinator.diagnostic_custody_pending());
    coordinator.failed_desktop(
        &mut resources,
        exit(run(1)?, None)?.failed_incident()?,
        FailureEvidence::Unexplained,
        context(2, 101)?,
    )?;
    assert!(coordinator.diagnostic_custody_pending());
    Ok(())
}

#[test]
fn diagnostic_custody_acknowledgement_waits_independent_sources_family_and_reporter() -> TestResult
{
    let mut actual = ready(1)?;
    assert!(actual.coord.diagnostic_custody_pending());
    present(&mut actual)?;
    acknowledge(&mut actual, false)?;
    assert!(actual.coord.diagnostic_custody_pending());
    actual.coord.observe(IncidentServiceObservation::SourcesRetired {
        scope: IncidentSourceScope::Live(actual.source),
    })?;
    actual.coord.observe(IncidentServiceObservation::FamilySettled { run: run(1)? })?;
    assert!(actual.coord.diagnostic_custody_pending());
    actual
        .coord
        .observe(IncidentServiceObservation::ReporterRetired { binding: actual.binding })?;
    assert!(!actual.coord.diagnostic_custody_pending());
    Ok(())
}
