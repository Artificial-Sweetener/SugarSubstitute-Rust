// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reuse actual constructor custody and consume real repository presentation facts.
use super::*;

/// Prepare the genuine controller using only the existing external entry-boundary fixture.
pub(in crate::production) fn prepared_host(
    client: &ExecutionClient,
    scope: &Scope,
    wake: &Wake,
    jobs: &JobSequence,
) -> TestResult<(IncidentHost, CoordinatorEntryOwner)> {
    let mut entry = entry_owner()?;
    let namespace = namespace(&mut entry, client, scope, wake, jobs)?;
    let until = namespace.original_preparation().until();
    let mut input = Some(PreparationInput::new(namespace, key()?));
    let mut host = IncidentHost::new();
    host.submit_prepare(client, scope, jobs, until, &mut input)?;
    captured(&mut host, client, wake)?;
    Ok((host, entry))
}
/// Project the actual repository fixture's original and presented CAS, with no native proof.
pub(in crate::production) fn present(
    host: &mut IncidentHost,
    original: IncidentSummary,
    presented: IncidentSummary,
    context: IncidentRequestContext,
) -> TestResult<ReportBinding> {
    let binding = recover(host, original, context)?;
    let coordinator = &mut host.vacant()?.coordinator;
    let effects =
        coordinator.observe(IncidentServiceObservation::ReporterPresented { binding, context })?;
    let scope = effects
        .iter()
        .find_map(|effect| match effect {
            IncidentEffect::RecordAttempt { scope, .. } => Some(*scope),
            _ => None,
        })
        .ok_or("Actual presentation scope missing")?;
    let effects = coordinator.observe(IncidentServiceObservation::AttemptCommitted {
        scope,
        binding,
        summary: presented,
    })?;
    let binding = effects
        .iter()
        .find_map(|effect| match effect {
            IncidentEffect::ReplaceReportView { current, .. } => Some(*current),
            _ => None,
        })
        .ok_or("Actual presented revision missing")?;
    assert!(
        coordinator
            .observe(IncidentServiceObservation::ReportViewReplaced { binding })?
            .iter()
            .next()
            .is_none()
    );
    Ok(binding)
}

/// Project only the external canonical summary; no family or native retirement is inferred.
pub(in crate::production) fn recover(
    host: &mut IncidentHost,
    original: IncidentSummary,
    context: IncidentRequestContext,
) -> TestResult<ReportBinding> {
    let coordinator = &mut host.vacant()?.coordinator;
    let effects = coordinator.begin_recovery(context.budget())?;
    let scope = effects
        .iter()
        .find_map(|effect| match effect {
            IncidentEffect::Recover { scope } => Some(*scope),
            _ => None,
        })
        .ok_or("Actual recovery scope missing")?;
    let effects = coordinator.observe(IncidentServiceObservation::Recovered {
        scope,
        original_operation: None,
        summary: original,
    })?;
    let binding = effects
        .iter()
        .find_map(|effect| match effect {
            IncidentEffect::LaunchReporter { binding, .. } => Some(*binding),
            _ => None,
        })
        .ok_or("Actual recovered report missing")?;
    Ok(binding)
}
/// Consume the actual finite store completion or cleanup from its causal execution wake.
pub(in crate::production) fn store_completed(
    host: &mut IncidentHost,
    client: &ExecutionClient,
    wake: &Wake,
    cleanup: bool,
) -> TestResult {
    delivery(client, wake, |batch| {
        host.capture_reporting(batch)?;
        let service = host.reporting_service()?;
        Ok(if cleanup { service.cleanup_outcome() } else { service.outcome() })
    })?;
    Ok(())
}
