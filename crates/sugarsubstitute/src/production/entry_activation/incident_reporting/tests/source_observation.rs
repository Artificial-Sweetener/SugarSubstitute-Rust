// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Source facts are inert boundary inputs; these tests create no verified peer or retirement proof.
use super::*;
use sugarsubstitute_contracts::lifecycle::{
    CoordinatorEpoch, CoordinatorKey, NamespaceId, ProcessId,
};
fn identity(session: u64, process: u64) -> Result<(DiagnosticKey, ProcessKey)> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    Ok((
        DiagnosticKey::new(owner, DiagnosticRole::Reporter, DiagnosticSessionId::new(session)?),
        ProcessKey { owner, id: ProcessId::new(process)? },
    ))
}
fn facts(process: Option<ProcessKey>, authenticated: bool, failed: bool) -> ReporterFacts {
    ReporterFacts { changed: true, failed, authenticated, process, pipe_settled: false }
}
#[test]
fn reporter_initial_view_admission_keeps_preauth_pending_without_renewing_original_cutoffs()
-> Result<()> {
    let (key, process) = identity(1, 1)?;
    let mut receiving = IncidentReporting::new();
    receiving.key = Some(key);
    receiving.initial_view_admitted = true;
    let original = IncidentOperationBudget::new(Moment::from_nanos(17))?;
    receiving.report_budget = Some(original);
    receiving.queue_until = Some(original.until());
    for process in [None, Some(process)] {
        receiving.observe_source_facts(facts(process, false, false))?;
        assert_eq!(receiving.process, process);
        assert_eq!(receiving.authenticated_peer_observed, None);
        assert!(receiving.initial_view_admitted);
        assert_eq!(receiving.report_budget, Some(original));
        assert_eq!(receiving.queue_until, Some(original.until()));
        assert_eq!(receiving.next_deadline(), Some(original.until()));
        assert!(receiving.relay_needed);
        assert!(receiving.failure().is_none());
    }
    Ok(())
}
#[test]
fn reporter_positive_observation_records_exact_identity_without_admitting_a_view_or_request()
-> Result<()> {
    let (key, process) = identity(1, 1)?;
    let mut receiving = IncidentReporting::new();
    receiving.key = Some(key);
    receiving.observe_source_facts(facts(Some(process), true, false))?;
    receiving.observe_source_facts(facts(Some(process), true, false))?;
    assert_eq!(receiving.authenticated_peer_observed, Some((key, process)));
    assert!(!receiving.initial_view_admitted);
    assert!(receiving.store.is_none());
    assert!(receiving.request.is_none());
    assert!(receiving.completion.is_none());
    assert!(receiving.relay.is_none());
    Ok(())
}
#[test]
fn reporter_positive_observation_never_masks_failed_source_even_with_sticky_peer() -> Result<()> {
    let (key, process) = identity(1, 1)?;
    for authenticated in [false, true] {
        let mut receiving = IncidentReporting::new();
        receiving.key = Some(key);
        receiving.observe_source_facts(facts(Some(process), true, false))?;
        receiving.first = Some(Box::new(std::io::Error::other("Original source boundary cause")));
        let original = std::ptr::from_ref(receiving.failure().ok_or("Cause missing")?).cast::<()>();
        let rejection = receiving
            .observe_source_facts(facts(Some(process), authenticated, true))
            .err()
            .ok_or("Failed source was accepted")?;
        assert_eq!(
            rejection.to_string(),
            "Actual Reporter source failed with original cause retained"
        );
        assert_eq!(receiving.authenticated_peer_observed, Some((key, process)));
        assert_eq!(
            std::ptr::from_ref(receiving.failure().ok_or("Original cause lost")?).cast::<()>(),
            original
        );
        let cause = receiving.failure().and_then(|cause| cause.downcast_ref::<std::io::Error>());
        assert_eq!(
            cause.map(ToString::to_string).as_deref(),
            Some("Original source boundary cause")
        );
    }
    Ok(())
}
#[test]
fn reporter_lost_observation_after_positive_is_rejected_without_resetting_identity() -> Result<()> {
    let (key, process) = identity(1, 1)?;
    let mut receiving = IncidentReporting::new();
    receiving.key = Some(key);
    receiving.observe_source_facts(facts(Some(process), true, false))?;
    assert!(receiving.observe_source_facts(facts(Some(process), false, false)).is_err());
    assert_eq!(receiving.authenticated_peer_observed, Some((key, process)));
    assert_eq!(receiving.process, Some(process));
    Ok(())
}
#[test]
fn reporter_observed_session_and_process_cannot_be_replaced_or_omitted() -> Result<()> {
    let (key, process) = identity(1, 1)?;
    let (other_key, other_process) = identity(2, 2)?;
    for (next_key, next_process) in
        [(other_key, Some(process)), (key, Some(other_process)), (key, None)]
    {
        let mut receiving = IncidentReporting::new();
        receiving.key = Some(key);
        receiving.observe_source_facts(facts(Some(process), true, false))?;
        receiving.key = Some(next_key);
        assert!(receiving.observe_source_facts(facts(next_process, true, false)).is_err());
        assert_eq!(receiving.authenticated_peer_observed, Some((key, process)));
        assert_eq!(receiving.process, Some(process));
    }
    Ok(())
}
#[test]
fn reporter_invalid_first_positive_or_failed_preauth_cannot_latch_authentication() -> Result<()> {
    let (key, process) = identity(1, 1)?;
    let foreign = ProcessKey {
        owner: CoordinatorKey { epoch: CoordinatorEpoch::new([3; 16])?, ..process.owner },
        ..process
    };
    for observation in [
        facts(None, true, false),
        facts(Some(foreign), true, false),
        facts(Some(process), true, true),
        facts(Some(process), false, true),
    ] {
        let mut receiving = IncidentReporting::new();
        receiving.key = Some(key);
        assert!(receiving.observe_source_facts(observation).is_err());
        assert_eq!(receiving.authenticated_peer_observed, None);
    }
    Ok(())
}
#[test]
fn reporter_close_and_unproven_recycling_cannot_reset_observed_authentication() -> Result<()> {
    let (key, process) = identity(1, 1)?;
    let mut receiving = IncidentReporting::new();
    receiving.key = Some(key);
    receiving.observe_source_facts(facts(Some(process), true, false))?;
    receiving.finish_accepted_session()?;
    assert_eq!(receiving.authenticated_peer_observed, Some((key, process)));
    let budget = ShutdownBudget::new(Moment::from_nanos(17))?;
    receiving.request_close(budget);
    assert_eq!(receiving.authenticated_peer_observed, Some((key, process)));
    receiving.family_retired = true;
    assert!(receiving.finish_accepted_session().is_err());
    assert_eq!(receiving.authenticated_peer_observed, Some((key, process)));
    assert_eq!(receiving.normal_close_budget(), None);
    assert_eq!(receiving.next_deadline(), Some(budget.settle_until));
    Ok(())
}
