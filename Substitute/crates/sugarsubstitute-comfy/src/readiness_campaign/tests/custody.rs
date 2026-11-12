// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire obsolete retry diagnostics only after actual current readiness succeeds.
use super::*;
use crate::readiness_transport::{ProbeRetryDisposition, READINESS_TRANSPORT_BYTES};

#[test]
fn actual_ready_releases_checked_closed_retry_credit_and_keeps_current_probe_credit() -> TestResult
{
    let mut harness = Harness::new()?;
    let first = harness.submit(Behavior::Unavailable, Some("close"), false, None)?;
    assert_eq!(harness.completion()?, CampaignStep::Progressed);
    assert_eq!(harness.retained_bytes(), READINESS_TRANSPORT_BYTES);
    harness.submit_close(Moment::from_nanos(5_000_000_000))?;
    first.entered()?;
    assert!(!harness.campaign.settled());
    assert!(!harness.campaign.needs_input(harness.now())?);
    first.release();
    assert_eq!(harness.completion()?, CampaignStep::Progressed);
    assert_eq!(harness.retained_bytes(), READINESS_TRANSPORT_BYTES);
    assert!(matches!(harness.campaign.failure(), Some(CampaignError::Transport(_))));
    assert_eq!(*first.closed.lock().unwrap_or_else(std::sync::PoisonError::into_inner), [3, 2, 1]);

    harness.set_time(50_000_000);
    assert!(harness.campaign.needs_input(harness.now())?);
    let current = harness.submit(Behavior::Success, Some("begin"), false, None)?;
    current.entered()?;
    assert_eq!(harness.retained_bytes(), 2 * READINESS_TRANSPORT_BYTES);
    assert!(harness.campaign.failure().is_some());
    current.release();
    assert_eq!(harness.completion()?, CampaignStep::Ready);
    assert!(harness.campaign.failure().is_none());
    assert!(!harness.campaign.settled());
    assert_eq!(harness.campaign.budget.until(), Moment::from_nanos(30_000_000_000));
    let proof = harness.campaign.take_readiness().ok_or("Missing actual current readiness")?;
    proof.validate_current()?;
    assert_eq!(proof.process_id(), 4242);
    assert_eq!(
        *current.closed.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
        [3, 2, 1]
    );

    let shutdown = ShutdownBudget::new(harness.now())?;
    harness.campaign.cancel(shutdown);
    harness.submit_close(shutdown.settle_until)?;
    assert_eq!(harness.completion()?, CampaignStep::Failed);
    assert!(harness.campaign.settled());
    assert!(proof.validate_current().is_err());
    let (campaign, attempt) = harness.finish_execution()?;
    assert!(campaign.settled());
    assert!(campaign.failure().is_none());
    assert_eq!(attempt.snapshot().reserved_bytes, 0);
    drop(proof);
    Ok(())
}

#[test]
fn terminal_retry_result_keeps_its_original_cause_and_credit_after_checked_cleanup() -> TestResult {
    for behavior in [Behavior::Malformed, Behavior::WrongAsset, Behavior::Panicked] {
        let mut harness = Harness::new()?;
        harness.submit(Behavior::Unavailable, None, false, None)?;
        assert_eq!(harness.completion()?, CampaignStep::Progressed);
        harness.submit_close(Moment::from_nanos(5_000_000_000))?;
        assert_eq!(harness.completion()?, CampaignStep::Progressed);
        assert_eq!(harness.retained_bytes(), READINESS_TRANSPORT_BYTES);
        harness.set_time(50_000_000);
        let terminal = harness.submit(behavior, Some("begin"), false, None)?;
        terminal.entered()?;
        assert_eq!(harness.retained_bytes(), 2 * READINESS_TRANSPORT_BYTES);
        terminal.release();
        assert_eq!(harness.completion()?, CampaignStep::Failed);
        assert!(harness.campaign.take_readiness().is_none());
        let Some(CampaignError::Transport(cause)) = harness.campaign.failure() else {
            return Err("Missing actual terminal transport cause".into());
        };
        assert_eq!(cause.retry_disposition(), ProbeRetryDisposition::Terminal);
        let original_stage = cause.stage();
        let original_source =
            std::ptr::from_ref(cause.source().ok_or("Missing original terminal cause")?);
        let original_cause = cause.source().ok_or("Missing original terminal cause")?.to_string();

        harness.submit_close(Moment::from_nanos(5_050_000_000))?;
        assert_eq!(harness.completion()?, CampaignStep::Failed);
        assert!(harness.campaign.settled());
        assert!(!harness.campaign.needs_input(harness.now())?);
        let failure = harness.campaign.take_failure().ok_or("Missing retained terminal cause")?;
        let CampaignError::Transport(ref cause) = failure else {
            return Err("Terminal cause changed owner".into());
        };
        assert_eq!(cause.stage(), original_stage);
        assert_eq!(cause.source().ok_or("Missing preserved cause")?.to_string(), original_cause);
        assert!(std::ptr::eq(
            original_source,
            std::ptr::from_ref(cause.source().ok_or("Missing preserved cause")?)
        ));
        let (campaign, attempt) = harness.finish_execution()?;
        assert!(campaign.settled());
        assert!(campaign.failure().is_none());
        assert_eq!(attempt.snapshot().reserved_bytes, READINESS_TRANSPORT_BYTES);
        drop(failure);
        assert_eq!(attempt.snapshot().reserved_bytes, 0);
    }
    Ok(())
}
