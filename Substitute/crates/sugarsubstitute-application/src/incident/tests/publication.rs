// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve exact immutable publication content through interrupted use cases.
use super::support::*;

#[test]
fn seed_commit_requires_exact_scope_and_content() -> TestResult {
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let mut resources = ResourceCoordinator::new(owner(2)?);
    register(&mut resources, run(1)?)?;
    let seed = seed(run(1)?)?;
    let effects = coord.seed(&mut resources, seed.clone(), context(1, 100)?)?;
    let scope = match effects.iter().next() {
        Some(IncidentEffect::PublishSeed { scope, .. }) => *scope,
        _ => return Err("Missing scope".into()),
    };
    assert_eq!(
        coord.observe(IncidentServiceObservation::SeedCommitted {
            scope,
            seed: super::support::seed(run(2)?)?
        }),
        Err(IncidentDecisionError::MismatchedCommit)
    );
    let mut foreign = scope;
    foreign.original = context(2, 100)?.budget();
    assert_eq!(
        coord.observe(IncidentServiceObservation::SeedCommitted {
            scope: foreign,
            seed: seed.clone()
        }),
        Err(IncidentDecisionError::StaleOperation)
    );
    coord.observe(IncidentServiceObservation::SeedCommitted { scope, seed: seed.clone() })?;
    assert!(coord.observe(IncidentServiceObservation::SeedCommitted { scope, seed })?.is_empty());
    Ok(())
}
#[test]
fn collection_retry_keeps_original_scope_budget_and_first_failure() -> TestResult {
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let mut resources = ResourceCoordinator::new(owner(2)?);
    register(&mut resources, run(1)?)?;
    let seed = seed(run(1)?)?;
    seed_ready(&mut coord, &mut resources, seed.clone())?;
    let effects = coord.failed_desktop(
        &mut resources,
        exit(run(1)?, None)?.failed_incident()?,
        FailureEvidence::Unexplained,
        context(2, 101)?,
    )?;
    let scope = match effects.iter().next() {
        Some(IncidentEffect::CollectEvidence { scope, .. }) => *scope,
        _ => return Err("Missing collection".into()),
    };
    coord.observe(IncidentServiceObservation::Failed {
        scope,
        failure: IncidentServiceFailure::Collection,
    })?;
    let effects = coord
        .observe(IncidentServiceObservation::EvidenceCollected { scope, evidence: evidence()? })?;
    let summary = match effects.iter().next() {
        Some(IncidentEffect::PublishSummary { scope: actual, summary }) => {
            assert_eq!(*actual, scope);
            summary.clone()
        }
        _ => return Err("Missing summary".into()),
    };
    coord.observe(IncidentServiceObservation::Failed {
        scope,
        failure: IncidentServiceFailure::SummaryPublication,
    })?;
    coord.observe(IncidentServiceObservation::SummaryCommitted { scope, summary })?;
    assert_eq!(
        coord.snapshot(seed.key()).ok_or("No snapshot")?.first_failure,
        Some(IncidentServiceFailure::Collection)
    );
    Ok(())
}
#[test]
fn dump_collection_does_not_reclassify_unexplained_exit() -> TestResult {
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let mut resources = ResourceCoordinator::new(owner(2)?);
    register(&mut resources, run(1)?)?;
    seed_ready(&mut coord, &mut resources, seed(run(1)?)?)?;
    let effects = coord.failed_desktop(
        &mut resources,
        exit(run(1)?, None)?.failed_incident()?,
        FailureEvidence::Unexplained,
        context(2, 101)?,
    )?;
    let scope = match effects.iter().next() {
        Some(IncidentEffect::CollectEvidence { scope, .. }) => *scope,
        _ => return Err("No scope".into()),
    };
    let evidence = CollectedIncidentEvidence::new(
        IncidentUtc::from_unix_seconds(101)?,
        CaptureEvidence::Finished(DumpId::new([7; 16])?),
        PayloadDisposition::Unavailable,
        SymbolOutcome::NotRequested,
    );
    let effects =
        coord.observe(IncidentServiceObservation::EvidenceCollected { scope, evidence })?;
    match effects.iter().next() {
        Some(IncidentEffect::PublishSummary { summary, .. }) => {
            assert_eq!(summary.facts().failure(), FailureEvidence::Unexplained);
            assert_eq!(
                summary.facts().exit(),
                ExitAttribution::Desktop { kind: DesktopExitKind::Unexpected, status: 0 }
            );
        }
        _ => return Err("No summary".into()),
    };
    Ok(())
}

#[test]
fn awaiting_capture_cannot_complete_collection_or_change_original_scope() -> TestResult {
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let mut resources = ResourceCoordinator::new(owner(2)?);
    register(&mut resources, run(1)?)?;
    seed_ready(&mut coord, &mut resources, seed(run(1)?)?)?;
    let effects = coord.failed_desktop(
        &mut resources,
        exit(run(1)?, None)?.failed_incident()?,
        FailureEvidence::Unexplained,
        context(2, 101)?,
    )?;
    let scope = match effects.iter().next() {
        Some(IncidentEffect::CollectEvidence { scope, .. }) => *scope,
        _ => return Err("No scope".into()),
    };
    let evidence = CollectedIncidentEvidence::new(
        IncidentUtc::from_unix_seconds(101)?,
        CaptureEvidence::Awaiting,
        PayloadDisposition::Unavailable,
        SymbolOutcome::NotRequested,
    );
    assert_eq!(
        coord.observe(IncidentServiceObservation::EvidenceCollected { scope, evidence }),
        Err(IncidentDecisionError::Contract(IncidentError::InvalidEvidence))
    );
    assert_eq!(
        coord.snapshot(seed(run(1)?)?.key()).ok_or("No snapshot")?.current_operation,
        Some(scope)
    );
    Ok(())
}
#[test]
fn mismatched_summary_and_session_exhaustion_leave_requested_content_pending() -> TestResult {
    let mut ready = ready(1)?;
    assert!(
        ready
            .coord
            .observe(IncidentServiceObservation::SummaryCommitted {
                scope: ready.source,
                summary: ready.summary.clone()
            })?
            .is_empty()
    );
    let changed =
        changed(&ready.summary, ReportDisposition::new(1, Some(ReportAttempt::Presented))?, None)?;
    assert_eq!(
        ready.coord.observe(IncidentServiceObservation::SummaryCommitted {
            scope: ready.source,
            summary: changed
        }),
        Err(IncidentDecisionError::StaleOperation)
    );
    let mut coord = IncidentCoordinator::new(owner(2)?);
    let mut resources = ResourceCoordinator::new(owner(2)?);
    register(&mut resources, run(1)?)?;
    seed_ready(&mut coord, &mut resources, ready.seed.clone())?;
    let effects = coord.failed_desktop(
        &mut resources,
        exit(run(1)?, None)?.failed_incident()?,
        FailureEvidence::Unexplained,
        context(2, 101)?,
    )?;
    let scope = match effects.iter().next() {
        Some(IncidentEffect::CollectEvidence { scope, .. }) => *scope,
        _ => return Err("No scope".into()),
    };
    coord
        .observe(IncidentServiceObservation::EvidenceCollected { scope, evidence: evidence()? })?;
    coord.next_session = u64::MAX;
    assert_eq!(
        coord.observe(IncidentServiceObservation::SummaryCommitted {
            scope,
            summary: ready.summary.clone()
        }),
        Err(IncidentDecisionError::ReportSessionExhausted)
    );
    coord.next_session = 0;
    assert_eq!(
        coord
            .observe(IncidentServiceObservation::SummaryCommitted {
                scope,
                summary: ready.summary
            })?
            .len(),
        2
    );
    Ok(())
}
