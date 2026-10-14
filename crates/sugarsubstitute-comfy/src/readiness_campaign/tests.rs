// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real campaign, source inspection, probe completion and cleanup owners.
mod clock;
mod harness;
mod incremental;
mod terminal;
use super::*;
use crate::readiness_transport::campaign_fixture::Behavior;
use harness::Harness;
use std::error::Error;
use sugarsubstitute_contracts::execution::ExecutionPort;
use sugarsubstitute_contracts::{
    configuration::LoopbackOrigin,
    execution::{CompletionBatch, CompletionNotification, JobId, ScopeId, TerminalKind},
};
type TestResult<T = ()> = Result<T, Box<dyn Error>>;
fn attached_target(port: u16) -> TestResult<BackendTarget> {
    Ok(BackendTarget::Attached(LoopbackOrigin::parse(&format!("http://127.0.0.1:{port}"), port)?))
}
fn empty_batch() -> CompletionBatch {
    CompletionBatch { notifications: Vec::new(), remaining: 0 }
}

#[test]
fn actual_success_delivers_once_and_retains_attached_identity_until_final_cleanup() -> TestResult {
    let mut harness = Harness::new()?;
    let fixture = harness.submit(Behavior::Success, None, false, None)?;
    assert_eq!(harness.completion()?, CampaignStep::Ready);
    let proof = harness.campaign.take_readiness().ok_or("Missing actual readiness")?;
    assert!(harness.campaign.take_readiness().is_none());
    assert_eq!(proof.process_id(), 4242);
    assert_eq!(proof.source().manifest().frontend_version(), "1.47.11");
    proof.validate_current()?;
    assert!(!harness.campaign.settled());
    assert!(harness.retained_bytes() > 0);
    assert_eq!(
        *fixture.closed.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
        [3, 2, 1]
    );
    harness.set_time(40_000_000_000);
    proof.validate_current()?;
    harness.campaign.cancel(ShutdownBudget::new(harness.now())?);
    harness.submit_close(Moment::from_nanos(55_000_000_000))?;
    assert_eq!(harness.completion()?, CampaignStep::Failed);
    assert!(proof.validate_current().is_err());
    assert!(harness.campaign.settled());
    assert_eq!(harness.retained_bytes(), 0);
    drop(proof);
    harness.finish()
}

#[test]
fn transient_retry_waits_for_actual_partial_close_and_original_backoff() -> TestResult {
    let mut harness = Harness::new()?;
    let first = harness.submit(Behavior::Unavailable, Some("close"), false, None)?;
    assert_eq!(harness.completion()?, CampaignStep::Progressed);
    assert!(!harness.campaign.needs_input(harness.now())?);
    assert_eq!(harness.retained_bytes(), crate::readiness_transport::READINESS_TRANSPORT_BYTES);
    harness.submit_close(Moment::from_nanos(30_000_000_000))?;
    first.entered()?;
    assert!(!harness.campaign.needs_input(harness.now())?);
    assert!(!harness.campaign.settled());
    first.release();
    assert_eq!(harness.completion()?, CampaignStep::Progressed);
    assert_eq!(harness.retained_bytes(), crate::readiness_transport::READINESS_TRANSPORT_BYTES);
    let cause = harness.campaign.take_failure().ok_or("Missing retained transient cause")?;
    assert_eq!(harness.retained_bytes(), crate::readiness_transport::READINESS_TRANSPORT_BYTES);
    drop(cause);
    assert_eq!(harness.retained_bytes(), 0);
    assert_eq!(harness.campaign.next_deadline(), Some(Moment::from_nanos(50_000_000)));
    assert!(!harness.campaign.needs_input(Moment::from_nanos(49_999_999))?);
    harness.set_time(50_000_000);
    assert_eq!(harness.campaign.consume(&empty_batch(), harness.now())?, CampaignStep::NeedsInput);
    let next = harness.submit(Behavior::Success, None, false, None)?;
    assert_eq!(harness.completion()?, CampaignStep::Ready);
    assert_eq!(harness.campaign.budget.until(), Moment::from_nanos(30_000_000_000));
    assert!(
        next.phases
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .all(|(_, deadline)| *deadline == 2_050_000_000)
    );
    harness.finish()
}

#[test]
fn backoff_increases_to_one_second_without_renewing_campaign() -> TestResult {
    let mut harness = Harness::new()?;
    let mut now = 0;
    for delay in [
        50_000_000,
        100_000_000,
        200_000_000,
        400_000_000,
        800_000_000,
        1_000_000_000,
        1_000_000_000,
    ] {
        harness.submit(Behavior::Unavailable, None, false, None)?;
        assert_eq!(harness.completion()?, CampaignStep::Progressed);
        harness.submit_close(Moment::from_nanos(30_000_000_000))?;
        harness.completion()?;
        now += delay;
        assert_eq!(harness.campaign.next_deadline(), Some(Moment::from_nanos(now)));
        harness.set_time(now);
        assert!(harness.campaign.needs_input(harness.now())?);
        assert_eq!(harness.campaign.budget.until(), Moment::from_nanos(30_000_000_000));
    }
    harness.finish()
}

#[test]
fn original_expiry_caps_final_backoff_and_closes_retry_admission() -> TestResult {
    let mut harness = Harness::new()?;
    harness.set_time(29_999_000_000);
    harness.submit(Behavior::Unavailable, None, false, None)?;
    harness.completion()?;
    harness.submit_close(Moment::from_nanos(30_000_000_000))?;
    harness.completion()?;
    assert_eq!(harness.campaign.next_deadline(), Some(Moment::from_nanos(30_000_000_000)));
    harness.set_time(30_000_000_000);
    assert_eq!(harness.campaign.consume(&empty_batch(), harness.now())?, CampaignStep::Failed);
    assert!(matches!(
        harness.campaign.failure(),
        Some(CampaignError::Budget(ProbeBudgetError::Expired))
    ));
    assert!(!harness.campaign.needs_input(harness.now())?);
    harness.finish()
}

#[test]
fn actual_success_delivered_at_probe_deadline_is_terminal_and_requires_close() -> TestResult {
    let mut harness = Harness::new()?;
    let fixture = harness.submit(Behavior::Success, Some("close"), false, None)?;
    fixture.entered()?;
    fixture.release();
    let batch = harness
        .client
        .drain_completions(std::num::NonZeroUsize::new(32).ok_or("Invalid drain")?)?;
    // Observe actual terminal completion causally before advancing only the injected clock.
    if batch.notifications.is_empty() {
        // The fixture remains complete; use the central wake to wait for real metadata.
        harness.wait_terminal_without_consuming()?;
    } else {
        harness.save_batch(batch);
    }
    harness.set_time(2_000_000_000);
    assert_eq!(harness.consume_saved()?, CampaignStep::Failed);
    assert!(harness.campaign.take_readiness().is_none());
    assert!(!harness.campaign.settled());
    harness.finish()
}

#[test]
fn forged_or_foreign_metadata_cannot_transfer_an_inflight_result() -> TestResult {
    let mut harness = Harness::new()?;
    let fixture = harness.submit(Behavior::Success, Some("begin"), false, None)?;
    fixture.entered()?;
    let attempt = harness.campaign.attempt.as_ref().ok_or("Missing attempt")?;
    let exact = CompletionNotification {
        scope_id: attempt.scope,
        job_id: attempt.job,
        kind: TerminalKind::Success,
        replacement: None,
    };
    for notification in [
        CompletionNotification { scope_id: ScopeId::new(90)?, ..exact },
        CompletionNotification { job_id: JobId::new(90)?, ..exact },
        exact,
    ] {
        assert_eq!(
            harness.campaign.consume(
                &CompletionBatch { notifications: vec![notification], remaining: 0 },
                harness.now()
            )?,
            CampaignStep::Waiting
        );
        assert!(harness.campaign.take_readiness().is_none());
        assert!(harness.campaign.attempt.is_some());
    }
    fixture.release();
    assert_eq!(harness.completion()?, CampaignStep::Ready);
    harness.finish()
}

#[test]
fn original_probe_bounds_and_input_target_reject_before_external_work() -> TestResult {
    let mut harness = Harness::new()?;
    let input = ReadinessProbeInput::attached(harness.lease.clone(), 8189)?;
    let spec = harness.spec(Moment::from_nanos(2_000_000_000))?;
    assert!(matches!(
        harness.campaign.submit_attempt(&harness.client, &harness.attempt, spec, input),
        Err(CampaignError::Rejected(CampaignRejection::WrongTarget))
    ));
    let input = ReadinessProbeInput::attached(harness.lease.clone(), 8188)?;
    let spec = harness.spec(Moment::from_nanos(2_000_000_001))?;
    assert!(matches!(
        harness.campaign.submit_attempt(&harness.client, &harness.attempt, spec, input),
        Err(CampaignError::Rejected(CampaignRejection::InvalidSpecification))
    ));
    assert!(harness.campaign.settled());
    assert_eq!(harness.retained_bytes(), 0);
    harness.finish()
}

#[test]
fn unsupported_origins_budget_kind_and_regressing_time_reject() -> TestResult {
    let attached = ConnectionBudget::new(ConnectionBudgetKind::Attached, Moment::from_nanos(10))?;
    for origin in ["http://localhost:8188", "http://[::1]:8188"] {
        assert!(matches!(
            ReadinessCampaign::new(
                BackendTarget::Attached(LoopbackOrigin::parse(origin, 8188)?),
                attached
            ),
            Err(CampaignError::Rejected(CampaignRejection::WrongTarget))
        ));
    }
    let managed = ConnectionBudget::new(ConnectionBudgetKind::Managed, Moment::from_nanos(10))?;
    assert!(ReadinessCampaign::new(attached_target(8188)?, managed).is_err());
    let campaign = ReadinessCampaign::new(attached_target(8188)?, attached)?;
    assert!(matches!(
        campaign.needs_input(Moment::from_nanos(9)),
        Err(CampaignError::Rejected(CampaignRejection::ClockRegression))
    ));
    Ok(())
}

#[test]
fn foreign_cleanup_rejection_keeps_custody_for_actual_owning_executor() -> TestResult {
    let mut harness = Harness::new()?;
    let other = Harness::new()?;
    harness.submit(Behavior::Unavailable, None, false, None)?;
    harness.completion()?;
    let original_bytes = harness.retained_bytes();
    let spec = harness.spec(Moment::from_nanos(5_000_000_000))?;
    assert!(matches!(
        harness.campaign.submit_cleanup(&harness.client, &other.cleanup, spec),
        Err(CampaignError::Rejected(CampaignRejection::ForeignExecution))
    ));
    assert_eq!(harness.retained_bytes(), original_bytes);
    assert!(!harness.campaign.settled());
    harness.finish()?;
    other.finish()
}
