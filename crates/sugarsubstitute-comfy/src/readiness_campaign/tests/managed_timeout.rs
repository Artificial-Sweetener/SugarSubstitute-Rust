// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise finite probe expiry through actual execution, HTTP-slot custody and campaign cleanup.
//! Existing attached HTTP fixtures exercise timing only and grant no managed native authority.
use super::managed::managed_campaign;
use super::*;
use crate::readiness_transport::{ProbeRetryDisposition, campaign_fixture::FixtureHttp};
use std::{sync::Arc, time::Duration};
fn live() -> TestResult<Harness> {
    let mut h = Harness::new()?;
    h.campaign = managed_campaign()?;
    h.campaign.admit_managed_live(h.now())?;
    h.set_time(600_000_000_000);
    Ok(h)
}
fn expire(h: &mut Harness, fail_close: bool) -> TestResult<(Arc<FixtureHttp>, CampaignStep)> {
    let until = h.campaign.probe_until(h.now())?;
    let fixture = h.submit(Behavior::Success, Some("begin"), fail_close, None)?;
    fixture.entered()?;
    assert_eq!(h.campaign.attempt.as_ref().ok_or("Actual probe absent")?.until, until);
    h.set_time(until.as_nanos());
    fixture.release();
    let step = h.completion()?;
    let Some(CampaignError::Transport(cause)) = h.campaign.failure() else {
        return Err("Actual timeout cause absent".into());
    };
    assert_eq!(cause.stage(), "attempt_timed_out");
    assert!(cause.source().is_some());
    assert!(h.campaign.take_readiness().is_none());
    assert!(!h.campaign.settled());
    assert!(h.retained_bytes() > 0);
    assert!(fixture.closed.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_empty());
    Ok((fixture, step))
}
fn cleanup_retry(h: &mut Harness, fixture: &FixtureHttp, delay: Duration) -> TestResult {
    let frozen = h.campaign.cleanup_deadline(h.now())?;
    assert_eq!(frozen, h.now().checked_add(Duration::from_secs(15))?);
    h.set_time(h.now().as_nanos().checked_add(1).ok_or("Fixture clock exhausted")?);
    assert_eq!(h.campaign.cleanup_deadline(h.now())?, frozen);
    assert!(!h.campaign.needs_input(h.now())?);
    let completed_at = h.now();
    h.submit_close(frozen)?;
    assert_eq!(h.completion()?, CampaignStep::Progressed);
    assert!(h.campaign.settled());
    assert_eq!(
        *fixture.closed.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
        [3, 2, 1]
    );
    let next = h.campaign.next_deadline().ok_or("Original backoff absent")?;
    assert_eq!(next, completed_at.checked_add(delay)?);
    h.set_time(next.as_nanos().checked_sub(1).ok_or("Fixture backoff underflow")?);
    assert!(!h.campaign.needs_input(h.now())?);
    h.set_time(next.as_nanos());
    assert!(h.campaign.needs_input(h.now())?);
    Ok(())
}
#[test]
fn managed_live_repeated_actual_probe_timeouts_cleanup_then_reach_real_http_ready() -> TestResult {
    let mut h = live()?;
    for delay in [Duration::from_millis(50), Duration::from_millis(100)] {
        let (fixture, step) = expire(&mut h, false)?;
        if step != CampaignStep::Progressed {
            h.finish()?;
            return Err(
                "Actual finite probe timeout incorrectly ended admitted live managed waiting"
                    .into(),
            );
        }
        assert_eq!(h.campaign.campaign_until(), None);
        cleanup_retry(&mut h, &fixture, delay)?;
        // Keep the first timeout cause retained: the second timeout must replace it as retryable.
        assert!(
            matches!(h.campaign.failure(), Some(CampaignError::Transport(cause)) if cause.stage() == "attempt_timed_out")
        );
    }
    let start = h.now();
    h.submit(Behavior::Success, None, false, None)?;
    assert_eq!(
        h.campaign.attempt.as_ref().ok_or("Fresh actual probe absent")?.until,
        start.checked_add(Duration::from_secs(2))?
    );
    assert_eq!(h.completion()?, CampaignStep::Ready);
    let proof = h.campaign.take_readiness().ok_or("Actual fresh HTTP result absent")?;
    assert_eq!(proof.managed_resource(), None);
    proof.validate_current()?;
    drop(proof);
    h.finish()
}
#[test]
fn finite_probe_timeout_remains_terminal_for_attached_and_unadmitted_managed_campaigns()
-> TestResult {
    for managed in [false, true] {
        let mut h = Harness::new()?;
        if managed {
            h.campaign = managed_campaign()?;
        }
        let original = h.campaign.campaign_until();
        let (_, step) = expire(&mut h, false)?;
        assert_eq!(step, CampaignStep::Failed);
        assert_eq!(h.campaign.campaign_until(), original);
        assert!(!h.campaign.needs_input(h.now())?);
        assert!(h.campaign.take_readiness().is_none());
        h.finish()?;
    }
    Ok(())
}
#[test]
fn managed_probe_timeout_does_not_hide_fatal_http_cleanup_or_late_success_delivery() -> TestResult {
    for behavior in [Behavior::Malformed, Behavior::WrongAsset, Behavior::Panicked] {
        let mut h = live()?;
        let (fixture, step) = expire(&mut h, false)?;
        assert_eq!(step, CampaignStep::Progressed);
        cleanup_retry(&mut h, &fixture, Duration::from_millis(50))?;
        h.submit(behavior, None, false, None)?;
        assert_eq!(h.completion()?, CampaignStep::Failed);
        assert!(
            matches!(h.campaign.failure(), Some(CampaignError::Transport(cause)) if cause.retry_disposition() == ProbeRetryDisposition::Terminal)
        );
        assert!(!h.campaign.needs_input(h.now())?);
        h.finish()?;
    }
    let mut h = live()?;
    let (_, step) = expire(&mut h, true)?;
    assert_eq!(step, CampaignStep::Progressed);
    let frozen = h.campaign.cleanup_deadline(h.now())?;
    h.submit_close(frozen)?;
    let error = h.completion().err().ok_or("Actual failed cleanup admitted a successor")?;
    assert!(!h.campaign.needs_input(h.now())?);
    assert_eq!(h.campaign.cleanup_deadline(h.now())?, frozen);
    assert!(h.campaign.take_readiness().is_none());
    assert!(error.source().is_some());
    drop(error);
    h.finish()?;
    let mut h = live()?;
    let until = h.campaign.probe_until(h.now())?;
    let fixture = h.submit(Behavior::Success, Some("close"), false, None)?;
    fixture.entered()?;
    fixture.release();
    h.wait_terminal_without_consuming()?;
    h.set_time(until.as_nanos());
    assert_eq!(h.consume_saved()?, CampaignStep::Failed);
    assert!(h.campaign.take_readiness().is_none());
    assert!(!h.campaign.needs_input(h.now())?);
    h.finish()
}
#[test]
fn cancellation_after_actual_managed_probe_timeout_prevents_fresh_admission() -> TestResult {
    let mut h = live()?;
    let (_, step) = expire(&mut h, false)?;
    assert_eq!(step, CampaignStep::Progressed);
    let shutdown = ShutdownBudget::new(h.now())?;
    let first_cleanup = h.campaign.cleanup_deadline(h.now())?;
    h.campaign.cancel(shutdown);
    h.campaign.cancel(ShutdownBudget::new(h.now().checked_add(Duration::from_secs(1))?)?);
    assert_eq!(h.campaign.cleanup_deadline(h.now())?, first_cleanup.min(shutdown.settle_until));
    h.submit_close(first_cleanup.min(shutdown.settle_until))?;
    assert_eq!(h.completion()?, CampaignStep::Failed);
    assert!(h.campaign.settled());
    assert!(!h.campaign.needs_input(h.now())?);
    assert_eq!(h.campaign.next_deadline(), None);
    assert!(h.submit(Behavior::Success, None, false, None).is_err());
    h.finish()
}
