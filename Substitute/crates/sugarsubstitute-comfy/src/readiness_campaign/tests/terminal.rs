// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify terminal failure, cancellation and actual retained native retirement.
use super::*;

#[test]
fn actual_panicked_completion_keeps_original_body_credit_until_escaped_cause_drops() -> TestResult {
    let mut harness = Harness::new()?;
    harness.submit(Behavior::Panicked, None, false, None)?;
    assert_eq!(harness.completion()?, CampaignStep::Failed);
    harness.submit_close(Moment::from_nanos(5_000_000_000))?;
    assert_eq!(harness.completion()?, CampaignStep::Failed);
    assert!(harness.campaign.settled());
    let cause = harness.campaign.take_failure().ok_or("Missing actual panic cause")?;
    assert_eq!(harness.retained_bytes(), crate::readiness_transport::READINESS_TRANSPORT_BYTES);
    let CampaignError::Transport(ref error) = cause else {
        return Err("Missing transport panic".into());
    };
    assert_eq!(error.stage(), "attempt_panicked");
    assert_eq!(
        error.source().ok_or("Missing supervised panic")?.to_string(),
        "original-campaign-panic"
    );
    drop(cause);
    assert_eq!(harness.retained_bytes(), 0);
    harness.finish()
}

#[test]
fn absent_listener_keeps_original_campaign_and_delays_http_until_observed_cleanup_and_backoff()
-> TestResult {
    let mut harness = Harness::new()?;
    let until = harness.campaign.budget.until();
    let fixture = harness.submit(Behavior::ListenerAbsent, None, false, None)?;
    assert_eq!(harness.completion()?, CampaignStep::Progressed);
    let Some(CampaignError::Transport(cause)) = harness.campaign.failure() else {
        return Err("Actual absent listener result missing".into());
    };
    assert_eq!(cause.stage(), "http_listener_absent");
    assert!(!harness.campaign.ready && !harness.campaign.settled());
    assert!(!harness.campaign.needs_input(harness.now())?);
    assert!(harness.retained_bytes() > 0);
    assert!(fixture.requests.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_empty());
    harness.submit_close(Moment::from_nanos(5_000_000_000))?;
    harness.completion()?;
    assert_eq!(harness.retained_bytes(), crate::readiness_transport::READINESS_TRANSPORT_BYTES);
    let cause = harness.campaign.take_failure().ok_or("Missing actual absent listener cause")?;
    drop(cause);
    assert_eq!(harness.retained_bytes(), 0);
    assert!(fixture.closed.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_empty());
    assert_eq!(harness.campaign.budget.until(), until);
    assert!(!harness.campaign.needs_input(harness.now())?);
    harness.set_time(50_000_000);
    assert!(harness.campaign.needs_input(harness.now())?);
    harness.submit(Behavior::Success, None, false, None)?;
    assert_eq!(harness.completion()?, CampaignStep::Ready);
    assert_eq!(harness.campaign.budget.until(), until);
    harness.finish()
}

#[test]
fn cancelled_attempt_scope_cannot_be_replaced_to_resume_campaign() -> TestResult {
    let mut harness = Harness::new()?;
    harness.attempt.cancel();
    let input = ReadinessProbeInput::attached(harness.lease.clone(), 8188)?;
    let spec = harness.spec(Moment::from_nanos(2_000_000_000))?;
    assert!(matches!(
        harness.campaign.submit_attempt(&harness.client, &harness.attempt, spec, input),
        Err(CampaignError::Rejected(CampaignRejection::ClosedScope))
    ));
    assert!(!harness.campaign.needs_input(harness.now())?);
    assert!(harness.campaign.settled());
    assert_eq!(harness.retained_bytes(), 0);
    harness.finish()
}

#[test]
fn malformed_response_and_wrong_asset_are_terminal_with_retained_partial_custody() -> TestResult {
    for behavior in [Behavior::Malformed, Behavior::WrongAsset] {
        let mut harness = Harness::new()?;
        harness.submit(behavior, None, false, None)?;
        assert_eq!(harness.completion()?, CampaignStep::Failed);
        assert!(matches!(harness.campaign.failure(), Some(CampaignError::Transport(_))));
        assert!(!harness.campaign.needs_input(harness.now())?);
        assert!(!harness.campaign.settled());
        assert!(harness.retained_bytes() > 0);
        harness.submit_close(Moment::from_nanos(30_000_000_000))?;
        assert_eq!(harness.completion()?, CampaignStep::Failed);
        assert!(harness.campaign.settled());
        harness.finish()?;
    }
    Ok(())
}

#[test]
fn failed_partial_close_preserves_original_cause_and_deadline_blocks_successor() -> TestResult {
    let mut harness = Harness::new()?;
    let fixture = harness.submit(Behavior::Unavailable, None, true, None)?;
    harness.completion()?;
    harness.submit_close(Moment::from_nanos(5_000_000_000))?;
    let error = harness.completion().err().ok_or("Expected original cleanup failure")?;
    let CampaignError::Transport(cause) =
        *error.downcast::<CampaignError>().map_err(|_| "Missing typed campaign cause")?
    else {
        return Err("Missing cleanup transport cause".into());
    };
    assert_eq!(cause.source().ok_or("Missing cause")?.to_string(), "original-close-cause");
    assert!(!harness.campaign.settled());
    assert!(!harness.campaign.needs_input(harness.now())?);
    assert_eq!(harness.campaign.cleanup_until, Some(Moment::from_nanos(5_000_000_000)));
    let larger = harness.spec(Moment::from_nanos(6_000_000_000))?;
    assert!(matches!(
        harness.campaign.submit_cleanup(&harness.client, &harness.cleanup, larger),
        Err(CampaignError::Rejected(CampaignRejection::InvalidSpecification))
    ));
    harness.submit_close(Moment::from_nanos(5_000_000_000))?;
    harness.completion()?;
    assert_eq!(
        *fixture.closed.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
        [3, 2, 1]
    );
    assert!(harness.campaign.settled());
    harness.finish()
}

#[test]
fn cancellation_waits_for_actual_inflight_return_and_separate_cleanup() -> TestResult {
    let mut harness = Harness::new()?;
    let fixture = harness.submit(Behavior::Success, Some("begin"), false, None)?;
    fixture.entered()?;
    let original = ShutdownBudget::new(Moment::from_nanos(0))?;
    harness.campaign.cancel(original);
    harness.campaign.cancel(ShutdownBudget::new(Moment::from_nanos(100))?);
    assert_eq!(harness.campaign.shutdown, Some(original));
    harness.attempt.cancel();
    assert!(!harness.campaign.settled());
    assert!(harness.campaign.attempt.is_some());
    fixture.release();
    assert_eq!(harness.completion()?, CampaignStep::Failed);
    assert!(harness.campaign.attempt.is_none());
    assert!(!harness.campaign.settled());
    harness.submit_close(original.settle_until)?;
    harness.completion()?;
    assert!(harness.campaign.settled());
    assert_eq!(harness.retained_bytes(), crate::readiness_transport::READINESS_TRANSPORT_BYTES);
    let cause = harness.campaign.take_failure().ok_or("Missing actual cancelled completion")?;
    drop(cause);
    assert_eq!(harness.retained_bytes(), 0);
    harness.finish()
}

#[test]
fn actual_late_boundary_return_cannot_promote_discarded_success() -> TestResult {
    let mut harness = Harness::new()?;
    harness.submit(Behavior::Success, None, false, Some(Moment::from_nanos(2_000_000_000)))?;
    assert_eq!(harness.completion()?, CampaignStep::Failed);
    assert!(harness.campaign.take_readiness().is_none());
    assert!(!harness.campaign.settled());
    harness.finish()
}

#[test]
fn supervised_attempt_and_cleanup_panics_preserve_original_payloads_and_partial_custody()
-> TestResult {
    let mut harness = Harness::new()?;
    harness.submit(Behavior::Panicked, None, false, None)?;
    assert_eq!(harness.completion()?, CampaignStep::Failed);
    let Some(CampaignError::Transport(cause)) = harness.campaign.failure() else {
        return Err("Missing attempt panic cause".into());
    };
    assert_eq!(cause.stage(), "attempt_panicked");
    assert_eq!(
        cause.source().ok_or("Missing original panic")?.to_string(),
        "original-campaign-panic"
    );
    assert!(!harness.campaign.settled());
    harness.finish()?;
    let mut harness = Harness::new()?;
    harness.submit(Behavior::CleanupPanicked, None, false, None)?;
    harness.completion()?;
    harness.submit_close(Moment::from_nanos(5_000_000_000))?;
    let error = harness.completion().err().ok_or("Expected actual cleanup panic")?;
    let CampaignError::CleanupPanicked(cause) =
        *error.downcast::<CampaignError>().map_err(|_| "Missing typed cleanup panic")?
    else {
        return Err("Missing panic payload".into());
    };
    assert_eq!(cause.as_str(), "original-cleanup-panic");
    assert!(!harness.campaign.settled());
    assert!(!harness.campaign.needs_input(harness.now())?);
    harness.finish()
}

#[test]
fn cancellation_during_pending_close_waits_for_actual_closure_and_prohibits_successor() -> TestResult
{
    let mut harness = Harness::new()?;
    let fixture = harness.submit(Behavior::Unavailable, Some("close"), false, None)?;
    harness.completion()?;
    harness.submit_close(Moment::from_nanos(5_000_000_000))?;
    fixture.entered()?;
    let original = harness.campaign.cleanup_until;
    harness.campaign.cancel(ShutdownBudget::new(harness.now())?);
    assert_eq!(harness.campaign.cleanup_until, original);
    assert!(!harness.campaign.settled());
    assert_eq!(harness.campaign.consume(&empty_batch(), harness.now())?, CampaignStep::Waiting);
    fixture.release();
    assert_eq!(harness.completion()?, CampaignStep::Failed);
    assert!(harness.campaign.settled());
    assert!(!harness.campaign.needs_input(harness.now())?);
    assert_eq!(harness.retained_bytes(), crate::readiness_transport::READINESS_TRANSPORT_BYTES);
    let cause = harness.campaign.take_failure().ok_or("Missing retained first native failure")?;
    drop(cause);
    assert_eq!(harness.retained_bytes(), 0);
    harness.finish()
}

#[test]
fn actual_admission_rejection_closes_campaign_before_factory_execution() -> TestResult {
    let mut harness = Harness::new()?;
    let input = ReadinessProbeInput::attached(harness.lease.clone(), 8188)?;
    let mut spec = harness.spec(Moment::from_nanos(2_000_000_000))?;
    spec.reservation_bytes = 1;
    assert!(matches!(
        harness.campaign.submit_attempt(&harness.client, &harness.attempt, spec, input),
        Err(CampaignError::Admission(_))
    ));
    assert!(!harness.campaign.needs_input(harness.now())?);
    assert!(harness.campaign.settled());
    assert_eq!(harness.retained_bytes(), 0);
    harness.finish()
}
