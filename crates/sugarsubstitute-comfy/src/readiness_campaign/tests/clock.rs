// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Clock rejection closes real admission while preserving actual completion and cleanup custody.
use super::*;

#[test]
fn regressing_consume_cancels_pending_actual_attempt_and_retains_custody() -> TestResult {
    let mut h = Harness::new()?;
    let external = h.submit(Behavior::Success, Some("begin"), false, None)?;
    external.entered()?;
    h.set_time(10);
    assert_eq!(h.campaign.consume(&empty_batch(), h.now())?, CampaignStep::Waiting);
    let original_charge = h.retained_bytes();
    assert!(matches!(
        h.campaign.consume(&empty_batch(), Moment::from_nanos(9)),
        Err(CampaignError::Rejected(CampaignRejection::ClockRegression))
    ));
    assert!(h.campaign.attempt.is_some());
    assert!(!h.campaign.settled());
    assert_eq!(h.retained_bytes(), original_charge);
    assert_eq!(h.campaign.next_deadline(), Some(Moment::from_nanos(2_000_000_000)));
    assert!(matches!(
        h.campaign.take_failure(),
        Some(CampaignError::Rejected(CampaignRejection::ClockRegression))
    ));
    external.release();
    assert_eq!(h.completion()?, CampaignStep::Failed);
    assert!(matches!(
        h.campaign.failure(),
        Some(CampaignError::Transport(cause))
            if cause.retry_disposition() == crate::readiness_transport::ProbeRetryDisposition::Terminal
    ));
    assert!(h.campaign.take_readiness().is_none());
    assert!(!h.campaign.needs_input(h.now())?);
    assert!(!h.campaign.settled());
    assert_eq!(h.campaign.budget.until(), Moment::from_nanos(30_000_000_000));
    h.finish()
}

#[test]
fn regressing_consume_cannot_publish_already_completed_actual_success() -> TestResult {
    let mut h = Harness::new()?;
    h.submit(Behavior::Success, None, false, None)?;
    h.wait_terminal_without_consuming()?;
    h.set_time(10);
    assert_eq!(h.campaign.consume(&empty_batch(), h.now())?, CampaignStep::Waiting);
    assert!(matches!(
        h.campaign.consume(&empty_batch(), Moment::from_nanos(9)),
        Err(CampaignError::Rejected(CampaignRejection::ClockRegression))
    ));
    assert_eq!(h.consume_saved()?, CampaignStep::Failed);
    assert!(h.campaign.take_readiness().is_none());
    assert!(matches!(
        h.campaign.failure(),
        Some(CampaignError::Rejected(CampaignRejection::ClockRegression))
    ));
    assert!(!h.campaign.needs_input(h.now())?);
    assert!(!h.campaign.settled());
    h.finish()
}

#[test]
fn regressing_attempt_submission_permanently_closes_fresh_input_admission() -> TestResult {
    let mut h = Harness::new()?;
    h.set_time(10);
    assert_eq!(h.campaign.consume(&empty_batch(), h.now())?, CampaignStep::NeedsInput);
    h.set_time(9);
    let spec = h.spec(Moment::from_nanos(2_000_000_000))?;
    let input = ReadinessProbeInput::attached(h.lease.clone(), 8188)?;
    assert!(matches!(
        h.campaign.submit_attempt(&h.client, &h.attempt, spec, input),
        Err(CampaignError::Rejected(CampaignRejection::ClockRegression))
    ));
    h.set_time(10);
    assert!(!h.campaign.needs_input(h.now())?);
    assert_eq!(h.campaign.consume(&empty_batch(), h.now())?, CampaignStep::Failed);
    assert!(h.campaign.settled());
    assert_eq!(h.retained_bytes(), 0);
    let spec = h.spec(Moment::from_nanos(2_000_000_000))?;
    let input = ReadinessProbeInput::attached(h.lease.clone(), 8188)?;
    assert!(matches!(
        h.campaign.submit_attempt(&h.client, &h.attempt, spec, input),
        Err(CampaignError::Rejected(CampaignRejection::WrongPhase))
    ));
    assert_eq!(h.campaign.next_deadline(), None);
    h.finish()
}

#[test]
fn regressing_cleanup_submission_revokes_pending_readiness_and_retains_real_owner() -> TestResult {
    let mut h = Harness::new()?;
    h.submit(Behavior::Success, None, false, None)?;
    assert_eq!(h.completion()?, CampaignStep::Ready);
    h.set_time(10);
    assert_eq!(h.campaign.consume(&empty_batch(), h.now())?, CampaignStep::Ready);
    let original_charge = h.retained_bytes();
    h.set_time(9);
    let spec = h.spec(Moment::from_nanos(5_000_000_000))?;
    assert!(matches!(
        h.campaign.submit_cleanup(&h.client, &h.cleanup, spec),
        Err(CampaignError::Rejected(CampaignRejection::ClockRegression))
    ));
    assert!(h.campaign.take_readiness().is_none());
    assert_eq!(h.retained_bytes(), original_charge);
    assert!(!h.campaign.settled());
    assert_eq!(h.campaign.next_deadline(), Some(Moment::from_nanos(30_000_000_000)));
    h.set_time(10);
    assert_eq!(h.campaign.consume(&empty_batch(), h.now())?, CampaignStep::Failed);
    h.finish()
}

#[test]
fn cancelled_actual_success_exposes_first_shutdown_and_actual_cleanup_deadlines() -> TestResult {
    let mut h = Harness::new()?;
    h.submit(Behavior::Success, None, false, None)?;
    assert_eq!(h.completion()?, CampaignStep::Ready);
    assert_eq!(h.campaign.next_deadline(), None);
    h.set_time(1_000_000_000);
    h.campaign.cancel(ShutdownBudget::new(h.now())?);
    h.campaign.cancel(ShutdownBudget::new(Moment::from_nanos(40_000_000_000))?);
    assert!(!h.campaign.settled());
    assert_eq!(h.campaign.next_deadline(), Some(Moment::from_nanos(16_000_000_000)));
    h.submit_close(Moment::from_nanos(5_000_000_000))?;
    assert_eq!(h.campaign.next_deadline(), Some(Moment::from_nanos(5_000_000_000)));
    assert_eq!(h.campaign.consume(&empty_batch(), h.now())?, CampaignStep::Waiting);
    assert_eq!(h.campaign.next_deadline(), Some(Moment::from_nanos(5_000_000_000)));
    assert!(!h.campaign.settled());
    assert_eq!(h.completion()?, CampaignStep::Failed);
    assert!(h.campaign.settled());
    assert_eq!(h.campaign.next_deadline(), None);
    h.finish()
}
